#!/usr/bin/env python3
"""Rough timing of `rangkern --embed` against a stand-in for Ollama on loopback (nothing real is read).

Builds an invented vault of N notes and a `gardener.db` with a vector for every note (as `vdb_messung.py`
does), starts a stand-in for Ollama on 127.0.0.1 with a free port (it answers `/api/embed` at once with a
vector made from the text, `Content-Length`, no delay), and measures at the caller:

  cold_embed   `rangkern search --index-db .. --embed URL`, one process per question
  cold_vector  `rangkern search --index-db .. --query-vector q.json`, one process per question (the same vector)
  equal        the output of both, byte for byte
  start        `rangkern serve --index-db .. --embed URL`: start of the process until the answer to `ping`
  embed        200 searches of `serve` without `query_vector` (the process embeds the question)
  vector       200 searches of `serve` with `query_vector` (the same vectors)
  calls        the calls the stand-in saw (one per question without `query_vector`)

The stand-in costs no time, so the numbers are the cost of the program itself: the connection, the request,
the answer. A real Ollama adds its own time. The numbers are rough: one run, one machine, no isolation from
other load. Everything is written below --work and removed at the end; the stand-in is stopped and its port
is checked to be closed. Usage:

  embed_messung.py --bin target/release/rangkern --work /var/tmp/embed-messung [--notes 1400] [--dim 768]
"""

import argparse
import hashlib
import json
import os
import random
import shutil
import socket
import subprocess
import sys
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import serve_messung as base  # noqa: E402  (the vault builder and the summary)
import vdb_messung as vdb  # noqa: E402  (the database writer)

ENV = {"LANG": "C.UTF-8", "TZ": "UTC"}
DIM = 768


def vector_of(text, dim):
    """The vector the stand-in gives for a text."""
    rng = random.Random(int.from_bytes(hashlib.sha256(text.encode()).digest()[:8], "big"))
    return [round(rng.gauss(0, 1), 6) for _ in range(dim)]


class Fake(BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"
    calls = []
    dim = DIM

    def do_POST(self):
        length = int(self.headers.get("Content-Length", "0"))
        body = json.loads(self.rfile.read(length))
        Fake.calls.append(body["input"])
        data = json.dumps({"embeddings": [vector_of(body["input"], Fake.dim)]}).encode()
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def log_message(self, *args):
        pass


class Serve:
    def __init__(self, binary, vault, index, url):
        self.started = time.perf_counter()
        self.process = subprocess.Popen(
            [binary, "serve", vault, "--index-db", index, "--embed", url],
            stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE, env=ENV,
        )

    def ask(self, request):
        started = time.perf_counter()
        self.process.stdin.write((json.dumps(request) + "\n").encode())
        self.process.stdin.flush()
        line = self.process.stdout.readline()
        return json.loads(line), time.perf_counter() - started


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--bin", required=True)
    parser.add_argument("--work", required=True)
    parser.add_argument("--notes", type=int, default=1400)
    parser.add_argument("--words", type=int, default=600)
    parser.add_argument("--dim", type=int, default=DIM)
    parser.add_argument("--searches", type=int, default=200)
    parser.add_argument("--cold", type=int, default=20)
    parser.add_argument("--seed", type=int, default=11)
    options = parser.parse_args()
    Fake.dim = options.dim
    rng = random.Random(options.seed)
    shutil.rmtree(options.work, ignore_errors=True)
    os.makedirs(options.work)
    vocabulary, weights, rels = base.build_vault(options.work, rng, options.notes, options.words, options.dim, False)
    os.unlink(os.path.join(options.work, "vectors.json"))
    vault = os.path.join(options.work, "vault")
    state = os.path.join(options.work, "state")
    os.makedirs(state)
    index = os.path.join(state, "gardener.db")
    vdb.write_db(index, [(rel, vdb.vector_text(rng, options.dim)) for rel in rels])

    httpd = ThreadingHTTPServer(("127.0.0.1", 0), Fake)
    httpd.daemon_threads = True
    port = httpd.server_address[1]
    threading.Thread(target=httpd.serve_forever, daemon=True).start()
    url = f"http://127.0.0.1:{port}"
    result = {"notes": len(rels), "dim": options.dim, "stand_in": "python http.server, no delay"}

    def question_text():
        return " ".join(rng.choices(vocabulary, weights, k=rng.randint(2, 5)))

    try:
        cold_embed, cold_vector, agree = [], [], 0
        for number in range(options.cold):
            text = question_text()
            qpath = os.path.join(options.work, f"q{number}.json")
            with open(qpath, "w") as handle:
                json.dump(vector_of(text, options.dim), handle)
            common = [options.bin, "search", vault, text, "-k", "5", "--today", "2026-11-19", "--index-db", index]
            started = time.perf_counter()
            a = subprocess.run(common + ["--embed", url], capture_output=True, text=True, env=ENV)
            cold_embed.append(time.perf_counter() - started)
            started = time.perf_counter()
            b = subprocess.run(common + ["--query-vector", qpath], capture_output=True, text=True, env=ENV)
            cold_vector.append(time.perf_counter() - started)
            assert a.returncode == 0 and b.returncode == 0, (a.stderr, b.stderr)
            agree += a.stdout == b.stdout
        result["cold_embed"] = base.summary(cold_embed)
        result["cold_vector"] = base.summary(cold_vector)
        result["equal"] = f"{agree} of {options.cold}"

        before = len(Fake.calls)
        server = Serve(options.bin, vault, index, url)
        try:
            answer, _ = server.ask({"op": "ping"})
            result["start_ms"] = round(1000 * (time.perf_counter() - server.started), 1)
            assert answer == {"ok": True}, answer
            texts = [question_text() for _ in range(options.searches)]
            embed, vector, equal = [], [], 0
            for text in texts:
                a, elapsed_a = server.ask({"op": "search", "query": text, "k": 5, "today": "2026-11-19"})
                b, elapsed_b = server.ask({"op": "search", "query": text, "k": 5, "today": "2026-11-19",
                                           "query_vector": vector_of(text, options.dim)})
                assert "hits" in a and "hits" in b, (a, b)
                embed.append(elapsed_a)
                vector.append(elapsed_b)
                equal += a == b
            result["embed"] = base.summary(embed)
            result["vector"] = base.summary(vector)
            result["serve_equal"] = f"{equal} of {options.searches}"
            result["calls"] = len(Fake.calls) - before
            answer, _ = server.ask({"op": "quit"})
            assert answer == {"ok": True}, answer
            result["exit"] = server.process.wait(timeout=10)
        finally:
            if server.process.poll() is None:
                server.process.kill()
                server.process.wait()
    finally:
        httpd.shutdown()
        httpd.server_close()
        shutil.rmtree(options.work, ignore_errors=True)
    probe = socket.socket()
    probe.settimeout(1)
    result["port_closed_after"] = probe.connect_ex(("127.0.0.1", port)) != 0
    probe.close()
    json.dump(result, sys.stdout, indent=1)
    print()


if __name__ == "__main__":
    main()
