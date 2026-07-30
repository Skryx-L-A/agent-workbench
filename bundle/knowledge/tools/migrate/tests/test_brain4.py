"""Tests for the Brain 4.0 migration tool.

Every test builds its own throwaway vault under tmp_path. Nothing here reads
or writes ~/Knowledge.
"""
from __future__ import annotations

import subprocess
import sys
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
import brain4  # noqa: E402


# --- fixtures --------------------------------------------------------------


def write(root: Path, rel: str, text: str) -> Path:
    p = root / rel
    p.parent.mkdir(parents=True, exist_ok=True)
    p.write_text(text, encoding="utf-8")
    return p


def note(title: str, ntype: str = "note", extra: str = "", body: str = "Body.\n") -> str:
    return (f"---\ntitle: {title}\ntype: {ntype}\n{extra}---\n{body}")


@pytest.fixture
def vault(tmp_path: Path) -> Path:
    root = tmp_path / "vault"
    write(root, "INDEX.md", note("INDEX", extra="permalink: main/index\n",
                                 body="See `tools/gardener` and `templates/note.md`.\n"
                                      "Drop zone: 00-inbox/. People: 10-global/people/.\n"))
    write(root, "00-inbox/mined-2026-07-13-thing.md", note("Mined Thing"))
    write(root, "00-inbox/gardener-report-2026-07-10.md", note("Report"))
    write(root, "00-inbox/drop/.keep", "")
    write(root, "10-global/people/person-1.md", note("der Nutzer", "person",
                                                    extra="branch: 10-global\n"))
    write(root, "10-global/shared-brain.md",
          note("Shared Brain", extra="branch: 10-global\n",
               body="Stand: 2026-07\n\nRules live in `tools/hooks/`.\n"))
    write(root, "10-global/session-2026-07-10-setup.md",
          note("Session setup", "session", extra="branch: 10-global\n"))
    write(root, "10-global/old-machine/faulty-ram.md", note("Faulty RAM"))
    write(root, "20-projects/demo/MOC.md", note("demo MOC"))
    write(root, "20-projects/demo/session-2026-07-01-x.md",
          note("Session x", "session", extra="branch: 20-projects/demo\n"))
    write(root, "20-projects/demo/fact.md", note("A fact"))
    write(root, "templates/note.md", note("note template",
                                          extra="permalink: main/templates/note\n"))
    write(root, "tools/gardener/gardener/config.py",
          'STAGING_DIR = "00-inbox"\nHEAT_LOG = "tools/state/read-heat.log"\n')
    write(root, "tools/hooks/read-tracking.sh", 'LOG="$VAULT/tools/state/read-heat.log"\n')
    write(root, "tools/state/read-heat.log", "x\n")
    write(root, "90-secrets/keys.md", note("secret"))
    return root


def run(root: Path, *cmd: str) -> str:
    argv = [cmd[0], "--vault", str(root), *cmd[1:]]
    import io
    import contextlib
    buf = io.StringIO()
    with contextlib.redirect_stdout(buf):
        rc = brain4.main(argv)
    return f"rc={rc}\n{buf.getvalue()}"


def migrate(root: Path) -> None:
    run(root, "plan")
    run(root, "apply", "--no-snapshot")
    run(root, "rewrite")


def git_init(root: Path) -> None:
    for cmd in (["init", "-q"], ["config", "user.email", "t@t"],
                ["config", "user.name", "t"]):
        subprocess.run(["git", *cmd], cwd=root, check=True)
    subprocess.run(["git", "add", "-A"], cwd=root, check=True,
                   capture_output=True)
    subprocess.run(["git", "commit", "-qm", "base"], cwd=root, check=True)


# --- path mapping ----------------------------------------------------------


def test_top_level_renames(vault: Path):
    m = brain4.map_path
    assert m(vault, "00-inbox/review-queue.md") == "00-sources/review-queue.md"
    assert m(vault, "00-inbox/drop/x.pdf") == "00-sources/drop/x.pdf"
    assert m(vault, "10-global/people/person-1.md") == "40-people/person-1.md"
    assert m(vault, "templates/note.md") == "_meta/templates/note.md"
    assert m(vault, "tools/gardener/cli.py") == "_meta/tools/gardener/cli.py"


def test_mined_files_get_their_own_folder(vault: Path):
    got = brain4.map_path(vault, "00-inbox/mined-2026-07-13-thing.md")
    assert got == "00-sources/mined/mined-2026-07-13-thing.md"
    # a report is not a mining find and stays at the source root
    assert brain4.map_path(vault, "00-inbox/gardener-report-2026-07-10.md") == \
        "00-sources/gardener-report-2026-07-10.md"


