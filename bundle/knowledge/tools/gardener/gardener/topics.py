"""Topic hubs (30-topics/<t>/MOC.md).

A topic hub links notes that live in OTHER branches, so "membership" cannot come
from the folder. It comes from embeddings: notes close to the hub's own text are
listed in the gardener block. The curated part above the block stays human.

Second job: notes that cluster tightly together but have no hub yet. Seit
2026-07-29 werden sie NICHT mehr als Vorschlag in die Review-Queue gelegt -
Themenbereiche wachsen automatisch (der Nutzer), die Synthese-Phase legt die
Seite selbst an. Ein Cluster braucht `config.CLUSTER_MIN_SIZE` Notizen, was zu
`config.SYNTH_MIN_SOURCES` passen muss, sonst wird gefunden, was nie gebaut
werden darf.

MOC.md is excluded from the linking corpus, so hubs are loaded (and embedded)
separately via load_hubs().
"""
from __future__ import annotations

import datetime as dt
import logging
import math
import re
from dataclasses import dataclass, field
from pathlib import Path

from . import blocks, config
from .linking import cosine
from .maintain import MOC_END, MOC_START, first_hook_line
from .queue import ReviewQueue
from .vault import Note, VaultWriter, parse_note

log = logging.getLogger("gardener")

AUTO_HEADING = "## Weitere Notes zum Thema (gardener)"

# Anteil eines Clusters, den ein bestehender Hub schon verlinken muss,
# damit der Cluster als abgedeckt gilt (siehe _covered).
COVERED_SHARE = 0.6

# words that never make a topic name (note-type words, dates, vault jargon)
STOPWORDS = {
    "session", "sessions", "note", "notes", "moc", "index", "overview",
    "plan", "brief", "state", "report", "readme", "setup", "brain", "vault",
    "todo", "draft", "info", "check", "checklist", "update", "final",
}


@dataclass
class TopicResult:
    mocs_updated: list[str] = field(default_factory=list)
    hubs_suggested: list[tuple[str, list[str]]] = field(default_factory=list)


def load_hubs(vault: Path) -> list[Note]:
    base = vault / "30-topics"
    if not base.is_dir():
        return []
    return [parse_note(vault, p / "MOC.md") for p in sorted(base.iterdir())
            if p.is_dir() and (p / "MOC.md").exists()]


def hub_members(hub: Note, notes: list[Note], vectors: dict[str, list[float]],
                min_sim: float = config.TOPIC_MEMBER_SIM,
                limit: int = config.TOPIC_MAX_AUTO_MEMBERS) -> list[Note]:
    """Notes semantically close to the hub, excluding the ones it already links."""
    if hub.rel not in vectors:
        return []
    scored = []
    for n in notes:
        if n.rel == hub.rel or n.rel not in vectors:
            continue
        if n.keys & hub.links:          # already curated by hand
            continue
        if n.ntype in ("report", "asset"):
            continue
        sim = cosine(vectors[hub.rel], vectors[n.rel])
        if sim >= min_sim:
            scored.append((sim, n))
    scored.sort(key=lambda t: -t[0])
    return [n for _s, n in scored[:limit]]


def render_block(members: list[Note]) -> str:
    listing = "\n".join(f"- [[{n.title}]] - {first_hook_line(n)}"
                        for n in members) or "- keine"
    return f"{MOC_START}\n{listing}\n{MOC_END}"


def update_topic_mocs(hubs: list[Note], notes: list[Note],
                      vectors: dict[str, list[float]],
                      writer: VaultWriter) -> list[str]:
    updated = []
    for hub in hubs:
        if hub.rel not in vectors:
            continue
        block = render_block(hub_members(hub, notes, vectors))
        old = hub.text
        new, ok = blocks.replace_block(old, MOC_START, MOC_END, block)
        if not ok:
            log.warning("malformed gardener block in %s - not rewriting it", hub.rel)
            continue
        if new == old:
            if MOC_START in old:
                continue                        # block already current
            new = old.rstrip("\n") + f"\n\n{AUTO_HEADING}\n{block}\n"
        if not writer.write(hub.path, new, expect=old):
            continue
        hub.text = new
        updated.append(hub.rel)
    return updated


