"""Topic synthesis (Brain 4.x): fully-regenerated `30-topics/<t>/MOC.md` pages.

Distinct from topics.py, which appends a "further notes" block below a
hand-curated hub. A synth page is `class: derived`: the gardener owns the
whole file, writes it end to end, and treats it as disposable - regenerable
any time from its sources, never hand-edited.

A topic page is a COMPRESSION of its sources. In a human wiki a skewed summary
is merely annoying; here it is AUTHORITATIVE, because the recall ladder in
CLAUDE.md tells every agent to read a topic page first. So every claim this
module writes is enforced in code, not just asked of the model:

1. No claim without a source: a generated line without a `[[wikilink]]` is
   dropped, never written (`filter_sourced_lines`).
2. Only real targets: a wikilink that does not resolve to one of the topic's
   own source notes is dropped too - the model cannot cite a title it invented
   or borrow one from an unrelated note.
3. `class: derived` plus a content-hash marker: a page is regenerated only
   when its sources changed, and never overwritten once a human has edited it
   (hash mismatch is reported, not silently clobbered).
4. No page below `config.SYNTH_MIN_SOURCES` source notes.
"""
from __future__ import annotations

import datetime as dt
import hashlib
import logging
import re
from dataclasses import dataclass, field
from pathlib import Path

from . import config, frontmatter
from . import topics as topics_mod
from .contradict import ContradictionStore
from .linking import cosine
from .ollama import OllamaError
from .vault import Note, VaultWriter, build_resolver, key_of, read_text

log = logging.getLogger("gardener")

# A complete, well-formed wikilink only - same shape as maintain.WIKILINK_INLINE_RE.
WIKILINK_RE = re.compile(r"\[\[([^\]|#\n]+)(?:\|[^\]\n]*)?\]\]")

SYNTH_SYSTEM = (
    "You maintain a personal markdown knowledge vault. You write a short topic "
    "summary page ('Themenseite') from a fixed set of source notes about ONE "
    "topic. Answer ONLY with a JSON object: "
    '{"intro": {"text": "<one or two sentences saying what this topic is>", '
    '"source": "<exact note title>"}, '
    '"facts": [{"text": "<one concrete, important fact, decision or setup '
    'detail>", "source": "<exact note title this fact comes from>"}, ...]}. '
    "Every `source` MUST be copied EXACTLY from the note titles listed below - "
    "never invent one, never combine two, never leave it empty. Put NOTHING "
    "into `text` except the fact itself: no citation, no wikilink, no date "
    "suffix - the source field carries that. State only what a source "
    "literally supports; when in doubt, leave the fact out. Do not repeat the "
    "same fact twice. Write at most 8 facts, most important first. If two "
    "sources disagree about the same fact, leave that fact out entirely - it "
    "is reported separately."
)

# Warum ein eigenes Feld statt "beende den Satz mit [[Titel]]" (gemessen 2026-07-29):
# Die erste Fassung verlangte den Wikilink IM Satz und erlaubte daneben ein
# optionales "(Stand: YYYY-MM)". ornith:9b behielt durchgaengig die Dekoration und
# liess das Pflichtelement weg - auf drei von zehn Themenseiten wurde deshalb JEDE
# Faktzeile verworfen, obwohl die Fakten selbst dicht und richtig waren. Die
# Prompt-Groesse war es nicht: zasterzentrale hat sechs Quellen, weit unter der
# Grenze von zwanzig, und verlor trotzdem alle zehn Zeilen. Ein separates Feld macht
# aus einer Formatierungsaufgabe eine Auswahl aus einer geschlossenen Liste, und das
# kann ein kleines Modell zuverlaessig.


def _fact_parts(item) -> tuple[str, str]:
    """(text, quelltitel) aus einem Fakt. Akzeptiert beide Formen.

    Neu ist das Objekt {"text": ..., "source": ...}; die alte Form war ein Satz mit
    eingebettetem [[Wikilink]]. Beide bleiben lesbar, damit ein Modell, das doch die
    alte Form liefert, nicht komplett verworfen wird.
    """
    if isinstance(item, dict):
        return str(item.get("text") or "").strip(), str(item.get("source") or "").strip()
    return str(item or "").strip(), ""




@dataclass
class TopicCandidate:
    name: str
    sources: list[Note]


@dataclass
class SynthResult:
    written: list[str] = field(default_factory=list)
    skipped_small: list[str] = field(default_factory=list)
    skipped_hand_edited: list[str] = field(default_factory=list)
    skipped_hand_written: list[str] = field(default_factory=list)
    unchanged: list[str] = field(default_factory=list)
    lines_dropped_no_link: int = 0
    lines_dropped_dead_link: int = 0