def test_only_session_notes_move_into_sessions(vault: Path):
    moves = brain4.plan_moves(vault)
    assert moves["20-projects/demo/session-2026-07-01-x.md"] == \
        "20-projects/demo/sessions/session-2026-07-01-x.md"
    assert "20-projects/demo/fact.md" not in moves
    assert "20-projects/demo/MOC.md" not in moves


def test_global_session_notes_move_too(vault: Path):
    """10-global gets no exception: the class has to be readable from the path
    there as well."""
    moves = brain4.plan_moves(vault)
    assert moves["10-global/session-2026-07-10-setup.md"] == \
        "10-global/sessions/session-2026-07-10-setup.md"
    assert "10-global/shared-brain.md" not in moves
    assert "10-global/old-machine/faulty-ram.md" not in moves


def test_secrets_never_move(vault: Path):
    moves = brain4.plan_moves(vault)
    assert not any("90-secrets" in k or "90-secrets" in v for k, v in moves.items())


# --- permalinks / class / branch -------------------------------------------


@pytest.mark.parametrize("rel,want", [
    ("INDEX.md", "main/index"),
    ("40-people/person-1.md", "main/40-people/person-1"),
    ("20-projects/demo/MOC.md", "main/20-projects/demo/moc"),
    ("20-projects/demo/sessions/session-2026-07-01-x.md",
     "main/20-projects/demo/sessions/session-2026-07-01-x"),
    ("_meta/templates/note.md", "main/_meta/templates/note"),
])
def test_permalink_from_path(rel, want):
    assert brain4.permalink_for(rel) == want


@pytest.mark.parametrize("rel,want", [
    ("00-sources/mined/x.md", "source"),
    ("00-sources/review-queue.md", "source"),
    ("10-global/x.md", "knowledge"),
    ("40-people/x.md", "knowledge"),
    ("_meta/tools/README.md", "meta"),
    ("_meta/templates/note.md", "meta"),
    # the class is the note class, not the folder prefix
    ("20-projects/demo/sessions/session-2026-07-01-x.md", "source"),
    ("10-global/sessions/session-2026-07-10-setup.md", "source"),
])
def test_class_from_path(rel, want):
    assert brain4.class_for(rel) == want


def test_project_session_note_is_class_source_end_to_end(vault: Path):
    migrate(vault)
    for rel in ("20-projects/demo/sessions/session-2026-07-01-x.md",
                "10-global/sessions/session-2026-07-10-setup.md"):
        fm = brain4.load_note(vault / rel, rel).fm_lines
        assert brain4.fm_get(fm, "class") == "source", rel
        assert brain4.fm_get(fm, "type") == "session", rel
    # a fact note in the same project branch stays knowledge
    fact = brain4.load_note(vault / "20-projects/demo/fact.md", "x").fm_lines
    assert brain4.fm_get(fact, "class") == "knowledge"


def test_branch_for():
    assert brain4.branch_for("40-people/person-1.md") == "40-people"
    assert brain4.branch_for("20-projects/demo/sessions/s.md") == "20-projects/demo"
    assert brain4.branch_for("30-topics/local-models/MOC.md") == "30-topics/local-models"


@pytest.mark.parametrize("rel", ["STATUS.md", "INDEX.md", "HOT.md", "LOG.md"])
def test_root_notes_have_no_branch(rel):
    """Deriving one from the filename produced `branch: STATUS.md`."""
    assert brain4.branch_for(rel) is None


@pytest.mark.parametrize("rel,want", [
    ("_meta/tools/migrate/README.md", "_meta/tools/migrate"),
    ("_meta/tools/README.md", "_meta/tools"),
    ("_meta/templates/note.md", "_meta/templates"),
    ("_meta/tools/eval/README.md", "_meta/tools/eval"),
])
def test_branch_under_meta_is_the_containing_directory(rel, want):
    """`_meta` is a tree of tools and schema, not one flat branch."""
    assert brain4.branch_for(rel) == want


def test_root_note_loses_a_wrong_branch_field(vault: Path):
    write(vault, "STATUS.md", note("knowledge-vault STATUS",
                                   extra="branch: 10-global\n"))
    migrate(vault)
    fm = brain4.load_note(vault / "STATUS.md", "STATUS.md").fm_lines
    assert brain4.fm_get(fm, "branch") is None
    assert not any(ln.startswith("branch:") for ln in fm)
    # everything else survives
    assert brain4.fm_get(fm, "title") == "knowledge-vault STATUS"
    assert brain4.ULID_RE.match(brain4.fm_get(fm, "id"))


