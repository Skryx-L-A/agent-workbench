"""Entity lint: the checks that keep the vault honest.

Dead wikilinks, stale `Stand:` markers, expired `review-after:` TTLs, orphans,
dead asset paths, MOC completeness, oversized notes and duplicate frontmatter
blocks (Basic-Memory prepends its own). Findings land in the health report; the
actionable ones additionally in the review-queue.
"""
from __future__ import annotations

import datetime as dt
import logging
import re
from dataclasses import dataclass
from pathlib import Path

from . import config, frontmatter
from .audit import dead_links, stale_claims
from .heat import cold_notes
from .ingest import asset_file
from .maintain import find_orphans
from .queue import ReviewQueue
from .vault import Note, WIKILINK_RE, key_of, read_text

log = logging.getLogger("gardener")

MONTH_RE = re.compile(r"^\d{4}-\d{2}$")

# kinds that are worth bothering der Nutzer with directly. cold-note is NOT in
# here: maintenance queues those (capped), the report lists them all.
QUEUE_KINDS = ("dead-asset-path", "review-after-expired")


@dataclass(frozen=True)
class Finding:
    kind: str
    rel: str
    detail: str


def _month_index(value: str) -> int | None:
    if not MONTH_RE.match(value):
        return None
    y, m = value.split("-")
    return int(y) * 12 + int(m)


def expired_reviews(notes: list[Note], today: dt.date | None = None) -> list[Finding]:
    today = today or dt.date.today()
    now = today.year * 12 + today.month
    out = []
    for n in notes:
        raw = str(n.fm.get("review-after") or "").strip()
        idx = _month_index(raw)
        if idx is not None and idx <= now:
            out.append(Finding("review-after-expired", n.rel,
                               f"review-after: {raw} ist erreicht/ueberschritten"))
    return out


def dead_asset_paths(vault: Path, notes: list[Note]) -> list[Finding]:
    out = []
    for n in notes:
        if n.ntype != "asset":
            continue
        target = asset_file(vault, n)
        if target is None:
            out.append(Finding("dead-asset-path", n.rel, "kein `path:` im Frontmatter"))
        elif not target.exists():
            out.append(Finding("dead-asset-path", n.rel,
                               f"Datei fehlt: {n.fm.get('path')}"))
    return out


def moc_gaps(vault: Path, notes: list[Note]) -> list[Finding]:
    """Notes of a project branch that its MOC does not link (directly)."""
    by_project: dict[str, list[Note]] = {}
    for n in notes:
        parts = Path(n.rel).parts
        if len(parts) >= 3 and parts[0] == "20-projects":
            by_project.setdefault(parts[1], []).append(n)
    out = []
    for proj, proj_notes in sorted(by_project.items()):
        moc = vault / "20-projects" / proj / "MOC.md"
        if not moc.exists():
            out.append(Finding("moc-missing", f"20-projects/{proj}",
                               "Projekt hat keine MOC.md"))
            continue
        links = {key_of(t) for t in WIKILINK_RE.findall(read_text(moc))}
        for n in sorted(proj_notes, key=lambda x: x.rel):
            if not (n.keys & links):
                out.append(Finding("moc-gap", n.rel,
                                   f"nicht von 20-projects/{proj}/MOC.md verlinkt"))
    return out


def token_estimate(text: str) -> int:
    _fm, body = frontmatter.parse(text)
    return int(len(body.split()) * config.TOKENS_PER_WORD)


def oversized(notes: list[Note],
              max_tokens: int = config.NOTE_MAX_TOKENS) -> list[Finding]:
    out = []
    for n in notes:
        if n.ntype in ("session", "report"):   # archives may be long
            continue
        est = token_estimate(n.text)
        if est > max_tokens:
            out.append(Finding("oversized", n.rel,
                               f"~{est} Tokens (> {max_tokens}) - Split vorschlagen: "
                               "Kernaussagen in Themen-Note, Details in Unter-Note"))
    return out


def duplicate_frontmatter(notes: list[Note]) -> list[Finding]:
    out = []
    for n in notes:
        blocks, _body = frontmatter.split_blocks(n.text)
        if len(blocks) > 1:
            out.append(Finding("duplicate-frontmatter", n.rel,
                               f"{len(blocks)} Frontmatter-Bloecke (Basic-Memory-Sync); "
                               "Felder werden gemerged gelesen"))
    return out


def run_lint(vault: Path, notes: list[Note], heat: dict | None = None,
             today: dt.date | None = None) -> list[Finding]:
    today = today or dt.date.today()
    findings: list[Finding] = []
    findings += [Finding("dead-link", rel, f"[[{target}]] zeigt ins Leere")
                 for rel, target in dead_links(notes, vault)]
    findings += [Finding("stale-stand", rel,
                         f"Stand: {stamp} ist aelter als "
                         f"{config.STALE_MARKER_MONTHS} Monate")
                 for rel, stamp in stale_claims(notes, today=today)]
    findings += expired_reviews(notes, today)
    findings += [Finding("orphan", n.rel, "keine Links ein/aus")
                 for n in find_orphans(notes)]
    findings += dead_asset_paths(vault, notes)
    findings += moc_gaps(vault, notes)
    findings += oversized(notes)
    findings += duplicate_frontmatter(notes)
    findings += [Finding("cold-note", n.rel,
                         f"seit >{config.COLD_MONTHS} Monaten nicht gelesen")
                 for n in cold_notes(notes, heat or {})]
    return findings


def queue_findings(findings: list[Finding], queue: ReviewQueue,
                   today: dt.date | None = None) -> list[Finding]:
    queued = []
    for f in findings:
        if f.kind not in QUEUE_KINDS:
            continue
        if queue.add(f"Lint [{f.kind}] {f.rel}: {f.detail}",
                     key=f"Lint [{f.kind}] {f.rel}", today=today):
            queued.append(f)
    return queued