# -- candidate topics ---------------------------------------------------------

def discover_candidates(vault: Path, notes: list[Note], hubs: list[Note],
                        vectors: dict[str, list[float]]) -> list[TopicCandidate]:
    """Candidate topics, in priority order - never invented by the model.

    1. Existing `30-topics/<name>/` hubs: membership by embedding similarity to
       the hub's own text (the same signal topics.hub_members uses).
    2. `20-projects/<p>/` branches at or above the size gate: folder membership,
       no embeddings needed.
    3. Wikilink/embedding clusters inside `10-global/` with no hub yet, reusing
       topics.suggest_hubs (no new clustering mechanism).
    """
    out: list[TopicCandidate] = []
    taken = topics_mod.existing_branch_names(vault)

    for hub in hubs:
        if hub.rel not in vectors:
            continue
        name = Path(hub.rel).parts[1]
        sources = [n for n in notes
                   if n.rel in vectors and n.ntype not in ("report", "asset")
                   and cosine(vectors[hub.rel], vectors[n.rel]) >= config.TOPIC_MEMBER_SIM]
        out.append(TopicCandidate(name, sources))

    claimed = {c.name for c in out}
    projects: dict[str, list[Note]] = {}
    for n in notes:
        parts = Path(n.rel).parts
        if len(parts) >= 2 and parts[0] == "20-projects":
            projects.setdefault(parts[1], []).append(n)
    for proj, proj_notes in sorted(projects.items()):
        if proj in claimed:
            continue
        out.append(TopicCandidate(proj, proj_notes))
        claimed.add(proj)

    # Cluster ueber den GANZEN Vault, nicht nur ueber 10-global. Ein Thema ist
    # per Definition querliegend ("cross-project theme hub", INDEX.md) - die
    # Beschraenkung auf einen Branch schloss genau die Faelle aus, fuer die Hubs
    # existieren. Gemessen 2026-07-29: `worker` (10-global + claude-workbench),
    # `shader` (mined + lumenpt) und `status` (acht Branches) wurden vom
    # Vorschlagsmechanismus gefunden, konnten hier aber nie entstehen und landeten
    # Lauf fuer Lauf als Vorschlag in der Review-Queue.
    # Eine Notiz wandert dabei nicht in den Themen-Branch, sie wird nur von dort
    # verlinkt - sie darf also zugleich in ihrem Projekt-Hub stehen.
    cluster_pool = [n for n in notes if not n.rel.startswith("30-topics/")]
    for name, cluster in topics_mod.suggest_hubs(
            hubs, cluster_pool, vectors,
            min_sim=config.CLUSTER_MIN_SIM, min_size=config.CLUSTER_MIN_SIZE,
            limit=config.SYNTH_MAX_CLUSTER_CANDIDATES, taken=taken):
        if name in claimed:
            continue
        out.append(TopicCandidate(name, cluster))
        claimed.add(name)

    return out


# -- code-enforced sourcing ----------------------------------------------------

def filter_sourced_lines(lines: list[str],
                         resolver: dict[str, Note]) -> tuple[list[str], int, int]:
    """(kept, dropped_no_link, dropped_dead_link).

    A line survives only if it carries at least one wikilink AND every
    wikilink on it resolves in `resolver`. This is the code check rule 1/2
    require - never a prompt, never trusted from the model's own claim.
    """
    kept: list[str] = []
    no_link = 0
    dead_link = 0
    for raw in lines:
        line = str(raw).strip()
        if not line:
            continue
        targets = WIKILINK_RE.findall(line)
        if not targets:
            no_link += 1
            continue
        if not all(key_of(t) in resolver for t in targets):
            dead_link += 1
            continue
        kept.append(line)
    return kept, no_link, dead_link


def sources_hash(sources: list[Note]) -> str:
    parts = sorted(f"{n.rel}:{n.content_hash}" for n in sources)
    return hashlib.sha256("|".join(parts).encode()).hexdigest()


# -- the judge call -------------------------------------------------------------

def build_prompt(cand: TopicCandidate) -> str:
    sources = sorted(cand.sources, key=lambda n: n.rel)[:config.SYNTH_MAX_SOURCES_IN_PROMPT]
    parts = [f"Topic name: {cand.name}", "Source notes (title, then excerpt):"]
    for n in sources:
        parts.append(f"### {n.title}\n{n.text[:config.SYNTH_MAX_SOURCE_CHARS]}")
    parts.append("Write the topic page JSON now.")
    return "\n\n".join(parts)


