"""Generates tests/data/*.json: expected values of the Python model for the unit tests of `rangkern`.

Usage (with the Python of the reference, which has `braincli` installed):
    python -I -B gen_unit_data.py <output dir>

Everything is invented. The values come from the model itself (`braincli.rangkern`, `numpy`), so the
unit tests pin the model's behaviour, not the author's reading of it.
"""
import json
import os
import random
import sys
import tempfile
from pathlib import Path

for _name in list(os.environ):
    if _name.startswith(("BRAIN_", "WB_")):
        del os.environ[_name]
os.environ["BRAIN_SEARCH_STEM_CACHE"] = "0"

import numpy as np  # noqa: E402
from braincli import rangkern as rk  # noqa: E402
from braincli import vault as vm  # noqa: E402

OUT = Path(sys.argv[1])
OUT.mkdir(parents=True, exist_ok=True)
rnd = random.Random(20261118)

WORDS = ("garten beet rasen schnecke schnecken zaun duese bewaesserung tropfschlauch kaffee maschine entkalken "
         "einbettung einbettungen suchindex index lichtschalter steckdose flur kueche ärger öl über straße strasse "
         "rhabarber kompost mulch frühjahr fruehjahr vertikutieren saat ernte tomaten gurken salat wetter regen "
         "sonne wind projekt stand status entscheidung übersicht moc kern aufgabe offen erledigt fehler test lauf "
         "ausgabe tor quelle original sitzung notiz vault gärtner linking embedding vektor suche rang bm25 titel "
         "grundstück grundstücke unterbau fundament dach ziegel schornstein heizung pumpe rohr ventil sensor").split()
STOP = "der die das und oder nicht ich wir the and of ist nicht auch".split()


def dump(name, data):
    (OUT / name).write_text(json.dumps(data, ensure_ascii=False, indent=0) + "\n", encoding="utf-8")


# ---- tokens ---------------------------------------------------------------------------------------------
TEXTS = [
    "", " ", "Einbettungen Suchindex", "Wer's nicht weiß, kann's nicht wissen", "can't won't ' '' 'a' a'b",
    "ÄRGER über die STRASSE", "Straße straße STRASSE", "ΑΣ ΑΣ. ΑΣ'Α Σ ΟΔΟΣ", "İstanbul İ", "x_y_z a_b _ __ _a",
    "v2.0 version2 2026-11-18 3x", "٣٤ ١٢ ½ ① Ⅷ x² ²", "日本語のテキスト 한국어", "naïve café résumé", "é ä ä",
    "ǅungla ǆ ǈ", "ﬁnden ﬃ", "emoji 😀 text 😀text", "a-b c/d e.f g,h", "ÜBER-Größe MAẞE", "Ünïcödé ẞ ß ss",
    "aeußerst oeffnen ueberall quelle que", "grundstücke grundstück haus häuser", "Zeile1\nZeile2\r\nZeile3\tTab",
    "Wörter mit Bindestrich-Wörter und Unter_strich", "aaaaaa aaaaa aaaaaaa bbbbbbbb", "straßenbahn straßenbahnen",
    "ªº ʰ ˆ ʼ", "ǀ ǁ", "​ zero​width", "a b a b",
]
for _ in range(120):
    TEXTS.append(" ".join(rnd.choice(WORDS + STOP) + rnd.choice(["", "", "", "n", "en", "er", "e", "'s", "2"]) for _ in range(rnd.randint(1, 8))))
tokens = []
for t in TEXTS:
    tokens.append({"text": t, "words": rk.tokenize(t), "stems": rk.stem_tokens(t), "index": rk.index_tokens(t),
                   "counts": list(rk.index_counts(t).items())})
dump("tokens.json", tokens)

# ---- character classes ----------------------------------------------------------------------------------
cps = []
for cp in list(range(0, 0x250)) + [rnd.randrange(0x110000) for _ in range(6000)] + list(range(0x2000, 0x2100)) + list(range(0x3000, 0x3040)):
    if 0xD800 <= cp <= 0xDFFF:
        continue
    c = chr(cp)
    cps.append([cp, c.isalpha(), c.isalnum() or c == "_", c.lower() if c.lower() != c else None])
dump("chars.json", cps)
dump("lowermap.json", {
    "lower": [[cp, chr(cp).lower()] for cp in range(0x110000) if not 0xD800 <= cp <= 0xDFFF and chr(cp).lower() != chr(cp)],
    "space": [cp for cp in range(0x110000) if not 0xD800 <= cp <= 0xDFFF and chr(cp).isspace()],
})
lower_cases = ["ΑΣ", "ΑΣ ΑΣ", "ΑΣ.", "ΑΣ'", "'ΑΣ", "ΑΣΑ", "Σ", "ΑΣ́", "ΑΣ́Α", "İ", "ǅ", "ẞ", "ΑΒΓΣ,ΑΒΓΣ", "A’Σ", "ΧΑΟΣ ΧΑΟΣΑ"]
dump("lower.json", [[s, s.lower()] for s in lower_cases])

