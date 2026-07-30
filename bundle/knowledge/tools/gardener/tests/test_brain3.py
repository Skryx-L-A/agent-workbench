import datetime as dt

from gardener import config, heat, indexes, lint, mine, topics
from gardener.maintain import MOC_END, MOC_START, run_maintenance
from gardener.queue import ReviewQueue
from gardener.vault import VaultWriter, load_notes, parse_note

from .conftest import FakeOllama, make_note

TODAY = dt.date(2026, 7, 12)
NOW = dt.datetime(2026, 7, 12, 8, 0)


# -- review queue -------------------------------------------------------

def test_queue_deduplicates_within_and_across_runs(tmp_vault):
    writer = VaultWriter(tmp_vault)
    q1 = ReviewQueue(writer)
    assert q1.add("orphan [[Beta]] - allein", key="orphan [[Beta]]", today=TODAY)
    assert not q1.add("orphan [[Beta]] - immer noch allein", key="orphan [[Beta]]",
                      today=TODAY)
    q2 = ReviewQueue(VaultWriter(tmp_vault))          # next run, file already there
    assert not q2.add("orphan [[Beta]] - allein", key="orphan [[Beta]]", today=TODAY)
    assert (tmp_vault / "review-queue.md").read_text().count("orphan [[Beta]]") == 1


# -- topic hubs ---------------------------------------------------------

def make_hub(vault, name, body="Hub body about local models."):
    p = vault / "30-topics" / name / "MOC.md"
    p.parent.mkdir(parents=True, exist_ok=True)
    p.write_text(f"---\ntitle: {name} MOC\ntype: note\n---\n\n{body}\n")
    return p


def test_topic_moc_gets_gardener_block_with_close_notes(tmp_vault):
    make_hub(tmp_vault, "themen")
    notes = load_notes(tmp_vault)
    hubs = topics.load_hubs(tmp_vault)
    assert [h.rel for h in hubs] == ["30-topics/themen/MOC.md"]
    vectors = {n.rel: [1.0, 0.0] for n in notes + hubs}
    vectors["10-global/beta.md"] = [0.0, 1.0]        # far away -> not a member
    writer = VaultWriter(tmp_vault)
    res = topics.run_topics(hubs, notes, vectors, writer, ReviewQueue(writer), TODAY)
    assert res.mocs_updated == ["30-topics/themen/MOC.md"]
    moc = (tmp_vault / "30-topics/themen/MOC.md").read_text()
    assert MOC_START in moc and MOC_END in moc
    assert "[[Alpha]]" in moc
    assert "[[Beta]]" not in moc


def test_topic_moc_skips_notes_the_hub_already_links(tmp_vault):
    make_hub(tmp_vault, "themen", "Kuratiert:\n- [[Alpha]] - schon von Hand drin\n")
    notes = load_notes(tmp_vault)
    hubs = topics.load_hubs(tmp_vault)
    vectors = {n.rel: [1.0, 0.0] for n in notes + hubs}
    writer = VaultWriter(tmp_vault)
    topics.run_topics(hubs, notes, vectors, writer, ReviewQueue(writer), TODAY)
    block = (tmp_vault / "30-topics/themen/MOC.md").read_text().split(MOC_START)[1]
    bullets = [ln for ln in block.splitlines() if ln.startswith("- [[")]
    assert not any(ln.startswith("- [[Alpha]]") for ln in bullets)  # curated already
    assert any(ln.startswith("- [[Gamma]]") for ln in bullets)