def synthesize(client, cand: TopicCandidate) -> tuple[str | None, list[str], int, int]:
    """(intro_or_None, fact_lines, dropped_no_link, dropped_dead_link).

    The resolver is scoped to THIS topic's own sources: a wikilink to a real
    but unrelated note is rejected exactly like one to a note that does not
    exist at all - citing something outside the given source set is not
    something the model gets to do.
    """
    resolver = build_resolver(cand.sources)
    verdict = client.judge(SYNTH_SYSTEM, build_prompt(cand))

    def compose(item) -> str:
        """Aus {"text","source"} eine Zeile mit angehaengtem Wikilink bauen.

        Der Link wird HIER gesetzt, nicht vom Modell geschrieben: das Modell nennt
        nur den Titel, und ob der zulaessig ist, entscheidet gleich der Resolver.
        Kommt die alte Form (Satz mit eingebettetem Link), bleibt sie unveraendert.
        """
        text, source = _fact_parts(item)
        if not text:
            return ""
        if not source:
            return text                      # alte Form oder fehlende Quelle
        if "[[" in text:                     # Modell hat den Link doch eingebaut
            return text
        return f"{text} [[{source}]]"

    intro_raw = compose(verdict.get("intro"))
    facts_in = verdict.get("facts")
    facts_raw = [compose(f) for f in facts_in] if isinstance(facts_in, list) else []
    facts_raw = [f for f in facts_raw if f]

    intro_kept, intro_no_link, intro_dead = filter_sourced_lines(
        [intro_raw] if intro_raw else [], resolver)
    facts_kept, facts_no_link, facts_dead = filter_sourced_lines(facts_raw, resolver)

    intro = intro_kept[0] if intro_kept else None
    facts = [f if f.startswith("- ") else f"- {f}" for f in facts_kept]
    return intro, facts, intro_no_link + facts_no_link, intro_dead + facts_dead


def contradiction_lines(cand: TopicCandidate, contra_store: ContradictionStore,
                        full_resolver: dict[str, Note]) -> list[str]:
    """Open findings touching one of this topic's sources - called out, never
    quietly resolved into one side of the story."""
    rels = {n.rel for n in cand.sources}
    raw = []
    for f in contra_store.open_findings():
        a, b = f["note_a"], f["note_b"]
        if a["rel"] not in rels and b["rel"] not in rels:
            continue
        tag = "ESKALIERT" if f["status"] == "escalated" else str(f.get("verdict", "")).upper()
        raw.append(f'- [{tag}] [[{a["title"]}]] vs [[{b["title"]}]]: '
                   f'"{a["quote"]}" <-> "{b["quote"]}" (Konfidenz {f["confidence"]:.2f})')
    kept, _no_link, _dead = filter_sourced_lines(raw, full_resolver)
    return kept


# -- rendering ------------------------------------------------------------------

def render_body(cand: TopicCandidate, intro: str | None, facts: list[str],
                contra_lines: list[str]) -> str:
    content = [intro if intro else
               f"Themenseite zu {cand.name}, automatisch aus "
               f"{len(cand.sources)} Quellnotizen erzeugt (gardener synth) - "
               "siehe Quellnotizen unten."]
    content.append("")
    content.append("## Stand")
    content += facts or ["- (noch keine belastbare Zusammenfassung - siehe Quellnotizen)"]
    if contra_lines:
        content.append("")
        content.append("## Offene Widersprueche")
        content += contra_lines
    content.append("")
    content.append("## Quellnotizen")
    content += [f"- [[{n.title}]]"
               for n in sorted(cand.sources, key=lambda n: n.title.lower())]
    return "\n" + "\n".join(content) + "\n"


def strip_foreign_blocks(body: str) -> str:
    """Remove the parts of the page that other phases own.

    The maintain phase appends its own `<!-- gardener:moc:start -->` block to the
    same file AFTER synth wrote the page and stored its hash. Hashing the whole
    body therefore covered someone else's content and every page failed rule 3
    right after a full run -- measured 2026-07-29 on all ten generated pages.
    Same pattern as the shared review queue: a writer only ever hashes, or
    rewrites, its own section.
    """
    from .maintain import MOC_END, MOC_START

    while MOC_START in body:
        head, rest = body.split(MOC_START, 1)
        body = head + (rest.split(MOC_END, 1)[1] if MOC_END in rest else "")
    return body