def _covered(cluster: list[Note], hubs: list[Note]) -> bool:
    """True if an existing hub already links a clear majority of the cluster.

    Frueher: "fast alle" (len-1). Zu streng, um Synonym-Themen zu verhindern -
    am 2026-07-29 entstand `30-topics/quassel/` neben dem bestehenden
    `30-topics/voxtype/`, obwohl drei der fuenf Quellnotizen dieselben waren:
    die App heisst another service, der Projektordner voxtype, und der Namensvorschlag
    kannte den zweiten Namen nicht. Ein Cluster, dessen Mehrheit schon unter
    einem Hub haengt, ist kein neues Thema.
    """
    for hub in hubs:
        hit = sum(1 for n in cluster if n.keys & hub.links)
        if hit >= max(2, math.ceil(len(cluster) * COVERED_SHARE)):
            return True
    return False


def suggest_name(cluster: list[Note], taken: set[str]) -> str | None:
    """Most common meaningful word across the titles. None when the cluster has
    no name worth proposing (only generic words, or an existing branch name) -
    an unnameable cluster is not actionable, so it is not suggested at all."""
    words: dict[str, int] = {}
    for n in cluster:
        for w in re.findall(r"[A-Za-zÄÖÜäöüß][A-Za-zÄÖÜäöüß0-9-]{3,}", n.title.lower()):
            if w in STOPWORDS or w in taken:
                continue
            words[w] = words.get(w, 0) + 1
    if not words:
        return None
    word, count = max(words.items(), key=lambda kv: (kv[1], -len(kv[0])))
    return word if count >= 2 else None


def suggest_hubs(hubs: list[Note], notes: list[Note],
                 vectors: dict[str, list[float]],
                 min_sim: float = config.CLUSTER_MIN_SIM,
                 min_size: int = config.CLUSTER_MIN_SIZE,
                 limit: int = config.MAX_HUB_SUGGESTIONS,
                 taken: set[str] | None = None,
                 ) -> list[tuple[str, list[Note]]]:
    """Disjoint clusters of >= min_size mutually close notes that no hub covers.

    Greedy: biggest cluster first, its members are then off the table, so one
    theme cannot produce a dozen overlapping suggestions. Session notes and
    reports are archives, not themes, and stay out of the pool.
    """
    taken = taken or set()
    pool = [n for n in notes
            if n.rel in vectors and n.ntype not in ("report", "asset", "session")]
    clusters = []
    for seed in pool:
        members = [seed] + [n for n in pool
                            if n.rel != seed.rel
                            and cosine(vectors[seed.rel], vectors[n.rel]) >= min_sim]
        if len(members) >= min_size:
            clusters.append(members)
    clusters.sort(key=len, reverse=True)

    assigned: set[str] = set()
    out: list[tuple[str, list[Note]]] = []
    for members in clusters:
        if len(out) >= limit:
            break
        fresh = [n for n in members if n.rel not in assigned]
        if len(fresh) < min_size or _covered(fresh, hubs):
            continue
        name = suggest_name(fresh, taken)
        if name is None:
            continue
        assigned |= {n.rel for n in fresh}
        taken.add(name)
        out.append((name, fresh))
    return out


def existing_branch_names(vault: Path) -> set[str]:
    """Project and topic folder names: a hub named after one of them is noise."""
    names = set()
    for top in ("20-projects", "30-topics"):
        base = vault / top
        if base.is_dir():
            names |= {d.name.lower() for d in base.iterdir() if d.is_dir()}
    return names


def run_topics(hubs: list[Note], notes: list[Note],
               vectors: dict[str, list[float]], writer: VaultWriter,
               queue: ReviewQueue, today: dt.date | None = None) -> TopicResult:
    result = TopicResult()
    result.mocs_updated = update_topic_mocs(hubs, notes, vectors, writer)
    taken = existing_branch_names(writer.vault)
    # Gefundene Cluster werden NICHT mehr als Vorschlag in die Review-Queue
    # gelegt (der Nutzer, 2026-07-29): Themenbereiche sollen automatisch wachsen,
    # nicht auf eine Freigabe warten. Die Synthese-Phase legt die Seite selbst
    # an - `discover_candidates` sieht dieselben Cluster, seit sie ueber den
    # ganzen Vault gebildet werden. Hier bleibt nur die Meldung fuer den Bericht,
    # damit im Report steht, was in diesem Lauf neu entstanden ist.
    for name, cluster in suggest_hubs(hubs, notes, vectors, taken=taken):
        result.hubs_suggested.append((name, [n.rel for n in cluster]))
    return result