def test_a_cluster_without_hub_is_reported_but_not_queued(tmp_vault):
    """Themenbereiche wachsen automatisch (der Nutzer, 2026-07-29).

    Frueher landete jeder Cluster als Vorschlag in der Review-Queue und wartete
    dort auf eine Freigabe. Die Seite legt jetzt die Synthese-Phase selbst an;
    hier bleibt nur die Meldung fuer den Bericht.
    """
    for i, what in enumerate(("Backup", "Firewall", "DNS", "Router")):
        make_note(tmp_vault, f"10-global/hetzner-{i}.md", f"Hetzner {what}",
                  f"Hetzner {what} Details.")
    notes = load_notes(tmp_vault)
    vectors = {n.rel: [1.0, 0.0] for n in notes}      # everything close: one cluster
    writer = VaultWriter(tmp_vault)
    queue = ReviewQueue(writer)
    res = topics.run_topics([], notes, vectors, writer, queue, TODAY)
    assert [name for name, _ in res.hubs_suggested] == ["hetzner"]
    assert not (tmp_vault / "review-queue.md").exists(), \
        "ein Cluster wurde wieder als Vorschlag in die Queue gelegt"


def test_three_close_notes_are_below_the_cluster_gate(tmp_vault):
    """CLUSTER_MIN_SIZE muss zu SYNTH_MIN_SOURCES passen.

    Bei 3 gefunden und bei 4 verworfen hiess: derselbe Cluster wurde Lauf fuer
    Lauf gemeldet, ohne dass je eine Seite daraus entstehen durfte.
    """
    for i, what in enumerate(("Backup", "Firewall", "DNS")):
        make_note(tmp_vault, f"10-global/hetzner-{i}.md", f"Hetzner {what}", "x")
    notes = load_notes(tmp_vault)
    # Nur die drei sind einander nah - mit identischen Vektoren fuer ALLE waeren
    # die Fixture-Notizen Teil desselben Clusters und die Groesse waere gar nicht
    # geprueft worden.
    vectors = {n.rel: ([1.0, 0.0] if "hetzner" in n.rel else [0.0, 1.0])
               for n in notes}
    found = {name for name, _ in topics.suggest_hubs([], notes, vectors)}
    assert "hetzner" not in found


def test_suggestions_are_disjoint_capped_and_never_generic(tmp_vault):
    # one theme must not produce one suggestion per member, and "session"/date
    # style titles are archives, not themes
    for what in ("Backup", "Firewall", "DNS", "Router"):
        make_note(tmp_vault, f"10-global/hetzner-{what}.md", f"Hetzner {what}", "x")
    for i in range(3):
        p = tmp_vault / f"10-global/session-2026-07-0{i}.md"
        p.write_text(f"---\ntitle: Session 2026-07-0{i}\ntype: session\n---\n\nlog\n")
    notes = load_notes(tmp_vault)
    vectors = {n.rel: [1.0, 0.0] for n in notes}
    found = topics.suggest_hubs([], notes, vectors)
    assert len(found) == 1                      # disjoint: one theme, one suggestion
    name, cluster = found[0]
    assert name == "hetzner"
    assert not any(n.ntype == "session" for n in cluster)
    assert len(found) <= config.MAX_HUB_SUGGESTIONS


def test_no_suggestion_named_after_an_existing_project(tmp_vault):
    for what in ("Backup", "Firewall", "DNS"):
        make_note(tmp_vault, f"10-global/demo-{what}.md", f"Demo {what}", "x")
    notes = load_notes(tmp_vault)
    vectors = {n.rel: [1.0, 0.0] for n in notes}
    # "demo" is an existing 20-projects branch -> a topic hub of that name is noise
    assert topics.suggest_hubs([], notes, vectors,
                               taken=topics.existing_branch_names(tmp_vault)) == []


def test_no_hub_suggestion_when_an_existing_hub_covers_the_cluster(tmp_vault):
    make_hub(tmp_vault, "themen",
             "- [[Alpha]]\n- [[Beta]]\n- [[Gamma]]\n- [[old report]]\n")
    notes = load_notes(tmp_vault)
    hubs = topics.load_hubs(tmp_vault)
    vectors = {n.rel: [1.0, 0.0] for n in notes + hubs}
    writer = VaultWriter(tmp_vault)
    res = topics.run_topics(hubs, notes, vectors, writer, ReviewQueue(writer), TODAY)
    assert res.hubs_suggested == []


# -- read heat / resurfacing --------------------------------------------