def test_meta_note_gets_its_containing_directory_as_branch(vault: Path):
    write(vault, "tools/README.md", note("tools README", "reference",
                                         extra="branch: tools\n"))
    migrate(vault)
    rel = "_meta/tools/README.md"
    fm = brain4.load_note(vault / rel, rel).fm_lines
    assert brain4.fm_get(fm, "branch") == "_meta/tools"


def test_stand_from_body():
    assert brain4.stand_from_body("bla\nStand: 2026-07\n") == "2026-07"
    assert brain4.stand_from_body("**Stand:** 2026-11 blah") == "2026-11"
    assert brain4.stand_from_body("no marker here") is None


# --- rewrite rules ---------------------------------------------------------


def test_rewrite_rules_hit_vault_paths(vault: Path):
    rules = brain4.build_rules(vault, brain4.plan_moves(vault))
    out, _ = brain4.apply_rules(
        "see tools/gardener and templates/note.md and 00-inbox/x", rules)
    assert out == "see _meta/tools/gardener and _meta/templates/note.md and 00-sources/x"


def test_rewrite_rules_leave_foreign_paths_alone(vault: Path):
    """The reason every rule is anchored: these all contain the substring but
    none of them is a vault path."""
    rules = brain4.build_rules(vault, brain4.plan_moves(vault))
    for text in ("docs/agents-and-tools/agent-skills/overview",
                 "~/AI/myproject/tools/finanztool",
                 "name/description/tools/model",
                 "separate tools/projects"):
        out, _ = brain4.apply_rules(text, rules)
        assert out == text, text


def test_rewrite_rules_are_idempotent(vault: Path):
    rules = brain4.build_rules(vault, brain4.plan_moves(vault))
    once, _ = brain4.apply_rules("tools/gardener templates/note.md 00-inbox", rules)
    twice, _ = brain4.apply_rules(once, rules)
    assert once == twice
    assert "_meta/_meta" not in twice


def test_already_moved_check_is_case_insensitive(vault: Path):
    """`_Meta/templates/note.md` is a real line in the gardener suite (a
    case-folding test). A case-sensitive guard turned it into
    `_Meta/_meta/templates/note.md`."""
    rules = brain4.build_rules(vault, brain4.plan_moves(vault))
    for text in ('assert is_excluded(Path("_Meta/templates/note.md"))',
                 "_META/tools/gardener", "_meta/templates/note.md"):
        out, _ = brain4.apply_rules(text, rules)
        assert out == text, text


# --- frontmatter -----------------------------------------------------------


def test_split_frontmatter_takes_only_the_first_block():
    text = "---\na: 1\n---\nbody\n---\nb: 2\n---\n"
    fm, body, has = brain4.split_frontmatter(text)
    assert has and fm == ["a: 1"] and body.startswith("body")


def test_merge_stacked_frontmatter_keeps_every_field():
    text = "---\npermalink: main/x\n---\n\n---\ntitle: T\ntype: note\n---\nbody\n"
    fm, body, _ = brain4.split_frontmatter(text)
    merged, rest, did = brain4.merge_stacked_frontmatter(fm, body)
    assert did
    assert merged == ["permalink: main/x", "title: T", "type: note"]
    assert rest.strip() == "body"
    # second pass finds nothing left to merge
    assert brain4.merge_stacked_frontmatter(merged, rest)[2] is False


def test_fm_set_never_drops_a_field():
    fm = ["title: T", "tags: [a, b]", "created: 2026-01-01"]
    out = brain4.fm_set(brain4.fm_set(fm, "schema", "4"), "title", "T2")
    assert "tags: [a, b]" in out and "created: 2026-01-01" in out
    assert "title: T2" in out and "schema: 4" in out


def test_ulid_shape_and_uniqueness():
    ids = {brain4.ulid() for _ in range(500)}
    assert len(ids) == 500
    assert all(brain4.ULID_RE.match(i) for i in ids)


# --- end to end ------------------------------------------------------------


