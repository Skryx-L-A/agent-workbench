#!/usr/bin/env python3
"""Rough timing of `rangkern serve` on an invented vault (nothing real is read).

Builds a vault of N notes of similar length from an invented vocabulary and a vector file for all of
them, starts `rangkern serve`, and measures at the caller, from writing a request line to reading
the answer line:

  start      start of the process until the answer to `ping`
  search     200 searches with a question vector on the unchanged vault (median, p90, p99, max)
  change     the first search after a note was rewritten (different size), 20 times
  same_size  the first search after a note was rewritten with the same size and the old mtime, 20 times
  rss        resident memory after the 200 searches (kB, from /proc)
  equal      20 answers of the process against `rangkern search` on the same files

The numbers are rough: one run, one machine, no isolation from other load. The vault is written
below --work and removed at the end. Usage:

  serve_messung.py --bin target/release/rangkern --work /var/tmp/serve-messung [--notes 1500]
"""

import argparse
import json
import math
import os
import random
import shutil
import statistics
import subprocess
import sys
import time

SYLLABLES = ["ba", "be", "bi", "bo", "da", "de", "di", "do", "fa", "fe", "ga", "ge", "gi", "ha", "he",
             "hi", "ka", "ke", "ki", "ko", "la", "le", "li", "lo", "ma", "me", "mi", "mo", "na", "ne",
             "ni", "no", "pa", "pe", "ra", "re", "ri", "ro", "sa", "se", "si", "so", "ta", "te", "ti",
             "to", "ung", "keit", "schaft", "lich", "tion", "gar", "wan", "bruch", "stein", "feld"]


def make_vocabulary(rng, size, long_words):
    """Invented words; with `long_words` of three to six syllables, so that most stems get 4-grams."""
    low, high = (3, 6) if long_words else (1, 4)
    words = set()
    while len(words) < size:
        words.add("".join(rng.choice(SYLLABLES) for _ in range(rng.randint(low, high))))
    return sorted(words)


def make_note(rng, vocabulary, weights, words, index):
    title = " ".join(rng.choices(vocabulary, weights, k=2)).capitalize()
    head = ["---", f"title: {title}"]
    if index % 20 == 0:
        head.append("class: derived")
    head.append("---")
    lines = []
    remaining = words
    while remaining > 0:
        count = min(remaining, rng.randint(6, 18))
        lines.append(" ".join(rng.choices(vocabulary, weights, k=count)))
        remaining -= count
    return "\n".join(head + lines) + "\n"