# ---- numbers --------------------------------------------------------------------------------------------
vals = []
for _ in range(80):
    n = rnd.choice([0, 1, 2, 3, 5, 8, 20, 130, 300, 1000])
    scale = rnd.choice([1, 1e-3, 1e8, 1e-12])
    vals.append([rnd.uniform(-1, 1) * scale * rnd.choice([1, 1, 1e6, 1e-6]) for _ in range(n)])
vals += [[1e100, 1.0, -1e100], [0.1] * 10, [1e16, 1.0, 1.0, -1e16], [-0.0], [0.5, -0.5]]
sums = []
for v in vals:
    sums.append({"values": [repr(x) for x in v], "py_sum": repr(float(sum(v))) if v else "0.0",
                 "np_sum": repr(float(np.array(v, dtype=float).sum())) if v else "0.0"})
dump("sums.json", sums)
cos = []
for _ in range(120):
    dim = rnd.choice([1, 2, 3, 5, 16])
    a = [round(rnd.uniform(-2, 2), rnd.choice([1, 3, 8])) for _ in range(dim)]
    b = [round(rnd.uniform(-2, 2), rnd.choice([1, 3, 8])) for _ in range(rnd.choice([dim, dim, max(1, dim - 1), dim + 1]))]
    if rnd.random() < 0.1:
        b = [0.0] * len(b)
    cos.append({"a": [repr(x) for x in a], "b": [repr(x) for x in b], "cosine": repr(float(rk.cosine(a, b)))})
dump("cosine.json", cos)

# ---- snippets -------------------------------------------------------------------------------------------


def para(n=None):
    n = n or rnd.randint(3, 25)
    out = []
    for i in range(n):
        out.append(rnd.choice(WORDS + STOP))
        if rnd.random() < 0.15:
            out[-1] += rnd.choice([".", "!", "?", ".", ","])
    t = " ".join(out)
    return t[0].upper() + t[1:]


def doc():
    parts = []
    if rnd.random() < 0.5:
        parts.append("---\ntitle: " + para(2) + "\n" + rnd.choice(["", "tags: [a]\n"]) + "---")
    for _ in range(rnd.randint(1, 6)):
        r = rnd.random()
        if r < 0.3:
            parts.append("#" * rnd.randint(1, 3) + " " + para(2))
        if r > 0.92:
            parts.append(rnd.choice(["Für künftige Sessions", "Fuer kuenftige Sessions"]) + ": " + para(5))
        body = para(rnd.choice([5, 10, 40, 90]))
        if rnd.random() < 0.3:
            body += "\n" + para(6)
        if rnd.random() < 0.15:
            body = body.replace(" ", "  ", 2).replace(".", ".  ", 1)
        parts.append(body)
    sep = rnd.choice(["\n\n", "\n\n", "\n \n", "\n\t\n"])
    text = sep.join(parts)
    if rnd.random() < 0.1:
        text = text.replace("\n", " ", 1)
    return text + rnd.choice(["", "\n", "\n\n"])


snips = []
fixed = [("", "x"), ("---\ntitle: a\n", "a"), ("---\n---\n", "a"), ("# Nur Kopf\n", "kopf"), ("\n\n\n", "q"), ("Satz eins. Satz zwei! Satz drei? Satz vier. Satz fünf.", "satz"),
         ("a" * 300 + " b", "a"), ("wort " * 100, "wort"), ("lang " + "x" * 400, "lang"), ("Für künftige Sessions: hier\nzweite Zeile", "zweite"), ("x\ry\r\nz", "z")]
for t, q in fixed:
    snips.append((t, q))
for _ in range(220):
    t = doc()
    words = [w for w in t.lower().split() if w.isalpha()]
    qr = rnd.random()
    if qr < 0.6 and words:
        q = " ".join(rnd.choice(words) for _ in range(rnd.randint(1, 3)))
    elif qr < 0.75:
        q = " ".join(rnd.choice(STOP) for _ in range(rnd.randint(1, 3)))
    elif qr < 0.85:
        q = ""
    else:
        q = rnd.choice(WORDS) + " " + rnd.choice(WORDS)
    snips.append((t, q))
dump("snippets.json", [{"text": t, "query": q, "best": [rk.best_snippet_span(t, q)[0], rk.best_snippet_span(t, q)[1]],
                         "hook": [rk.hook_snippet_span(t)[0], rk.hook_snippet_span(t)[1]]} for t, q in snips])