def test_full_migration(vault: Path):
    migrate(vault)
    assert (vault / "00-sources/mined/mined-2026-07-13-thing.md").exists()
    assert (vault / "40-people/person-1.md").exists()
    assert (vault / "_meta/templates/note.md").exists()
    assert (vault / "_meta/tools/gardener/gardener/config.py").exists()
    assert (vault / "20-projects/demo/sessions/session-2026-07-01-x.md").exists()
    assert not (vault / "00-inbox").exists()
    assert not (vault / "tools").exists()

    cfg = (vault / "_meta/tools/gardener/gardener/config.py").read_text()
    assert 'STAGING_DIR = "00-sources"' in cfg
    assert 'HEAT_LOG = "_meta/tools/state/read-heat.log"' in cfg

    idx = (vault / "INDEX.md").read_text()
    assert "_meta/tools/gardener" in idx and "40-people/" in idx
    assert "00-inbox" not in idx


def test_ids_are_stable_across_reruns(vault: Path):
    migrate(vault)
    first = {p.name: brain4.fm_get(brain4.load_note(p, p.name).fm_lines, "id")
             for p in vault.rglob("*.md") if "90-secrets" not in p.parts}
    migrate(vault)
    second = {p.name: brain4.fm_get(brain4.load_note(p, p.name).fm_lines, "id")
              for p in vault.rglob("*.md") if "90-secrets" not in p.parts}
    assert first == second
    assert all(v is None or brain4.ULID_RE.match(v) for v in first.values())


def test_second_run_changes_nothing(vault: Path):
    migrate(vault)
    # the manifest is the tool's own output (it carries a timestamp), not vault content
    def snap():
        return {p: p.read_bytes() for p in sorted(vault.rglob("*"))
                if p.is_file() and p.name != brain4.MANIFEST_NAME}
    before = snap()
    migrate(vault)
    after = snap()
    changed = [str(p) for p in before if before[p] != after.get(p)]
    assert changed == []
    assert set(before) == set(after)


def test_branch_is_corrected_only_where_the_move_broke_it(vault: Path):
    migrate(vault)
    person = brain4.load_note(vault / "40-people/person-1.md", "40-people/person-1.md")
    assert brain4.fm_get(person.fm_lines, "branch") == "40-people"
    s = vault / "20-projects/demo/sessions/session-2026-07-01-x.md"
    session = brain4.load_note(s, "x")
    assert brain4.fm_get(session.fm_lines, "branch") == "20-projects/demo"


def test_secrets_are_untouched_end_to_end(vault: Path):
    secret = vault / "90-secrets/keys.md"
    before = secret.read_bytes()
    stat_before = secret.stat().st_mtime_ns
    migrate(vault)
    assert secret.read_bytes() == before
    assert secret.stat().st_mtime_ns == stat_before
    assert "verify" and run(vault, "verify").startswith("rc=0")


def test_verify_fails_when_a_permalink_is_wrong(vault: Path):
    migrate(vault)
    p = vault / "10-global/shared-brain.md"
    p.write_text(p.read_text().replace("permalink: main/10-global/shared-brain",
                                       "permalink: main/wrong/place"))
    assert run(vault, "verify").startswith("rc=1")


def test_verify_fails_on_duplicate_ids(vault: Path):
    migrate(vault)
    a = vault / "10-global/shared-brain.md"
    b = vault / "20-projects/demo/fact.md"
    dup = brain4.fm_get(brain4.load_note(a, "a").fm_lines, "id")
    old = brain4.fm_get(brain4.load_note(b, "b").fm_lines, "id")
    b.write_text(b.read_text().replace(f"id: {old}", f"id: {dup}"))
    assert run(vault, "verify").startswith("rc=1")


def test_the_tool_never_rewrites_its_own_source(vault: Path):
    """brain4.py encodes the old paths on purpose and its README quotes both
    sides of every rename; rewriting them turns the rules into no-ops."""
    write(vault, "tools/migrate/brain4.py",
          'DIR_RENAMES = [("00-inbox", "00-sources"), ("tools", "_meta/tools")]\n')
    write(vault, "tools/migrate/README.md",
          note("brain4", body="Fix `tools/hooks/auto-recall.sh` by hand.\n"))
    migrate(vault)
    src = (vault / "_meta/tools/migrate/brain4.py").read_text()
    assert src == ('DIR_RENAMES = [("00-inbox", "00-sources"), '
                   '("tools", "_meta/tools")]\n')
    readme = (vault / "_meta/tools/migrate/README.md").read_text()
    assert "`tools/hooks/auto-recall.sh`" in readme
    assert "id:" not in readme.split("---")[1]


