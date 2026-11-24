#!/usr/bin/env python3
"""Rough timing of `rangkern --index-db` on invented databases (nothing real is read).

Builds an invented vault of N notes, a `gardener.db` with a vector for every note (768 numbers as JSON
text, the table of the model) and an `extra-embeddings.db` with a few rows (some of them overlap), then
measures at the caller:

  cold_db        `rangkern search --index-db .. --extra-db .. --query-vector q.json`, one process per question
  cold_file      `rangkern search --vectors F`, F written beforehand from the same databases (the helper of
                 the model does this; writing F is not timed), one process per question
  equal          the output of both, byte for byte, for every question of the cold runs
  start          `rangkern serve --index-db ..`: start of the process until the answer to `ping`
  search         200 searches with a question vector on unchanged databases
  change         the first search after one row of `gardener.db` was written again, 20 times
  rss            resident memory after the searches (kB, from /proc)
  busy           after every answer of `serve` a second connection writes a row without waiting; the number
                 of `database is locked` errors in 200 rounds

The numbers are rough: one run, one machine, no isolation from other load. Everything is written below
--work and removed at the end. Usage:

  vdb_messung.py --bin target/release/rangkern --work /var/tmp/vdb-messung [--notes 1400] [--dim 768]
"""

import argparse
import json
import os
import random
import shutil
import sqlite3
import statistics
import subprocess
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import serve_messung as base  # noqa: E402  (the vault builder and the summary of the other tool)

ENV = {"LANG": "C.UTF-8", "TZ": "UTC"}
SCHEMA = "CREATE TABLE embeddings (rel TEXT PRIMARY KEY, hash TEXT NOT NULL, vector TEXT NOT NULL, updated REAL NOT NULL)"


def vector_text(rng, dim):
    return json.dumps([round(rng.gauss(0, 1), 6) for _ in range(dim)])


def write_db(path, rows):
    connection = sqlite3.connect(path)
    connection.execute(SCHEMA)
    connection.executemany("INSERT INTO embeddings VALUES (?, 'h', ?, 0.0)", rows)
    connection.commit()
    connection.close()


def merged_from_dbs(index, extra):
    """What `load_merged_embeddings` returns for the two databases (read only)."""
    merged = {}
    for path in (index, extra):
        if not os.path.exists(path):
            continue
        connection = sqlite3.connect(f"file:{path}?mode=ro", uri=True)
        merged.update({rel: json.loads(vec) for rel, vec in connection.execute("SELECT rel, vector FROM embeddings")})
        connection.close()
    return merged