def write_heat(vault, lines):
    p = vault / config.HEAT_LOG
    p.parent.mkdir(parents=True, exist_ok=True)
    p.write_text("\n".join(lines) + "\n")
    return p


def test_missing_heat_log_is_not_an_error(tmp_vault):
    assert heat.load_heat(tmp_vault) == {}
    assert heat.cold_notes(load_notes(tmp_vault), {}) == []


def test_heat_log_parsed_and_hot_and_cold_derived(tmp_vault):
    write_heat(tmp_vault, [
        "2026-07-11T10:00:00\t10-global/alpha.md",
        "2026-07-12T07:00:00\t10-global/alpha.md",
        f"2026-07-10T09:00:00\t{tmp_vault / '20-projects/demo/gamma.md'}",  # absolute
        "2025-01-02T09:00:00\t10-global/beta.md",                           # ancient
        "kaputte zeile ohne tab",
    ])
    h = heat.load_heat(tmp_vault)
    assert len(h["10-global/alpha.md"]) == 2
    assert "20-projects/demo/gamma.md" in h
    notes = load_notes(tmp_vault)
    hottest = heat.hottest(notes, h, k=2, now=NOW)
    assert hottest[0][0].rel == "10-global/alpha.md" and hottest[0][1] == 2
    cold = [n.rel for n in heat.cold_notes(notes, h, now=NOW)]
    assert "10-global/beta.md" in cold           # last read 2025
    assert "10-global/alpha.md" not in cold


def test_utc_z_stamps_and_relative_paths_from_the_hook(tmp_vault, monkeypatch):
    # regression: the B4 hook writes UTC 'Z' stamps and vault-RELATIVE paths.
    # Resolving those against the CWD pointed every entry at the wrong note, and
    # the tz-aware stamps blew up every comparison against naive datetimes.
    monkeypatch.chdir(tmp_vault.parent)
    write_heat(tmp_vault, ["2026-07-12T05:41:40Z\t10-global/alpha.md"])
    h = heat.load_heat(tmp_vault)
    assert list(h) == ["10-global/alpha.md"]
    assert h["10-global/alpha.md"][0].tzinfo is None
    notes = load_notes(tmp_vault)
    assert heat.hottest(notes, h, now=NOW)[0][0].rel == "10-global/alpha.md"
    assert "10-global/alpha.md" not in [n.rel for n in heat.cold_notes(notes, h, now=NOW)]


def test_fresh_unread_note_is_not_cold(tmp_vault):
    write_heat(tmp_vault, ["2026-07-12T07:00:00\t10-global/alpha.md"])
    h = heat.load_heat(tmp_vault)
    now = dt.datetime.now()   # tmp files are seconds old -> mtime stands in
    cold = [n.rel for n in heat.cold_notes(load_notes(tmp_vault), h, now=now)]
    assert cold == []


def test_hot_md_has_heat_and_resurfacing_sections(tmp_vault):
    import os
    write_heat(tmp_vault, ["2026-07-12T07:00:00\t10-global/alpha.md"])
    schatz = make_note(tmp_vault, "10-global/schatz.md", "Schatz",
                       "Sehr langer Inhalt. " * 60)
    old = (dt.datetime.now() - dt.timedelta(days=400)).timestamp()
    os.utime(schatz, (old, old))          # long unread AND long untouched
    notes = load_notes(tmp_vault)
    writer = VaultWriter(tmp_vault)
    res = run_maintenance(notes, writer, client=None, today=TODAY)
    hot = (tmp_vault / "HOT.md").read_text()
    assert "## Meistgelesen" in hot
    assert "[[Alpha]] (1x)" in hot
    assert "## Vergessene Schaetze" in hot
    assert "[[Schatz]]" in hot                   # longest never-read note
    assert res.resurfaced
    assert "lange nicht gelesen" in (tmp_vault / "review-queue.md").read_text()


# -- DECISIONS.md / OPEN-QUESTIONS.md -----------------------------------