def test_plan_writes_nothing_but_the_manifest(vault: Path):
    before = {p: p.read_bytes() for p in sorted(vault.rglob("*")) if p.is_file()}
    run(vault, "plan")
    after = {p: p.read_bytes() for p in sorted(vault.rglob("*")) if p.is_file()}
    new = set(after) - set(before)
    assert {p.name for p in new} == {brain4.MANIFEST_NAME}
    assert all(before[p] == after[p] for p in before)


def test_apply_uses_git_mv_so_history_follows(vault: Path):
    git_init(vault)
    run(vault, "plan")
    run(vault, "apply", "--no-snapshot")
    out = subprocess.run(["git", "status", "--porcelain"], cwd=vault,
                         capture_output=True, text=True).stdout
    assert "R  10-global/people/person-1.md -> 40-people/person-1.md" in out


# --- apply guards ----------------------------------------------------------


def test_apply_refuses_a_dirty_worktree(vault: Path):
    git_init(vault)
    (vault / "10-global/shared-brain.md").write_text("changed\n")
    run(vault, "plan")
    with pytest.raises(SystemExit) as exc:
        brain4.main(["apply", "--vault", str(vault), "--no-snapshot"])
    assert "working tree is not clean" in str(exc.value)
    assert "shared-brain.md" in str(exc.value)
    # nothing was moved
    assert (vault / "10-global/people/person-1.md").exists()
    assert not (vault / "40-people").exists()


def test_the_manifest_alone_does_not_count_as_dirty(vault: Path):
    """`plan` writes it immediately before `apply` - it must not block."""
    git_init(vault)
    run(vault, "plan")
    assert brain4.MANIFEST_NAME in subprocess.run(
        ["git", "status", "--porcelain"], cwd=vault,
        capture_output=True, text=True).stdout
    assert run(vault, "apply", "--no-snapshot").startswith("rc=0")
    assert (vault / "40-people/person-1.md").exists()


def test_apply_checks_write_permission_before_the_snapshot(vault: Path,
                                                           tmp_path: Path):
    """Stress finding B03: a read-only directory used to raise PermissionError
    inside move_one() - half the vault moved, no rollback, a raw traceback."""
    git_init(vault)
    run(vault, "plan")
    snap = tmp_path / "snapshot"
    ro = vault / "10-global"
    ro.chmod(0o555)
    try:
        with pytest.raises(SystemExit) as exc:
            brain4.main(["apply", "--vault", str(vault),
                         "--snapshot-dir", str(snap)])
    finally:
        ro.chmod(0o755)
    msg = str(exc.value)
    assert "not writable" in msg and "10-global" in msg
    assert "chmod" in msg                     # says how to get out of it
    assert not snap.exists()                  # checked BEFORE the snapshot
    assert (vault / "10-global/people/person-1.md").exists()
    assert not (vault / "40-people").exists()  # and before the first move


def test_rewrite_writes_nothing_when_one_file_is_read_only(vault: Path, capsys):
    """B03, rewrite half: the same run must not leave the vault half rewritten."""
    run(vault, "plan")
    target = vault / "10-global/shared-brain.md"
    before, index_before = target.read_text(), (vault / "INDEX.md").read_text()
    target.chmod(0o444)
    try:
        rc = brain4.main(["rewrite", "--vault", str(vault)])
    finally:
        target.chmod(0o644)
    err = capsys.readouterr().err
    assert rc == 1
    assert "not writable" in err and "shared-brain.md" in err
    assert "Traceback" not in err
    assert target.read_text() == before
    assert (vault / "INDEX.md").read_text() == index_before   # nothing written


def test_apply_resumes_an_interrupted_run(vault: Path):
    """B12: after a SIGKILL mid-apply the staged renames are the run's progress,
    not a dirty tree. Committing them would commit half a migration."""
    git_init(vault)
    run(vault, "plan")
    # what an interrupted apply leaves behind: some planned renames staged
    subprocess.run(["git", "mv", "00-inbox", "00-sources"], cwd=vault,
                   check=True, capture_output=True)
    out = run(vault, "apply", "--no-snapshot")
    assert out.startswith("rc=0")
    assert "resuming an interrupted run" in out
    assert "Commit or stash" not in out
    assert (vault / "40-people/person-1.md").exists()
    assert (vault / "00-sources/mined/mined-2026-07-13-thing.md").exists()