class Serve:
    def __init__(self, binary, vault, index, extra):
        self.started = time.perf_counter()
        self.process = subprocess.Popen(
            [binary, "serve", vault, "--index-db", index, "--extra-db", extra],
            stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE, env=ENV,
        )

    def ask(self, request):
        started = time.perf_counter()
        self.process.stdin.write((json.dumps(request) + "\n").encode())
        self.process.stdin.flush()
        line = self.process.stdout.readline()
        elapsed = time.perf_counter() - started
        return json.loads(line), elapsed

    def rss_kb(self):
        with open(f"/proc/{self.process.pid}/status") as status:
            for line in status:
                if line.startswith("VmRSS:"):
                    return int(line.split()[1])
        return None


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--bin", required=True)
    parser.add_argument("--work", required=True)
    parser.add_argument("--notes", type=int, default=1400)
    parser.add_argument("--words", type=int, default=600)
    parser.add_argument("--dim", type=int, default=768)
    parser.add_argument("--searches", type=int, default=200)
    parser.add_argument("--cold", type=int, default=20)
    parser.add_argument("--seed", type=int, default=11)
    options = parser.parse_args()
    rng = random.Random(options.seed)
    shutil.rmtree(options.work, ignore_errors=True)
    os.makedirs(options.work)
    vocabulary, weights, rels = base.build_vault(options.work, rng, options.notes, options.words, options.dim, False)
    os.unlink(os.path.join(options.work, "vectors.json"))
    vault = os.path.join(options.work, "vault")
    state = os.path.join(options.work, "state")
    os.makedirs(state)
    index = os.path.join(state, "gardener.db")
    extra = os.path.join(state, "extra-embeddings.db")
    write_db(index, [(rel, vector_text(rng, options.dim)) for rel in rels])
    overlap = rng.sample(rels, 8)
    write_db(extra, [(rel, vector_text(rng, options.dim)) for rel in overlap] + [("MOC.md", vector_text(rng, options.dim))])
    result = {"notes": len(rels), "dim": options.dim, "db_bytes": os.path.getsize(index)}

    def question():
        words = rng.choices(vocabulary, weights, k=rng.randint(2, 5))
        return {
            "op": "search", "query": " ".join(words), "k": 5, "today": "2026-11-18",
            "query_vector": [round(rng.gauss(0, 1), 6) for _ in range(options.dim)],
        }

    # cold runs: one process per question, from the databases and from the file of the model's helper
    notes = merged_from_dbs(index, extra)
    cold_db, cold_file, agree = [], [], 0
    for number in range(options.cold):
        request = question()
        qpath = os.path.join(options.work, f"q{number}.json")
        fpath = os.path.join(options.work, f"f{number}.json")
        with open(qpath, "w") as handle:
            json.dump(request["query_vector"], handle)
        with open(fpath, "w") as handle:
            json.dump({"query": request["query_vector"], "notes": notes}, handle)
        common = [options.bin, "search", vault]
        tail = ["-k", "5", "--today", request["today"]]
        started = time.perf_counter()
        a = subprocess.run(common + [request["query"]] + tail + ["--index-db", index, "--extra-db", extra, "--query-vector", qpath],
                           capture_output=True, text=True, env=ENV)
        cold_db.append(time.perf_counter() - started)
        started = time.perf_counter()
        b = subprocess.run(common + [request["query"]] + tail + ["--vectors", fpath], capture_output=True, text=True, env=ENV)
        cold_file.append(time.perf_counter() - started)
        assert a.returncode == 0 and b.returncode == 0, (a.stderr, b.stderr)
        if a.stdout == b.stdout:
            agree += 1
    result["cold_db"] = base.summary(cold_db)
    result["cold_file"] = base.summary(cold_file)
    result["equal"] = f"{agree} of {options.cold}"

    server = Serve(options.bin, vault, index, extra)
    try:
        answer, _ = server.ask({"op": "ping"})
        result["start_ms"] = round(1000 * (time.perf_counter() - server.started), 1)
        assert answer == {"ok": True}, answer
        times = []
        for _ in range(options.searches):
            answer, elapsed = server.ask(question())
            assert "hits" in answer, answer
            times.append(elapsed)
        result["search"] = base.summary(times)
        result["rss_kb_after_searches"] = server.rss_kb()

        changed = []
        for number in range(20):
            connection = sqlite3.connect(index)
            connection.execute("INSERT OR REPLACE INTO embeddings VALUES (?, 'h', ?, 0.0)", (rng.choice(rels), vector_text(rng, options.dim)))
            connection.commit()
            connection.close()
            answer, elapsed = server.ask(question())
            assert "hits" in answer, answer
            changed.append(elapsed)
        result["change"] = base.summary(changed)

        locked = 0
        for number in range(200):
            writer = sqlite3.connect(index, timeout=0)
            try:
                writer.execute("INSERT OR REPLACE INTO embeddings VALUES (?, 'h', ?, 0.0)", (rng.choice(rels), vector_text(rng, options.dim)))
                writer.commit()
            except sqlite3.OperationalError as error:
                if "locked" in str(error) or "busy" in str(error):
                    locked += 1
                else:
                    raise
            finally:
                writer.close()
            server.ask(question())
        result["busy_errors_in_200_rounds"] = locked
        result["files_in_state_folder"] = sorted(os.listdir(state))

        answer, _ = server.ask({"op": "quit"})
        assert answer == {"ok": True}, answer
        result["exit"] = server.process.wait(timeout=10)
    finally:
        if server.process.poll() is None:
            server.process.kill()
            server.process.wait()
        shutil.rmtree(options.work, ignore_errors=True)
    json.dump(result, sys.stdout, indent=1)
    print()


if __name__ == "__main__":
    main()