def test_decisions_index_per_project(tmp_vault):
    p = tmp_vault / "20-projects/demo/entscheidung.md"
    p.write_text("---\ntitle: Entscheidung X\ntype: decision\n---\n\n"
                 "Wir nehmen Astro, weil statisch.\n")
    notes = load_notes(tmp_vault)
    writer = VaultWriter(tmp_vault)
    written = indexes.update_decisions(notes, writer, TODAY)
    assert written == ["20-projects/demo/DECISIONS.md"]
    text = (tmp_vault / "20-projects/demo/DECISIONS.md").read_text()
    assert "[[Entscheidung X]]" in text
    assert "Wir nehmen Astro" in text
    # generated indexes are not part of the corpus
    assert not any(n.rel.endswith("DECISIONS.md") for n in load_notes(tmp_vault))


def test_open_questions_collected_and_dropped_when_answered(tmp_vault):
    p = tmp_vault / "10-global/offen.md"
    p.write_text("---\ntitle: Offen\ntype: note\n---\n\n"
                 "OFFEN: Wer zahlt die Domain?\n"
                 "TODO: AVV unterschreiben\n\n"
                 "## Offene Fragen\n- Brauchen wir LFS?\n\n"
                 "## Anderes\n- kein Fragezeichen hier\n")
    writer = VaultWriter(tmp_vault)
    changed, total = indexes.update_open_questions(load_notes(tmp_vault), writer, TODAY)
    assert changed and total == 3
    text = (tmp_vault / "OPEN-QUESTIONS.md").read_text()
    assert "Wer zahlt die Domain?" in text
    assert "Brauchen wir LFS?" in text
    assert "kein Fragezeichen hier" not in text

    p.write_text("---\ntitle: Offen\ntype: note\n---\n\nAlles geklaert.\n")
    indexes.update_open_questions(load_notes(tmp_vault), VaultWriter(tmp_vault), TODAY)
    text = (tmp_vault / "OPEN-QUESTIONS.md").read_text()
    assert "Wer zahlt die Domain?" not in text   # regenerated, not appended


# -- lint ----------------------------------------------------------------

def test_lint_finds_every_category(tmp_vault):
    # a project MOC that links none of its notes -> moc-gap
    (tmp_vault / "20-projects/demo/MOC.md").write_text(
        "---\ntitle: demo MOC\ntype: note\n---\n\nnoch nichts verlinkt.\n")
    # dead asset path
    (tmp_vault / "20-projects/demo/_assets").mkdir(parents=True)
    (tmp_vault / "20-projects/demo/_assets/ghost.md").write_text(
        "---\ntitle: Ghost\ntype: asset\nbranch: 20-projects/demo\n"
        "path: _assets/weg.pdf\n---\n\nStub ohne Datei. [[Gamma]]\n")
    # expired TTL + oversize + duplicate frontmatter + dead link
    (tmp_vault / "10-global/ttl.md").write_text(
        "---\npermalink: main/x\n---\n\n"
        "---\ntitle: TTL\ntype: note\nreview-after: 2026-01\n---\n\n"
        "[[Nirgendwo]]\n" + "wort " * 700 + "\n")
    notes = load_notes(tmp_vault)
    findings = lint.run_lint(tmp_vault, notes, heat={}, today=TODAY)
    kinds = {f.kind for f in findings}
    assert {"dead-link", "review-after-expired", "orphan", "dead-asset-path",
            "moc-gap", "oversized", "duplicate-frontmatter"} <= kinds
    assert any(f.rel == "20-projects/demo/_assets/ghost.md"
               and f.kind == "dead-asset-path" for f in findings)
    assert any(f.rel == "10-global/ttl.md" and f.kind == "oversized" for f in findings)