def test_an_interrupted_run_is_never_answered_with_commit_or_stash(vault: Path):
    """B12, the dangerous half: unrelated changes on top of an interrupted run
    still block - but with the rollback/resume way out, not with a commit."""
    git_init(vault)
    run(vault, "plan")
    subprocess.run(["git", "mv", "10-global/people", "40-people"], cwd=vault,
                   check=True, capture_output=True)
    (vault / "10-global/shared-brain.md").write_text("changed by someone else\n")
    with pytest.raises(SystemExit) as exc:
        brain4.main(["apply", "--vault", str(vault), "--no-snapshot"])
    msg = str(exc.value)
    assert "interrupted" in msg and "shared-brain.md" in msg
    assert "Commit or stash them first" not in msg
    assert "git reset --hard" in msg and "resumes" in msg
    assert not (vault / "00-sources").exists()    # nothing else was moved


def test_untracked_files_of_a_parallel_writer_do_not_block(vault: Path):
    """B14: Obsidian, the sync or a second agent writes one note during the run
    - that must not stop a vault migration."""
    git_init(vault)
    run(vault, "plan")
    stray = vault / "10-global/eindringling.md"
    stray.write_text("written by another agent mid-run\n")
    out = run(vault, "apply", "--no-snapshot")
    assert out.startswith("rc=0")
    assert "eindringling.md" in out               # named, not silently swept
    assert (vault / "40-people/person-1.md").exists()
    assert stray.read_text() == "written by another agent mid-run\n"


def test_apply_writes_a_snapshot_before_moving(vault: Path, tmp_path: Path):
    snap = tmp_path / "snapshot"
    git_init(vault)
    run(vault, "plan")
    run(vault, "apply", "--snapshot-dir", str(snap))
    # the snapshot holds the PRE-migration layout and the sha to roll back to
    assert (snap / "10-global/people/person-1.md").exists()
    assert (snap / "tools/gardener/gardener/config.py").exists()
    assert not (snap / "40-people").exists()
    sha = (snap / "HEAD-before-brain4.txt").read_text().strip()
    assert len(sha) == 40
    # 90-secrets is never copied, not even into a snapshot
    assert not (snap / "90-secrets").exists()


def test_apply_keeps_an_existing_snapshot(vault: Path, tmp_path: Path):
    snap = tmp_path / "snapshot"
    snap.mkdir()
    (snap / "marker.txt").write_text("from an earlier run")
    run(vault, "plan")
    run(vault, "apply", "--snapshot-dir", str(snap))
    assert (snap / "marker.txt").read_text() == "from an earlier run"


# --- verify without a manifest ---------------------------------------------


def test_verify_without_a_manifest_explains_itself(vault: Path, capsys):
    migrate(vault)
    (vault / brain4.MANIFEST_NAME).unlink()      # runbook step 11 does this
    rc = brain4.main(["verify", "--vault", str(vault)])
    err = capsys.readouterr().err
    assert rc == 2                                # 2 = cannot run, 1 = violations
    assert "no manifest" in err and "plan" in err
    assert "Traceback" not in err


def test_rewrite_refuses_on_an_already_migrated_vault(vault: Path, capsys):
    """Prose that names the old paths on purpose - a rename log, a migration
    instruction - is indistinguishable from a stale reference."""
    migrate(vault)
    write(vault, "LOG.md", note("LOG", body="2026-07-28 | 10-global/people -> 40-people\n"))
    run(vault, "plan")
    rc = brain4.main(["rewrite", "--vault", str(vault)])
    err = capsys.readouterr().err
    assert rc == 2
    assert "already migrated" in err
    assert "10-global/people -> 40-people" in (vault / "LOG.md").read_text()


def test_force_lets_rewrite_run_anyway(vault: Path):
    migrate(vault)
    write(vault, "LOG.md", note("LOG", body="10-global/people -> 40-people\n"))
    run(vault, "plan")
    assert run(vault, "rewrite", "--force").startswith("rc=0")
    assert "40-people -> 40-people" in (vault / "LOG.md").read_text()


def test_verify_with_a_broken_manifest_explains_itself(vault: Path, capsys):
    migrate(vault)
    (vault / brain4.MANIFEST_NAME).write_text("{not json")
    rc = brain4.main(["verify", "--vault", str(vault)])
    assert rc == 2
    assert "not valid JSON" in capsys.readouterr().err


def test_real_vault_is_refused_without_the_flag(monkeypatch, tmp_path: Path):
    monkeypatch.setattr(brain4, "REAL_VAULT", tmp_path.resolve())
    with pytest.raises(SystemExit):
        brain4.main(["apply", "--vault", str(tmp_path)])