def body_fingerprint(body: str) -> str:
    """Hash what a reader would call the CONTENT, not the exact bytes.

    The raw-bytes version of this check was broken on delivery (2026-07-29): all
    ten generated pages reported as hand-edited the moment they were written,
    which would have frozen the synthesis layer permanently -- rule 3 refuses to
    overwrite a page it believes a human touched.

    The cause is that this module is not the only writer. The Basic-Memory sync
    stamps a `permalink:` into every note and normalizes the trailing newline
    while doing it; the write gate now stamps `id:`/`schema:` as well. None of
    that is a human editing the page, and a check that cannot tell the two apart
    raises a false alarm on every note, forever.

    So the fingerprint ignores what a normalizer changes -- trailing whitespace
    per line, blank lines at either end -- and nothing else. Change a word,
    a link or a line and it still fires.
    """
    own = strip_foreign_blocks(body)
    lines = [ln.rstrip() for ln in own.strip("\n").splitlines()]
    return hashlib.sha256("\n".join(lines).encode()).hexdigest()


def render_page(cand: TopicCandidate, intro: str | None, facts: list[str],
                contra_lines: list[str], generated_at: dt.datetime,
                src_hash: str) -> str:
    body = render_body(cand, intro, facts, contra_lines)
    content_hash = body_fingerprint(body)
    fields = {
        "title": f"{cand.name} MOC",
        "type": "moc",
        "branch": f"30-topics/{cand.name}",
        "class": "derived",
        "gardener-generated": generated_at.isoformat(timespec="seconds"),
        "gardener-sources-hash": src_hash,
        "gardener-content-hash": content_hash,
    }
    return frontmatter.render(fields) + body


# -- the run --------------------------------------------------------------------

def run_synth(vault: Path, notes: list[Note], hubs: list[Note],
             vectors: dict[str, list[float]], writer: VaultWriter, client,
             contra_store: ContradictionStore, min_sources: int | None = None,
             only_topic: str | None = None,
             today: dt.datetime | None = None) -> SynthResult:
    min_sources = config.SYNTH_MIN_SOURCES if min_sources is None else min_sources
    today = today or dt.datetime.now()
    result = SynthResult()
    full_resolver = build_resolver(notes)

    candidates = discover_candidates(vault, notes, hubs, vectors)
    if only_topic:
        candidates = [c for c in candidates if c.name == only_topic]

    for cand in candidates:
        if len(cand.sources) < min_sources:
            result.skipped_small.append(cand.name)
            continue

        page_path = vault / "30-topics" / cand.name / "MOC.md"
        src_hash = sources_hash(cand.sources)
        existing_text = None
        if page_path.exists():
            existing_text = read_text(page_path)
            existing_fm, existing_body = frontmatter.parse(existing_text)
            # A page a person wrote by hand is not raw material for this phase.
            # The old code treated "no stored hash" as "never synthesized, go
            # ahead", which silently converted the four hand-curated hubs
            # (claude-orchestration, dsgvo-recht, local-models, macos-setup) into
            # generated pages and replaced 114 hand-written lines - measured
            # 2026-07-29, recovered from git. `class: knowledge` is the author
            # saying this page is theirs; only `derived` pages belong to synth.
            # Eigentum muss BELEGT sein, nicht aus fehlender Angabe erschlossen:
            # nur `class: derived` heisst "diese Phase hat die Seite geschrieben".
            # Ein faelschlich uebersprungener Lauf kostet nichts, ein faelschlich
            # ueberschriebener Text ist weg. Eine ganz neue Seite kommt hier nie
            # an - dann existiert die Datei noch nicht.
            if str(existing_fm.get("class") or "").strip() != "derived":
                result.skipped_hand_written.append(cand.name)
                continue
            stored_hash = existing_fm.get("gardener-content-hash")
            if stored_hash:
                actual_hash = body_fingerprint(existing_body)
                if actual_hash != stored_hash:
                    # a page never gets here until it was generated once; a
                    # mismatch now means a human edited it since - see module
                    # docstring rule 3. Report it, never clobber it.
                    result.skipped_hand_edited.append(cand.name)
                    continue
                if existing_fm.get("gardener-sources-hash") == src_hash:
                    result.unchanged.append(cand.name)
                    continue
            # no stored hash yet: first-ever synth of this page - proceed.
            # (A hand-written page never reaches this point; the `class:
            # knowledge` guard above sent it away.)

        try:
            intro, facts, no_link, dead = synthesize(client, cand)
        except OllamaError as e:
            log.warning("synth failed for %s: %s - skipping this run", cand.name, e)
            continue
        result.lines_dropped_no_link += no_link
        result.lines_dropped_dead_link += dead

        contra_lines = contradiction_lines(cand, contra_store, full_resolver)
        text = render_page(cand, intro, facts, contra_lines, today, src_hash)
        if writer.write(page_path, text, expect=existing_text):
            result.written.append(f"30-topics/{cand.name}/MOC.md")
    return result