def test_lint_queues_only_actionable_kinds(tmp_vault):
    (tmp_vault / "10-global/ttl.md").write_text(
        "---\ntitle: TTL\ntype: note\nreview-after: 2026-01\n---\n\n[[Alpha]]\n")
    notes = load_notes(tmp_vault)
    writer = VaultWriter(tmp_vault)
    queue = ReviewQueue(writer)
    findings = lint.run_lint(tmp_vault, notes, heat={}, today=TODAY)
    queued = lint.queue_findings(findings, queue, TODAY)
    assert {f.kind for f in queued} <= set(lint.QUEUE_KINDS)
    text = (tmp_vault / "review-queue.md").read_text()
    assert "review-after-expired" in text
    assert "oversized" not in text               # report-only category


def test_future_review_after_is_not_flagged(tmp_vault):
    (tmp_vault / "10-global/ttl.md").write_text(
        "---\ntitle: TTL\ntype: note\nreview-after: 2027-01\n---\n\n[[Alpha]]\n")
    notes = load_notes(tmp_vault)
    assert not lint.expired_reviews(notes, TODAY)


# -- transcript mining ---------------------------------------------------

def write_transcript(root, name, msgs):
    import json
    root.mkdir(parents=True, exist_ok=True)
    p = root / name
    p.write_text("\n".join(
        json.dumps({"message": {"role": r, "content": c}}) for r, c in msgs) + "\n")
    return p


def test_mining_writes_unverified_candidates_to_inbox(tmp_vault, tmp_path, store):
    root = tmp_path / "projects" / "proj"
    write_transcript(root, "s1.jsonl", [
        ("user", "Wie haben wir die Ollama-Kontextlaenge gesetzt? " * 20),
        ("assistant", [{"type": "text", "text": "Via LaunchAgent, 131072. " * 20}]),
    ])
    writer = VaultWriter(tmp_vault)
    client = FakeOllama(verdicts=[{"items": [
        {"title": "Ollama Kontextlaenge", "body": "OLLAMA_CONTEXT_LENGTH=131072 "
                                                  "steht im LaunchAgent-plist."},
        {"title": "Alpha", "body": "Existiert schon als Note, muss ignoriert werden."},
    ]}])
    res = mine.run_mining(tmp_vault, load_notes(tmp_vault), writer, client, store,
                          root=tmp_path / "projects", today=TODAY)
    assert res.transcripts == 1
    assert len(res.candidates) == 1              # "Alpha" collides with an existing note
    text = (tmp_vault / res.candidates[0]).read_text()
    assert "quelle: transcript-mining" in text
    assert "status: UNVERIFIED" in text
    assert "131072" in text
    # existing notes are never touched by mining
    assert "131072" not in (tmp_vault / "10-global/alpha.md").read_text()


def test_mining_does_not_repeat_the_same_item(tmp_vault, tmp_path, store):
    root = tmp_path / "projects" / "proj"
    write_transcript(root, "s1.jsonl", [("user", "Langer Text. " * 100)])
    verdict = {"items": [{"title": "Fakt", "body": "Ein dauerhafter Fakt ueber das Setup."}]}
    writer = VaultWriter(tmp_vault)
    mine.run_mining(tmp_vault, load_notes(tmp_vault), writer,
                    FakeOllama(verdicts=[verdict]), store,
                    root=tmp_path / "projects", today=TODAY)
    res2 = mine.run_mining(tmp_vault, load_notes(tmp_vault), VaultWriter(tmp_vault),
                           FakeOllama(verdicts=[verdict]), store,
                           root=tmp_path / "projects", today=TODAY)
    assert res2.candidates == []                 # already mined


def test_mining_is_capped_per_transcript_and_per_run(tmp_vault, tmp_path, store):
    # 00-sources is a human queue: one transcript flooding it with 10 "insights"
    # (or 15 transcripts x 3) is a dump, not a harvest
    root = tmp_path / "projects"
    many = {"items": [{"title": f"Fakt {i}", "body": "Ein dauerhafter Fakt ueber "
                                                     "das Setup, lang genug."}
                      for i in range(10)]}
    for t in range(6):
        write_transcript(root / f"p{t}", f"s{t}.jsonl", [("user", "Langer Text. " * 100)])
    writer = VaultWriter(tmp_vault)
    client = FakeOllama(verdicts=[dict(many) for _ in range(6)])
    res = mine.run_mining(tmp_vault, load_notes(tmp_vault), writer, client, store,
                          root=root, today=TODAY)
    assert len(res.candidates) <= config.MINE_MAX_PER_RUN
    assert len(set(res.candidates)) == len(res.candidates)   # no filename collisions