def build_vault(root, rng, notes, words, dim, long_words):
    vocabulary = make_vocabulary(rng, 6000, long_words)
    weights = [1.0 / (rank + 1) ** 0.9 for rank in range(len(vocabulary))]
    rels = []
    for i in range(notes):
        rel = f"d{i % 40:02d}/n{i:04d}.md"
        path = os.path.join(root, "vault", rel)
        os.makedirs(os.path.dirname(path), exist_ok=True)
        with open(path, "w", encoding="utf-8") as handle:
            handle.write(make_note(rng, vocabulary, weights, words + rng.randint(-words // 4, words // 4), i))
        rels.append(rel)
    vectors = {"notes": {rel: [round(rng.gauss(0, 1), 6) for _ in range(dim)] for rel in rels}}
    with open(os.path.join(root, "vectors.json"), "w") as handle:
        json.dump(vectors, handle)
    return vocabulary, weights, rels


class Server:
    def __init__(self, binary, vault, vectors):
        self.started = time.perf_counter()
        self.process = subprocess.Popen(
            [binary, "serve", vault, "--vectors", vectors],
            stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE, env={"LANG": "C.UTF-8", "TZ": "UTC"},
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


def percentile(values, share):
    ordered = sorted(values)
    return ordered[min(len(ordered) - 1, math.ceil(share * len(ordered)) - 1)]


def summary(values):
    return {
        "median_ms": round(1000 * statistics.median(values), 2),
        "p90_ms": round(1000 * percentile(values, 0.9), 2),
        "p99_ms": round(1000 * percentile(values, 0.99), 2),
        "max_ms": round(1000 * max(values), 2),
        "n": len(values),
    }


def same(a, b):
    if isinstance(a, float) or isinstance(b, float):
        return isinstance(a, (int, float)) and isinstance(b, (int, float)) and math.isclose(a, b, rel_tol=1e-9, abs_tol=1e-12)
    if isinstance(a, dict) and isinstance(b, dict):
        return a.keys() == b.keys() and all(same(a[key], b[key]) for key in a)
    if isinstance(a, list) and isinstance(b, list):
        return len(a) == len(b) and all(same(x, y) for x, y in zip(a, b))
    return a == b


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--bin", required=True)
    parser.add_argument("--work", required=True)
    parser.add_argument("--notes", type=int, default=1500)
    parser.add_argument("--words", type=int, default=600)
    parser.add_argument("--dim", type=int, default=768)
    parser.add_argument("--searches", type=int, default=200)
    parser.add_argument("--long", action="store_true", help="words of three to six syllables")
    parser.add_argument("--seed", type=int, default=7)
    options = parser.parse_args()
    rng = random.Random(options.seed)
    shutil.rmtree(options.work, ignore_errors=True)
    os.makedirs(options.work)
    vocabulary, weights, rels = build_vault(options.work, rng, options.notes, options.words, options.dim, options.long)
    vault = os.path.join(options.work, "vault")
    vectors = os.path.join(options.work, "vectors.json")
    size = sum(os.path.getsize(os.path.join(vault, rel)) for rel in rels)
    result = {"notes": len(rels), "vault_bytes": size, "dim": options.dim}
    server = Server(options.bin, vault, vectors)
    try:
        answer, _ = server.ask({"op": "ping"})
        result["start_ms"] = round(1000 * (time.perf_counter() - server.started), 1)
        assert answer == {"ok": True}, answer

        def question():
            words = rng.choices(vocabulary, weights, k=rng.randint(2, 5))
            return {
                "op": "search", "query": " ".join(words), "k": 5, "today": "2026-11-18",
                "query_vector": [round(rng.gauss(0, 1), 6) for _ in range(options.dim)],
            }

        times, asked, answers = [], [], []
        for _ in range(options.searches):
            request = question()
            answer, elapsed = server.ask(request)
            assert "hits" in answer, answer
            times.append(elapsed)
            asked.append(request)
            answers.append(answer)
        result["search"] = summary(times)
        result["rss_kb_after_searches"] = server.rss_kb()

        changed, same_size = [], []
        for round_number in range(20):
            rel = rng.choice(rels)
            path = os.path.join(vault, rel)
            with open(path, "a", encoding="utf-8") as handle:
                handle.write(f"zusatz {round_number} " + " ".join(rng.choices(vocabulary, weights, k=30)) + "\n")
            _, elapsed = server.ask(question())
            changed.append(elapsed)
        for round_number in range(20):
            rel = rng.choice(rels)
            path = os.path.join(vault, rel)
            before = os.stat(path)
            with open(path, "rb") as handle:
                data = bytearray(handle.read())
            for at in range(len(data) - 1, 0, -1):
                if 97 <= data[at] <= 122:
                    data[at] = 97 if data[at] != 97 else 98
                    break
            with open(path, "wb") as handle:
                handle.write(data)
            os.utime(path, ns=(before.st_atime_ns, before.st_mtime_ns))
            _, elapsed = server.ask(question())
            same_size.append(elapsed)
        result["change"] = summary(changed)
        result["same_size"] = summary(same_size)

        agree = 0
        for request in asked[:20]:
            answer, _ = server.ask(request)
            with open(os.path.join(options.work, "q.json"), "w") as handle:
                json.dump({"query": request["query_vector"], "notes": json.load(open(vectors))["notes"]}, handle)
            cold = subprocess.run(
                [options.bin, "search", vault, "--vectors", os.path.join(options.work, "q.json"),
                 "--today", request["today"], "--", request["query"]],
                capture_output=True, text=True, env={"LANG": "C.UTF-8", "TZ": "UTC"},
            )
            started = time.perf_counter()
            if same(json.loads(cold.stdout), answer):
                agree += 1
            del started
        result["equal"] = f"{agree} of 20"

        cold_times = []
        for request in asked[:3]:
            started = time.perf_counter()
            subprocess.run(
                [options.bin, "search", vault, "--vectors", os.path.join(options.work, "q.json"),
                 "--today", request["today"], "--", request["query"]],
                capture_output=True, text=True, env={"LANG": "C.UTF-8", "TZ": "UTC"},
            )
            cold_times.append(time.perf_counter() - started)
        result["rangkern_search_cold_ms"] = [round(1000 * value, 1) for value in cold_times]

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