# ---- BM25 on invented vaults ----------------------------------------------------------------------------
bm = []
for case in range(40):
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        files = {}
        for i in range(rnd.randint(1, 14)):
            rel = rnd.choice(["a", "b/c", "d"]) + "/" + rnd.choice(WORDS) + str(i) + ".md"
            fm = ""
            lines = []
            if rnd.random() < 0.6:
                lines.append("title: " + para(rnd.randint(1, 3)).title())
            if rnd.random() < (0.3 if case % 2 else 0.0):
                lines.append("class: derived")
            if lines:
                fm = "---\n" + "\n".join(lines) + "\n---\n"
            files[rel] = fm + doc()
            p = root / rel
            p.parent.mkdir(parents=True, exist_ok=True)
            p.write_text(files[rel], encoding="utf-8")
        notes = vm.load_search_notes(root)
        korpus = rk.LexKorpus(notes)
        queries = []
        for _ in range(4):
            words = [w for t in files.values() for w in t.lower().split() if w.isalpha()] or WORDS
            q = " ".join(rnd.choice(words) for _ in range(rnd.randint(1, 4)))
            for k in (1, 3, 20):
                h = korpus.hits(q, k, snippets=False)
                queries.append({"query": q, "k": k, "scores": [repr(float(s)) for s in korpus.scores(q)],
                                "hits": [[x.rel, repr(float(x.score)), x.bm25_rang, repr(float(x.bm25_norm)), x.derived, x.title] for x in h]})
        bm.append({"files": files, "rels": [n.rel for n in notes], "queries": queries})
dump("bm25.json", bm)

# ---- fusion ---------------------------------------------------------------------------------------------
fuse = []
NAV = ["wo faengt es an", "Stand von x", "Übersicht", "nichts besonderes", "status vom Garten", " moc", "themenhub"]
for _ in range(60):
    rels = [rnd.choice(["garten", "x", "y/z", "20-projects/p"]) + "/" + rnd.choice(["MOC.md", "STATUS.md", "n1.md", "n2.md", "n3.md", "n4.md", "n5.md", "n6.md"]) + "" for _ in range(rnd.randint(2, 9))]
    rels = list(dict.fromkeys(rels))
    sem, txt = [], []
    for r in rnd.sample(rels, rnd.randint(0, len(rels))):
        h = rk.SearchHit(rel=r, score=round(rnd.uniform(0.01, 1), 3), title=rnd.choice(["", "T " + r]), match="semantic", derived=rnd.random() < 0.25)
        h.cosine = h.score
        sem.append(h)
    for i, h in enumerate(sem, 1):
        h.vektor_rang = i
    for r in rnd.sample(rels, rnd.randint(0, len(rels))):
        h = rk.SearchHit(rel=r, score=round(rnd.uniform(0.01, 3), 3), title=rnd.choice(["", "T " + r]), match="text", derived=rnd.random() < 0.25)
        txt.append(h)
    for i, h in enumerate(txt, 1):
        h.bm25_rang = i
        h.bm25_norm = h.score
    q = rnd.choice(NAV)
    k = rnd.choice([1, 2, 3, 5, 20])

    def hj(h):
        return {"rel": h.rel, "score": repr(h.score), "title": h.title, "matched": h.match, "cosine": repr(h.cosine), "bm25_rang": h.bm25_rang,
                "bm25_norm": repr(h.bm25_norm), "vektor_rang": h.vektor_rang, "derived": h.derived}
    inp_s = [hj(h) for h in sem]
    inp_t = [hj(h) for h in txt]
    out = rk.fuse_hybrid(sem, txt, q, k)
    fuse.append({"semantic": inp_s, "text": inp_t, "query": q, "k": k,
                 "out": [hj(h) for h in out]})
dump("fuse.json", fuse)

# ---- validity -------------------------------------------------------------------------------------------
val = []
FIELDS = {
    "status": ["", "active", "superseded", "archived", "expired", "Superseded", " archived ", "draft", [], ["superseded"]],
    "superseded-by": ["[[x]]", "x", "[[x|y]]", "[[x#h]]", "[[a]] und [[b]]", "", [], ["[[x]]", "[[y]]"], ["x", "x"], "[[ ]]", "[[[x]]", "y [[z"],
    "valid-until": ["2026-09", "2026-10", "2026-11-17", "2026-11-18", "2026-11-19", "2026", "2025", "2027-01", "2026-1", "morgen", "", [], ["2020"], "2026-11-18 "],
}
for _ in range(220):
    fm = {}
    for key, options in FIELDS.items():
        if rnd.random() < 0.5:
            fm[key] = rnd.choice(options)
    if rnd.random() < 0.1:
        fm = {}
    today = rnd.choice(["2026-11-18", "2026-09-30", "2026-10-01", "2025-12-31", "2026-11-17"])
    r = rk.frontmatter_validity(fm, today)
    val.append({"fields": fm, "today": today, "out": None if r is None else [r[0], r[1], r[2]]})
dump("validity.json", val)
links = []
for v in FIELDS["superseded-by"] + [None]:
    links.append({"value": v, "out": rk.link_targets(v)})
dump("links.json", links)
print("ok")