def test_only_recent_transcripts_are_read(tmp_path):
    import os
    root = tmp_path / "projects"
    p = write_transcript(root / "old", "old.jsonl", [("user", "alt")])
    stale = dt.datetime(2026, 1, 1).timestamp()
    os.utime(p, (stale, stale))
    write_transcript(root / "new", "new.jsonl", [("user", "neu")])
    names = [f.name for f in mine.recent_transcripts(root, now=NOW.timestamp())]
    assert names == ["new.jsonl"]


def test_a_synonym_of_an_existing_hub_is_not_a_new_topic(tmp_vault):
    """Am 2026-07-29 entstand `30-topics/quassel/` neben `30-topics/voxtype/`,
    obwohl drei der fuenf Quellnotizen dieselben waren: die App heisst another service,
    der Projektordner voxtype. `_covered` verlangte "fast alle" (len-1) und liess
    drei von fuenf durch."""
    for i in range(5):
        make_note(tmp_vault, f"10-global/quassel-{i}.md", f"another service Thema {i}", "x")
    notes = [n for n in load_notes(tmp_vault) if "quassel-" in n.rel]
    # Ein bestehender Hub verlinkt DREI der fuenf.
    hub_path = tmp_vault / "30-topics" / "voxtype" / "MOC.md"
    hub_path.parent.mkdir(parents=True, exist_ok=True)
    hub_path.write_text(
        "---\ntitle: voxtype MOC\n---\n\n"
        + "".join(f"- [[another service Thema {i}]]\n" for i in range(3)),
        encoding="utf-8")
    hub = parse_note(tmp_vault, hub_path)
    vectors = {n.rel: [1.0, 0.0] for n in notes}
    vectors[hub.rel] = [1.0, 0.0]

    found = topics.suggest_hubs([hub], notes, vectors)

    assert found == [], f"Synonym-Thema entstand trotzdem: {[n for n, _ in found]}"


def test_a_genuinely_new_theme_still_gets_through(tmp_vault):
    """Die schaerfere Regel darf echte Querschnittsthemen nicht mitsperren."""
    # Kein gemeinsames Fuellwort in den Titeln: bei Gleichstand gewinnt das
    # KUERZERE Wort, "Hetzner Thema 0..4" haette das Thema "thema" ergeben.
    for i, what in enumerate(("Backup", "Firewall", "DNS", "Router", "Konsole")):
        make_note(tmp_vault, f"10-global/hetzner-{i}.md", f"Hetzner {what}", "x")
    notes = [n for n in load_notes(tmp_vault) if "hetzner-" in n.rel]
    hub_path = tmp_vault / "30-topics" / "anderes" / "MOC.md"
    hub_path.parent.mkdir(parents=True, exist_ok=True)
    hub_path.write_text("---\ntitle: anderes MOC\n---\n\n- [[Hetzner Backup]]\n",
                        encoding="utf-8")
    hub = parse_note(tmp_vault, hub_path)
    vectors = {n.rel: [1.0, 0.0] for n in notes}
    vectors[hub.rel] = [1.0, 0.0]

    found = topics.suggest_hubs([hub], notes, vectors)

    assert [name for name, _ in found] == ["hetzner"]


def test_a_mined_note_declares_its_layer():
    """Rohmaterial in 00-sources/ ist Quellschicht - ohne `class: source` zaehlte
    es als Wissens-Waise und stand als unbehebbarer Befund in der Queue
    (2026-07-29, neun Notizen)."""
    text = mine.candidate_note("Titel", "Rumpf", "transcript.jsonl", TODAY)
    assert "class: source\n" in text
