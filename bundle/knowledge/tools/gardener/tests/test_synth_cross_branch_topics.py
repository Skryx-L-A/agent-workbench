"""Ein Thema liegt quer - sonst braucht es keinen Hub.

`discover_candidates` bildete Cluster-Themen nur aus `10-global/`. Der
Vorschlagsmechanismus dagegen suchte im ganzen Vault, fand `worker`
(10-global + claude-workbench), `shader` (mined + lumenpt) und `status`
(acht Branches) - und die konnten hier nie entstehen. Ergebnis: dieselben drei
Vorschlaege standen Lauf fuer Lauf in der Review-Queue, ohne dass je eine Seite
daraus wurde (gemessen 2026-07-29).
"""
from __future__ import annotations

from pathlib import Path

from gardener import synth
from gardener.vault import parse_note


def _note(vault: Path, rel: str, title: str, body: str) -> None:
    p = vault / rel
    p.parent.mkdir(parents=True, exist_ok=True)
    p.write_text(f"---\ntitle: {title}\nclass: knowledge\n---\n\n{body}\n",
                 encoding="utf-8")


def _vec(seed: float) -> list[float]:
    return [seed, 1.0 - seed, 0.0]


def _setup(tmp_path: Path):
    """Vier eng verwandte Notizen, verteilt ueber drei Branches."""
    # Der Namensvorschlag zaehlt WOERTER in Titeln, und der Wortfilter erfasst
    # "worker-tiling-fix" als ein Wort - "worker" muss also allein vorkommen,
    # wie in den echten Titeln, aus denen der Vorschlag `worker` entstand.
    rels = [
        ("10-global/prozess-hygiene.md", "Prozess-Hygiene fuer Orchestrator und Worker"),
        ("20-projects/claude-workbench/tiling-fix.md", "Worker im Spawner richtig kacheln"),
        ("00-sources/mined/mined-worker-sichtbar.md", "Laufende Worker waren unsichtbar"),
        ("10-global/peer-orchestration.md", "Peer-Rechner-Orchestrierung sichtbar steuern"),
    ]
    for rel, title in rels:
        _note(tmp_path, rel, title, f"Text zu {title}.")
    notes = [parse_note(tmp_path, tmp_path / rel) for rel, _ in rels]
    vectors = {n.rel: _vec(0.5) for n in notes}   # alle identisch -> ein Cluster
    return notes, vectors


def test_a_cluster_across_branches_becomes_a_candidate(tmp_path: Path):
    notes, vectors = _setup(tmp_path)

    cands = synth.discover_candidates(tmp_path, notes, [], vectors)

    cluster = [c for c in cands if len(c.sources) >= 3]
    assert cluster, "kein branchenuebergreifender Kandidat entstanden"
    branches = {s.rel.split("/")[0] for s in cluster[0].sources}
    assert len(branches) > 1, f"Kandidat blieb in einem Branch: {branches}"


def test_an_existing_hub_page_is_not_its_own_source(tmp_path: Path):
    """30-topics/ selbst gehoert nicht in den Cluster-Pool."""
    notes, vectors = _setup(tmp_path)
    _note(tmp_path, "30-topics/worker/MOC.md", "worker MOC", "Hub-Text.")
    hub = parse_note(tmp_path, tmp_path / "30-topics/worker/MOC.md")
    notes.append(hub)
    vectors[hub.rel] = _vec(0.5)

    cands = synth.discover_candidates(tmp_path, notes, [], vectors)

    for c in cands:
        assert not any(s.rel.startswith("30-topics/") for s in c.sources), \
            "eine Hub-Seite wurde als eigene Quelle gezaehlt"
