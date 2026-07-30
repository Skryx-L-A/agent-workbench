#!/usr/bin/env python3
# /// script
# requires-python = ">=3.11"
# dependencies = []
# ///
"""Brain 4.0 vault migration: plan -> apply -> rewrite -> verify.

Contract: 20-projects/brain3/BRAIN4-PLAN.md. Four subcommands, each idempotent:

  plan     write a manifest of every intended change; touches nothing
  apply    perform the moves via `git mv` (history follows)
  rewrite  frontmatter schema v4 + path references in prose, code, hooks, configs
  verify   hard checks against the manifest; exit != 0 on any violation

Every phase refuses to look below `90-secrets/`. Running against the real vault
needs --allow-real-vault, so a stray invocation cannot touch it.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import secrets
import subprocess
import sys
import time
from dataclasses import dataclass, field
from pathlib import Path

# --- hard exclusions -------------------------------------------------------

SECRETS_DIR = "90-secrets"
# Never read, never move, never rewrite. 90-secrets is first for a reason.
EXCLUDE_PARTS = {
    SECRETS_DIR, ".git", ".obsidian", ".venv", "__pycache__",
    ".pytest_cache", "node_modules", ".ruff_cache", ".mypy_cache",
}
# Text files we rewrite path references in. Anything else is left alone.
TEXT_SUFFIXES = {
    ".md", ".py", ".sh", ".toml", ".yaml", ".yml", ".json", ".txt", ".cfg",
    ".ini", ".canvas", ".base",
}
# Suffix-less files that are still text we must rewrite (the pre-push hook is
# the one that actually matters).
TEXT_FILENAMES = {".gitignore", ".gitattributes", ".brainignore", "pre-push",
                  "Makefile"}
# Generated/cached artefacts: their content is reproduced by the tool that owns
# them, rewriting them by hand only creates a stale second truth.
SKIP_REWRITE_SUFFIXES = {".lock"}
MANIFEST_NAME = ".brain4-manifest.json"

REAL_VAULT = (Path.home() / "Knowledge").resolve()

# --- ULID ------------------------------------------------------------------

_CROCKFORD = "0123456789ABCDEFGHJKMNPQRSTVWXYZ"
ULID_RE = re.compile(r"^[0-9A-HJKMNP-TV-Z]{26}$")


def ulid(now_ms: int | None = None) -> str:
    """Crockford-base32 ULID: 48 bit ms timestamp + 80 bit randomness."""
    ts = int(time.time() * 1000) if now_ms is None else now_ms
    value = (ts << 80) | secrets.randbits(80)
    out = []
    for shift in range(125, -1, -5):
        out.append(_CROCKFORD[(value >> shift) & 0x1F])
    return "".join(out)


# --- frontmatter -----------------------------------------------------------

FM_KEY_RE = re.compile(r"^([A-Za-z][\w-]*)\s*:\s?(.*)$")


@dataclass
class Note:
    path: Path                 # absolute
    rel: str                   # vault-relative, posix
    fm_lines: list[str]        # raw frontmatter lines, without the --- fences
    body: str                  # everything after the closing fence
    has_fm: bool


def split_frontmatter(text: str) -> tuple[list[str], str, bool]:
    """Split off the FIRST frontmatter block only.

    templates/topic-moc.md carries a second, illustrative `---` block in its
    body; parsing greedily would rewrite the example instead of the real one.
    """
    if not text.startswith("---\n"):
        return [], text, False
    end = text.find("\n---", 3)
    if end == -1:
        return [], text, False
    # closing fence must be a line of its own
    line_end = text.find("\n", end + 1)
    if line_end == -1:
        line_end = len(text)
    if text[end + 1:line_end].strip() != "---":
        return [], text, False
    fm = text[4:end + 1].splitlines()
    return fm, text[line_end + 1:], True


def fm_get(fm_lines: list[str], key: str) -> str | None:
    for line in fm_lines:
        m = FM_KEY_RE.match(line)
        if m and m.group(1) == key:
            return m.group(2).strip()
    return None


def fm_set(fm_lines: list[str], key: str, value: str) -> list[str]:
    """Replace `key` in place, or append it. Never removes anything else."""
    out = list(fm_lines)
    for i, line in enumerate(out):
        m = FM_KEY_RE.match(line)
        if m and m.group(1) == key:
            out[i] = f"{key}: {value}"
            return out
    out.append(f"{key}: {value}")
    return out


def fm_del(fm_lines: list[str], key: str) -> list[str]:
    """Drop a key. Used only where the field is wrong by construction - a root
    note has no branch, so `branch: STATUS.md` is not data worth keeping."""
    return [ln for ln in fm_lines
            if not ((m := FM_KEY_RE.match(ln)) and m.group(1) == key)]


def render(fm_lines: list[str], body: str) -> str:
    return "---\n" + "\n".join(fm_lines) + "\n---\n" + body


def merge_stacked_frontmatter(fm_lines: list[str],
                              body: str) -> tuple[list[str], str, bool]:
    """Fold a second frontmatter block back into the first.

    13 notes in the vault carry a permalink-only stub block written on top of
    the real one (Basic Memory did this), so `title`/`type` sit in what every
    parser reads as prose. Migrating only the first block would leave `id` and
    `class` in the stub and the note's actual metadata stranded. Merging is
    additive: keys already present in the first block win, nothing is dropped.
    """
    stripped = body.lstrip("\n")
    second_fm, rest, has = split_frontmatter(stripped)
    if not has:
        return fm_lines, body, False
    if not all(FM_KEY_RE.match(ln) or not ln.strip() for ln in second_fm):
        return fm_lines, body, False
    present = {m.group(1) for ln in fm_lines if (m := FM_KEY_RE.match(ln))}
    merged = list(fm_lines)
    for ln in second_fm:
        m = FM_KEY_RE.match(ln)
        if not m:
            continue
        if m.group(1) in present:
            continue
        merged.append(ln)
        present.add(m.group(1))
    return merged, rest, True


# --- vault scanning --------------------------------------------------------


def excluded(rel: str) -> bool:
    return any(part in EXCLUDE_PARTS for part in Path(rel).parts)


def iter_files(root: Path):
    for dirpath, dirnames, filenames in os.walk(root):
        dirnames[:] = [d for d in dirnames if d not in EXCLUDE_PARTS]
        for name in sorted(filenames):
            if name == MANIFEST_NAME:
                continue
            p = Path(dirpath) / name
            rel = p.relative_to(root).as_posix()
            if excluded(rel):
                continue
            yield p, rel


def load_note(path: Path, rel: str) -> Note:
    text = path.read_text(encoding="utf-8", errors="replace")
    fm, body, has = split_frontmatter(text)
    return Note(path=path, rel=rel, fm_lines=fm, body=body, has_fm=has)


def is_note(rel: str, path: Path) -> bool:
    """A Note is a .md file carrying frontmatter, outside the excluded trees.

    Files without frontmatter (pytest-cache READMEs and the like) are not
    knowledge and never receive an id.
    """
    if not rel.endswith(".md"):
        return False
    try:
        with path.open("r", encoding="utf-8", errors="replace") as fh:
            return fh.readline() == "---\n"
    except OSError:
        return False


# Directories under _meta whose markdown is regenerated by a tool. Stamping an
# id into them only guarantees the next tool run drops it again.
GENERATED_SEGMENTS = {"logs", "results", "state"}


def fm_migration_target(rel: str) -> bool:
    """Which notes take part in the frontmatter migration.

    Templates are excluded: an id in a template propagates into every note
    copied from it, which would produce colliding ids by design. Generated
    tool output is excluded for the same class of reason.
    """
    parts = Path(rel).parts
    if is_migration_tool(rel):
        return False
    if parts[:2] == ("_meta", "templates") or parts[:1] == ("templates",):
        return False
    if parts[0] in ("_meta", "tools") and GENERATED_SEGMENTS & set(parts):
        return False
    return True


def is_migration_tool(rel: str) -> bool:
    """This tool's own directory.

    Its source encodes the OLD paths on purpose ('00-inbox' -> '00-sources'),
    and its README quotes both sides of every rename. Rewriting them turns the
    rules into no-ops and the documentation into nonsense - measured: after a
    self-rewrite 9 of its own tests failed.
    """
    parts = Path(rel).parts
    return parts[:2] == ("tools", "migrate") or \
        parts[:3] == ("_meta", "tools", "migrate")


def is_text(path: Path, rel: str) -> bool:
    if path.name == MANIFEST_NAME or is_migration_tool(rel):
        return False
    if path.suffix in SKIP_REWRITE_SUFFIXES:
        return False
    return path.suffix in TEXT_SUFFIXES or path.name in TEXT_FILENAMES


# --- path mapping ----------------------------------------------------------

# Whole-tree renames. Done as directory moves in `apply` so that gitignored
# children (.venv, __pycache__) travel with their package instead of being
# orphaned at the old path.
DIR_RENAMES: list[tuple[str, str]] = [
    ("00-inbox", "00-sources"),
    ("10-global/people", "40-people"),
    ("templates", "_meta/templates"),
    ("tools", "_meta/tools"),
]

MINED_PREFIX = "mined-"


def plan_moves(root: Path) -> dict[str, str]:
    """rel_old -> rel_new. Derived from what is on disk, never hardcoded."""
    moves: dict[str, str] = {}
    for path, rel in iter_files(root):
        new = map_path(root, rel, path)
        if new != rel:
            moves[rel] = new
    return moves


def map_path(root: Path, rel: str, path: Path | None = None) -> str:
    parts = Path(rel).parts
    # 1) 00-inbox -> 00-sources, loose mined-*.md into mined/. Both spellings
    #    are accepted so the file step still works after the directory move.
    if parts[0] in ("00-inbox", "00-sources"):
        rest = parts[1:]
        if len(rest) == 1 and rest[0].startswith(MINED_PREFIX) and rest[0].endswith(".md"):
            return f"00-sources/mined/{rest[0]}"
        return "/".join(("00-sources",) + rest)
    # 2) people become their own class
    if parts[:2] == ("10-global", "people"):
        return "/".join(("40-people",) + parts[2:])
    # 3) schema/templates/tooling move under _meta
    if parts[0] in ("templates", "tools"):
        return "/".join(("_meta",) + parts)
    # 4) session notes become path-visible as immutable - in a project branch
    #    and in 10-global alike. An exception in 10-global would defeat the
    #    point of the layout: the note class must be readable from the path.
    if rel.endswith(".md") and parts[-1] not in ("MOC.md", "STATUS.md",
                                                 "DECISIONS.md"):
        branch_depth = 2 if parts[0] in ("20-projects", "30-topics") else 1
        if (len(parts) == branch_depth + 1
                and path is not None and note_type(path) == "session"):
            return "/".join(parts[:branch_depth] + ("sessions", parts[-1]))
    return rel


def note_type(path: Path) -> str | None:
    if not path.name.endswith(".md"):
        return None
    try:
        text = path.read_text(encoding="utf-8", errors="replace")
    except OSError:
        return None
    fm, _, has = split_frontmatter(text)
    if not has:
        return None
    return fm_get(fm, "type")


# --- permalink / class -----------------------------------------------------

SLUG_STRIP = re.compile(r"[^a-z0-9]+")


def slugify(name: str) -> str:
    s = SLUG_STRIP.sub("-", name.lower()).strip("-")
    return s


def permalink_for(rel: str) -> str:
    p = Path(rel)
    stem = slugify(p.stem)
    parent = p.parent.as_posix()
    if parent in (".", ""):
        return f"main/{stem}"
    return f"main/{parent}/{stem}"


def branch_for(rel: str) -> str | None:
    """The branch a note belongs to, or None for a note that has no branch.

    Root notes (INDEX, HOT, LOG, STATUS, ...) are not in a branch at all and
    carry no `branch` field - deriving one from the filename produced
    `branch: STATUS.md`. Under `_meta/` the branch is the containing directory
    (`_meta/tools/migrate`), because `_meta` is a tree of tools and schema, not
    one flat branch.
    """
    parts = Path(rel).parts
    if len(parts) == 1:
        return None
    if parts[0] == "_meta":
        return "/".join(parts[:-1])
    if parts[0] in ("20-projects", "30-topics") and len(parts) > 2:
        return "/".join(parts[:2])
    return parts[0]


def class_for(rel: str) -> str:
    """The note class, not the folder prefix.

    A session note is immutable raw material wherever it lives, so a
    `sessions/` segment makes it `source` even inside 20-projects.
    """
    parts = Path(rel).parts
    if parts[0] == "_meta":
        return "meta"
    if parts[0] == "00-sources" or "sessions" in parts[:-1]:
        return "source"
    return "knowledge"


STAND_RE = re.compile(r"\**\s*Stand\s*\**\s*:?\s*\**\s*(\d{4})-(\d{2})")


def stand_from_body(body: str) -> str | None:
    m = STAND_RE.search(body)
    if not m:
        return None
    return f"{m.group(1)}-{m.group(2)}"


# --- text rewrite rules ----------------------------------------------------

@dataclass
class Rule:
    label: str
    pattern: re.Pattern
    replacement: str


def build_rules(root: Path, moves: dict[str, str]) -> list[Rule]:
    """Anchored path-reference rules, most specific first.

    Anchoring matters: a bare `tools/` replacement would corrupt unrelated
    strings such as the URL fragment `agents-and-tools/agent-skills`. Every
    rule therefore requires a word boundary that a hyphen does not satisfy,
    and the generic names (`tools`, `templates`) additionally require a real
    child that exists on disk.
    """
    rules: list[Rule] = []
    left = r"(?<![-\w])"

    def add(label: str, old: str, new: str, right: str = r"(?![-\w])"):
        rules.append(Rule(label, re.compile(left + re.escape(old) + right), new))

    # -- session-note moves (full relative path, with and without .md)
    for old, new in sorted(moves.items(), key=lambda kv: -len(kv[0])):
        if "/sessions/" not in new:
            continue
        add(f"session:{old}", old, new)
        add(f"session:{old[:-3]}", old[:-3], new[:-3])

    # -- mined files land in their own folder
    add("mined", "00-inbox/mined-", "00-sources/mined/mined-", right="")

    # -- unique, unambiguous tokens: safe without a child anchor
    add("people", "10-global/people", "40-people")
    add("inbox", "00-inbox", "00-sources")

    # -- generic names: only rewritten in front of a child that really exists
    for generic in ("templates", "tools"):
        base = root / generic
        if not base.exists():
            base = root / "_meta" / generic
        children = set()
        if base.exists():
            for c in base.iterdir():
                if c.name in EXCLUDE_PARTS:
                    continue
                children.add(c.name)
                if c.suffix:
                    children.add(c.stem)
        for child in sorted(children, key=len, reverse=True):
            add(f"{generic}/{child}", f"{generic}/{child}",
                f"_meta/{generic}/{child}")
        # bare `templates/` / `tools/` with nothing usable after it
        rules.append(Rule(
            f"{generic}/",
            re.compile(left + re.escape(generic) + r"/(?!\w)"),
            f"_meta/{generic}/"))
        # `~/Knowledge/tools`, `Knowledge/templates` - vault-anchored, no slash
        rules.append(Rule(
            f"vault-anchored:{generic}",
            re.compile(r"(?<=Knowledge/)" + re.escape(generic) + r"(?![-\w/])"),
            f"_meta/{generic}"))
    return rules


ALREADY_MOVED = "_meta/"


def apply_rules(text: str, rules: list[Rule]) -> tuple[str, dict[str, int]]:
    """Apply every rule, skipping matches that already sit under `_meta/`.

    This is what makes a second pass a no-op: without it `_meta/templates/note`
    matches the `templates/note` rule again and grows another `_meta/` every
    run. The check is case-insensitive on purpose - a lookbehind was not, and
    `_Meta/templates/note.md` (a case-folding test in the gardener suite) slipped
    through and would have become `_Meta/_meta/templates/note.md`.
    """
    hits: dict[str, int] = {}
    n = len(ALREADY_MOVED)
    for rule in rules:
        count = 0

        def repl(m, rule=rule):
            nonlocal count
            if m.string[max(0, m.start() - n):m.start()].lower() == ALREADY_MOVED:
                return m.group(0)
            count += 1
            return rule.replacement

        text = rule.pattern.sub(repl, text)
        if count:
            hits[rule.label] = hits.get(rule.label, 0) + count
    return text, hits


# --- wikilinks -------------------------------------------------------------

WIKILINK_RE = re.compile(r"!?\[\[([^\]\n|#^]+)(?:[#^][^\]\n|]*)?(?:\|[^\]\n]*)?\]\]")


def wikilink_targets(text: str) -> list[str]:
    return [m.group(1).strip() for m in WIKILINK_RE.finditer(text)]


def build_link_index(root: Path) -> tuple[dict[str, list[str]], set[str]]:
    """Every handle a wikilink in this vault is actually written against.

    Obsidian resolves by file path/basename, Basic Memory by `title` and
    `permalink`, and this vault's notes use all of them interchangeably -
    `[[myproject MOC]]` is a title, `[[MOC]]` is a basename. An index that
    only knew basenames would report ~700 false breakages.
    """
    by_name: dict[str, list[str]] = {}
    all_rel: set[str] = set()
    def index(key: str, rel: str) -> None:
        for k in (key, key.lower()):
            by_name.setdefault(k, []).append(rel)

    for path, rel in iter_files(root):
        all_rel.add(rel)
        p = Path(rel)
        for key in (p.name, p.stem):
            index(key, rel)
        if not rel.endswith(".md"):
            continue
        try:
            text = path.read_text(encoding="utf-8", errors="replace")
        except OSError:
            continue
        fm, _body, has = split_frontmatter(text)
        if not has:
            continue
        for key_name in ("title", "permalink"):
            val = (fm_get(fm, key_name) or "").strip().strip("'\"")
            if val:
                index(val, rel)
                index(val.rsplit("/", 1)[-1], rel)
        aliases = (fm_get(fm, "aliases") or "").strip()
        for a in re.findall(r"[^,\[\]]+", aliases.strip("[]")):
            a = a.strip().strip("'\"")
            if a:
                index(a, rel)
    return by_name, all_rel


def resolve_link(target: str, by_name: dict[str, list[str]],
                 all_rel: set[str]) -> bool:
    t = target.strip().lstrip("./")
    if t in all_rel or f"{t}.md" in all_rel:
        return True
    # Obsidian and Basic Memory both match case-insensitively; `[[myproject moc]]`
    # is written against the title `myproject MOC`.
    for key in (t, Path(t).name, Path(t).stem):
        if key in by_name or key.lower() in by_name:
            return True
    return False


# --- guards ----------------------------------------------------------------


def guard_vault(root: Path, allow_real: bool, phase: str) -> None:
    if root.resolve() == REAL_VAULT and not allow_real:
        sys.exit(f"refusing to {phase} the real vault at {REAL_VAULT} "
                 f"without --allow-real-vault")
    if (root / SECRETS_DIR).exists():
        # never a hard error - the real vault has it - but it must stay untouched
        pass


def secrets_snapshot(root: Path) -> dict[str, str]:
    """Hash every file under 90-secrets *by name and mtime only*.

    Contents are never read: proving we did not touch them must not require
    reading them.
    """
    base = root / SECRETS_DIR
    if not base.exists():
        return {}
    out = {}
    for dirpath, _dirnames, filenames in os.walk(base):
        for name in filenames:
            p = Path(dirpath) / name
            try:
                st = p.stat()
            except OSError:
                continue
            out[p.relative_to(root).as_posix()] = f"{st.st_size}:{st.st_mtime_ns}"
    return out


# --- manifest --------------------------------------------------------------


def body_hash(path: Path) -> str:
    text = path.read_text(encoding="utf-8", errors="replace")
    _fm, body, _has = split_frontmatter(text)
    return hashlib.sha256(body.encode("utf-8")).hexdigest()


def default_manifest(root: Path) -> Path:
    """Vault root: the one directory `apply` never renames, so plan and verify
    find the same file before and after the migration."""
    return root / MANIFEST_NAME


def find_manifest(root: Path, given: str | None) -> Path:
    return Path(given) if given else default_manifest(root)


# --- plan ------------------------------------------------------------------


def cmd_plan(args) -> int:
    root = Path(args.vault).resolve()
    moves = plan_moves(root)
    rules = build_rules(root, moves)

    notes: list[str] = []
    id_new: list[str] = []
    permalink_changes: list[dict] = []
    bodies: dict[str, str] = {}
    for path, rel in iter_files(root):
        if is_note(rel, path):
            notes.append(rel)
            bodies[rel] = body_hash(path)
            if not fm_migration_target(rel):
                continue
            note = load_note(path, rel)
            if not (fm_get(note.fm_lines, "id") or "").strip():
                id_new.append(rel)
            new_rel = moves.get(rel, rel)
            old_pl = fm_get(note.fm_lines, "permalink")
            new_pl = permalink_for(new_rel)
            if old_pl != new_pl:
                permalink_changes.append(
                    {"file": rel, "from": old_pl, "to": new_pl})

    # which files carry path references that must be rewritten
    content_rewrites: list[dict] = []
    for path, rel in iter_files(root):
        if not is_text(path, rel):
            continue
        try:
            text = path.read_text(encoding="utf-8")
        except (UnicodeDecodeError, OSError):
            continue
        new_text, hits = apply_rules(text, rules)
        if new_text != text:
            content_rewrites.append(
                {"file": rel, "target": moves.get(rel, rel), "hits": hits})

    by_name, all_rel = build_link_index(root)
    links_total = 0
    links_broken: list[dict] = []
    for path, rel in iter_files(root):
        if not rel.endswith(".md"):
            continue
        text = path.read_text(encoding="utf-8", errors="replace")
        for t in wikilink_targets(text):
            links_total += 1
            if not resolve_link(t, by_name, all_rel):
                links_broken.append({"file": rel, "target": t})

    stacked: list[str] = []
    for path, rel in iter_files(root):
        if not is_note(rel, path):
            continue
        fm, body, _has = split_frontmatter(
            path.read_text(encoding="utf-8", errors="replace"))
        if merge_stacked_frontmatter(fm, body)[2]:
            stacked.append(rel)

    basenames: dict[str, list[str]] = {}
    for _p, rel in iter_files(root):
        if rel.endswith(".md"):
            basenames.setdefault(Path(rel).name, []).append(rel)
    dupes = {n: v for n, v in basenames.items() if len(v) > 1}

    manifest = {
        "vault": str(root),
        "generated": time.strftime("%Y-%m-%dT%H:%M:%S"),
        "counts": {
            "files": sum(1 for _ in iter_files(root)),
            "notes": len(notes),
            "moves": len(moves),
            "new_ids": len(id_new),
            "permalink_changes": len(permalink_changes),
            "content_rewrites": len(content_rewrites),
            "wikilinks": links_total,
            "wikilinks_broken": len(links_broken),
            "stacked_frontmatter": len(stacked),
        },
        "moves": moves,
        "notes": sorted(notes),
        "body_hashes": bodies,
        "new_ids": sorted(id_new),
        "permalink_changes": permalink_changes,
        "content_rewrites": content_rewrites,
        "wikilinks_broken": links_broken,
        "stacked_frontmatter": sorted(stacked),
        "duplicate_basenames": {k: v for k, v in sorted(dupes.items())},
        "secrets_snapshot": secrets_snapshot(root),
    }

    out = find_manifest(root, args.manifest)
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(json.dumps(manifest, indent=2, ensure_ascii=False) + "\n",
                   encoding="utf-8")

    c = manifest["counts"]
    print(f"vault              {root}")
    print(f"files              {c['files']}")
    print(f"notes              {c['notes']}")
    print(f"moves              {c['moves']}")
    print(f"new ids            {c['new_ids']}")
    print(f"permalink changes  {c['permalink_changes']}")
    print(f"content rewrites   {c['content_rewrites']}")
    print(f"wikilinks          {c['wikilinks']} ({c['wikilinks_broken']} unresolved)")
    print(f"stacked frontmatter {len(stacked)}")
    print(f"duplicate names    {len(dupes)}")
    print(f"manifest           {out}")
    if args.verbose:
        for old, new in sorted(moves.items()):
            print(f"  MV  {old}  ->  {new}")
        for r in content_rewrites:
            print(f"  RW  {r['file']}  {r['hits']}")
    return 0


# --- apply -----------------------------------------------------------------


def git(root: Path, *cmd: str) -> subprocess.CompletedProcess:
    return subprocess.run(["git", "-C", str(root), *cmd],
                          capture_output=True, text=True)


def move_one(root: Path, old: str, new: str, has_git: bool) -> None:
    src, dst = root / old, root / new
    if SECRETS_DIR in Path(old).parts or SECRETS_DIR in Path(new).parts:
        sys.exit(f"apply: refusing to touch {old} -> {new}")
    if dst.exists():
        sys.exit(f"apply: target already exists: {new}")
    dst.parent.mkdir(parents=True, exist_ok=True)
    if has_git:
        r = git(root, "mv", old, new)
        if r.returncode == 0:
            return
        # untracked path: git mv refuses, a plain rename is equivalent
    src.rename(dst)


def undo_move(root: Path, old: str, new: str, has_git: bool) -> bool:
    """Move `new` back to `old`. False if the vault would not take it back."""
    src, dst = root / new, root / old
    if not src.exists() or dst.exists():
        return False
    try:
        dst.parent.mkdir(parents=True, exist_ok=True)
        if has_git and git(root, "mv", new, old).returncode == 0:
            return True
        src.rename(dst)
        return True
    except OSError:
        return False


def rollback_moves(root: Path, done: list[tuple[str, str]], has_git: bool,
                   failed_move: str, why: str) -> None:
    """Put every move of THIS run back, then leave with an explanation."""
    stuck = [f"{new} -> {old}" for old, new in reversed(done)
             if not undo_move(root, old, new, has_git)]
    head = (f"apply: stopped at {failed_move}: {why}\n"
            f"apply: rolled back {len(done) - len(stuck)} of {len(done)} moves "
            f"made in this run.")
    if stuck:
        listing = "\n  ".join(stuck[:15])
        sys.exit(f"{head}\nThese could NOT be put back:\n  {listing}\n"
                 f"Use the snapshot: git reset --hard "
                 f"$(cat <snapshot>/HEAD-before-brain4.txt) && git clean -fd, "
                 f"then restore untracked files from the snapshot copy.")
    sys.exit(f"{head}\nThe vault is back at the state before this run. Fix the "
             f"cause above, then start again at `plan`.")


def existing_parent(path: Path) -> Path:
    p = path
    while not p.exists() and p.parent != p:
        p = p.parent
    return p


def unwritable_dirs(root: Path, pairs: list[tuple[str, str]]) -> list[str]:
    """Directories a move must be able to change, but cannot.

    A rename needs write permission on the directory it leaves AND on the one
    it enters. Learning that halfway through leaves a half-migrated vault -
    the one state a non-specialist cannot get out of by hand - so it is checked
    before the snapshot and before the first rename.
    """
    bad: set[str] = set()
    for old, new in pairs:
        src = root / old
        if not src.exists():
            continue
        for d in (src.parent, existing_parent((root / new).parent)):
            if not os.access(d, os.W_OK | os.X_OK):
                rel = d.relative_to(root).as_posix() if d != root else "."
                bad.add(rel)
    return sorted(bad)


def parse_status(root: Path) -> list[tuple[str, str, str | None]]:
    """(xy, path, origin) per entry of `git status --porcelain -z`.

    NUL-separated on purpose: with the plain format git quotes and escapes
    paths, and a rename entry then has to be split on a literal ` -> `.
    """
    r = git(root, "status", "--porcelain", "-z")
    if r.returncode != 0:
        sys.exit(f"apply: cannot read git status in {root}: {r.stderr.strip()}")
    fields = r.stdout.split("\0")
    out: list[tuple[str, str, str | None]] = []
    i = 0
    while i < len(fields):
        f = fields[i]
        i += 1
        if len(f) < 4:
            continue
        xy, path = f[:2], f[3:]
        origin = None
        if "R" in xy or "C" in xy:
            # rename/copy records are `XY <new>NUL<orig>NUL`
            origin = fields[i] if i < len(fields) else None
            i += 1
        out.append((xy, path, origin))
    return out


def planned_pair(root: Path, manifest_moves: dict[str, str],
                 old: str, new: str) -> bool:
    """Is `old -> new` a move this migration itself would make?

    The manifest is the record of where a run stood, so it is asked first; the
    rules answer for a vault whose manifest was deleted. `map_path` is called
    without the file (it moved away), which only costs the session-note rule -
    those pairs are covered by the manifest.
    """
    if manifest_moves.get(old) == new:
        return True
    for a, b in DIR_RENAMES:
        if old == a and new == b:
            return True
        if old.startswith(a + "/") and new == f"{b}/{old[len(a) + 1:]}":
            return True
    return map_path(root, old) == new


def check_worktree(root: Path, manifest_path: Path) -> int:
    """Refuse a dirty tree - but tell the three cases apart.

    * staged renames the plan predicted: an `apply` that was interrupted.
      "Commit or stash them first" is the worst possible advice here, it
      commits half a migration. The moves still missing are simply carried out.
    * untracked files no move touches: a parallel writer (Obsidian, the sync,
      another agent). Not our business - they get named, the run continues.
    * modified or deleted tracked files: still blocking. They would be swept
      into the migration commit, and the documented rollback would take them.

    Returns how many moves of an interrupted run were found already staged.
    """
    if not (root / ".git").exists():
        return 0
    manifest_moves: dict[str, str] = {}
    if manifest_path.exists():
        try:
            manifest_moves = json.loads(
                manifest_path.read_text(encoding="utf-8")).get("moves", {})
        except (json.JSONDecodeError, OSError, AttributeError):
            manifest_moves = {}

    resumed: list[str] = []
    untracked: list[str] = []
    blocking: list[str] = []
    for xy, path, origin in parse_status(root):
        if path == MANIFEST_NAME:
            continue
        if xy == "??":
            untracked.append(path)
        elif origin is not None and planned_pair(root, manifest_moves,
                                                 origin, path):
            resumed.append(f"{origin} -> {path}")
        else:
            blocking.append(f"{xy} {origin} -> {path}" if origin
                            else f"{xy} {path}")

    if blocking:
        listing = "\n  ".join(blocking[:15])
        more = (f"\n  ... and {len(blocking) - 15} more"
                if len(blocking) > 15 else "")
        if resumed:
            sys.exit(
                f"apply: an earlier run was interrupted - {len(resumed)} of its "
                f"moves are already staged - and there are {len(blocking)} "
                f"unrelated changes on top:\n  {listing}{more}\n"
                f"Do NOT commit now: that would commit a half-finished "
                f"migration. Either put the unrelated changes aside (git stash "
                f"push -- <paths>) and run `apply` again, which resumes the "
                f"interrupted run, or roll everything back with `git reset "
                f"--hard $(cat <snapshot>/HEAD-before-brain4.txt) && git clean "
                f"-fd` and start again at `plan`.")
        sys.exit(
            f"apply: refusing to run, the working tree is not clean "
            f"({len(blocking)} entries):\n  {listing}{more}\n"
            f"Commit or stash them first. `apply` makes one atomic commit's "
            f"worth of moves, and the rollback (git reset --hard + git clean "
            f"-fd) would take these with it.")

    if resumed:
        print(f"apply: resuming an interrupted run - {len(resumed)} moves are "
              f"already staged, the rest follows now.")
    if untracked:
        shown = ", ".join(untracked[:5])
        more = f" (+{len(untracked) - 5} more)" if len(untracked) > 5 else ""
        print(f"apply: {len(untracked)} untracked file(s) are none of this "
              f"migration's business, leaving them alone: {shown}{more}")
        print("apply: they are in the snapshot - a rollback's `git clean -fd` "
              "would remove them from the vault.")
    return len(resumed)


def make_snapshot(root: Path, snapshot_dir: Path) -> Path:
    """Full worktree copy plus the HEAD sha, before anything is moved.

    90-secrets is never copied (contract); .venv is rebuildable and large.
    """
    if snapshot_dir.exists() and any(snapshot_dir.iterdir()):
        print(f"apply: snapshot already present, keeping it: {snapshot_dir}")
        return snapshot_dir
    snapshot_dir.mkdir(parents=True, exist_ok=True)
    r = subprocess.run(
        ["rsync", "-a", "--exclude", SECRETS_DIR, "--exclude", ".venv",
         f"{root}/", f"{snapshot_dir}/"], capture_output=True, text=True)
    if r.returncode != 0:
        sys.exit(f"apply: snapshot failed, refusing to move anything: "
                 f"{r.stderr.strip()}")
    head = git(root, "rev-parse", "HEAD")
    if head.returncode == 0:
        (snapshot_dir / "HEAD-before-brain4.txt").write_text(
            head.stdout.strip() + "\n", encoding="utf-8")
    print(f"apply: snapshot written to {snapshot_dir}")
    return snapshot_dir


def cmd_apply(args) -> int:
    root = Path(args.vault).resolve()
    guard_vault(root, args.allow_real_vault, "apply to")
    resumed = check_worktree(root, find_manifest(root, args.manifest))

    # Order matters and has saved this once already: check first, then the
    # snapshot, then the first byte of writing.
    pairs = [(o, n) for o, n in DIR_RENAMES if (root / o).exists()]
    pairs += sorted(plan_moves(root).items())
    bad = unwritable_dirs(root, pairs)
    if bad:
        listing = "\n  ".join(bad)
        sys.exit(
            f"apply: refusing to run, {len(bad)} directory(s) in the vault are "
            f"not writable:\n  {listing}\n"
            f"The migration renames files in and out of them, so it would stop "
            f"halfway. Make them writable (chmod u+rwx <dir>, and chmod u+w on "
            f"read-only files inside), then run `apply` again.")

    if args.no_snapshot:
        print("apply: --no-snapshot given, no rollback copy is being made")
    else:
        default = (Path.home() / ".local" / "trash-snapshots" /
                   f"{time.strftime('%Y-%m-%d')}-brain4")
        make_snapshot(root, Path(args.snapshot_dir) if args.snapshot_dir
                      else default)
    has_git = (root / ".git").exists()
    done: list[tuple[str, str]] = []

    def perform(old: str, new: str) -> None:
        """Any OS refusal mid-run puts this run's moves back before leaving."""
        try:
            move_one(root, old, new, has_git)
        except OSError as exc:
            reason = (exc.strerror or str(exc)).lower()
            rollback_moves(root, done, has_git, f"{old} -> {new}",
                           f"errno {exc.errno}, {reason}")
        done.append((old, new))

    dirs_moved = 0
    for old, new in DIR_RENAMES:
        if not (root / old).exists():
            continue
        perform(old, new)
        dirs_moved += 1
        if args.verbose:
            print(f"  MVDIR  {old}/  ->  {new}/")

    moves = plan_moves(root)
    if not moves and not dirs_moved:
        if resumed:
            print(f"apply: nothing left to move - the interrupted run had "
                  f"already made all {resumed} moves")
        else:
            print("apply: nothing to move (already migrated)")
        return 0

    moved = skipped = 0
    for old, new in sorted(moves.items()):
        if not (root / old).exists():
            skipped += 1
            continue
        perform(old, new)
        moved += 1
        if args.verbose:
            print(f"  MV  {old}  ->  {new}")
    print(f"apply: renamed {dirs_moved} directories, moved {moved} files, "
          f"skipped {skipped} (git mv: {has_git})")
    return 0


# --- rewrite ---------------------------------------------------------------


def cmd_rewrite(args) -> int:
    root = Path(args.vault).resolve()
    guard_vault(root, args.allow_real_vault, "rewrite")
    manifest_path = find_manifest(root, args.manifest)
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    moves: dict[str, str] = manifest.get("moves", {})
    if not moves and not args.force:
        # `plan` found nothing to move, so this vault has already been migrated.
        # The path rules are unconditional and would hit prose that names the
        # old paths ON PURPOSE - measured on the live vault: LOG.md's rename
        # entry "10-global/people -> 40-people" collapses to
        # "40-people -> 40-people", and STATUS.md's "change `tools/…` to
        # `_meta/tools/…`" loses its before-side.
        print("rewrite: the manifest records no moves, so this vault is "
              "already migrated - refusing.", file=sys.stderr)
        print("Running the path rules again would rewrite text that names the "
              "old paths deliberately (the LOG entry documenting the rename, "
              "the STATUS instruction telling people what to change). Pass "
              "--force if that is really what you want.", file=sys.stderr)
        return 2
    rules = build_rules(root, moves)

    ids_assigned = 0
    fm_touched = 0
    content_touched = 0
    stacked_merged = 0
    # Same order as `apply`: everything is computed and checked first, the
    # writing happens afterwards in one go. A read-only file must not leave
    # half the vault rewritten.
    planned: list[tuple[Path, str, str]] = []

    for path, rel in iter_files(root):
        if SECRETS_DIR in Path(rel).parts:
            continue
        if not is_text(path, rel):
            continue
        try:
            text = path.read_text(encoding="utf-8")
        except (UnicodeDecodeError, OSError):
            continue

        note_file = is_note(rel, path) and fm_migration_target(rel)
        if note_file:
            fm_lines, body, _has = split_frontmatter(text)
            fm_lines, body, merged = merge_stacked_frontmatter(fm_lines, body)
            if merged:
                stacked_merged += 1
            before_fm = list(fm_lines)

            existing = (fm_get(fm_lines, "id") or "").strip()
            if not ULID_RE.match(existing):
                if existing:
                    # keep whatever was there; a foreign id is data, not noise
                    fm_lines = fm_set(fm_lines, "legacy-id", existing)
                fm_lines = fm_set(fm_lines, "id", ulid())
                ids_assigned += 1
            fm_lines = fm_set(fm_lines, "schema", "4")
            fm_lines = fm_set(fm_lines, "class", class_for(rel))
            fm_lines = fm_set(fm_lines, "permalink", permalink_for(rel))
            # `branch` is only corrected where the move made it wrong - a
            # session note stays on its project branch, a person note does not
            # stay on 10-global after landing in 40-people.
            old_branch = (fm_get(fm_lines, "branch") or "").strip()
            if old_branch and not rel.startswith(old_branch.rstrip("/") + "/"):
                want = branch_for(rel)
                fm_lines = (fm_del(fm_lines, "branch") if want is None
                            else fm_set(fm_lines, "branch", want))
            if not (fm_get(fm_lines, "stand") or "").strip():
                s = stand_from_body(body)
                if s:
                    fm_lines = fm_set(fm_lines, "stand", s)

            # `id` and `schema` belong at the top; everything else keeps its order
            head_idx = [i for i, ln in enumerate(fm_lines)
                        if (m := FM_KEY_RE.match(ln))
                        and m.group(1) in ("id", "schema")]
            head = [fm_lines[i] for i in sorted(
                head_idx, key=lambda i: FM_KEY_RE.match(fm_lines[i]).group(1))]
            fm_lines = head + [ln for i, ln in enumerate(fm_lines)
                               if i not in set(head_idx)]

            # path references inside the frontmatter (branch:, path:, source:)
            fm_text, _ = apply_rules("\n".join(fm_lines), rules)
            fm_lines = fm_text.split("\n")

            new_body, _ = apply_rules(body, rules)
            new_text = render(fm_lines, new_body)
            if fm_lines != before_fm:
                fm_touched += 1
            if new_body != body:
                content_touched += 1
        else:
            new_text, _ = apply_rules(text, rules)
            if new_text != text:
                content_touched += 1

        if new_text != text:
            planned.append((path, rel, new_text))

    blocked = [rel for path, rel, _ in planned if not os.access(path, os.W_OK)]
    if blocked:
        listing = "\n  ".join(blocked[:15])
        more = f"\n  ... and {len(blocked) - 15} more" if len(blocked) > 15 else ""
        print(f"rewrite: refusing to write, {len(blocked)} of "
              f"{len(planned)} file(s) that need changing are not writable:"
              f"\n  {listing}{more}", file=sys.stderr)
        print("Nothing was written. Make them writable (chmod u+w <file>) and "
              "run `rewrite` again - it is idempotent.", file=sys.stderr)
        return 1

    failed: list[str] = []
    for path, rel, new_text in planned:
        try:
            path.write_text(new_text, encoding="utf-8")
        except OSError as exc:
            failed.append(f"{rel} (errno {exc.errno}, "
                          f"{(exc.strerror or str(exc)).lower()})")

    print(f"rewrite: ids assigned {ids_assigned}, frontmatter updated "
          f"{fm_touched}, content rewritten {content_touched}, "
          f"stacked frontmatter merged {stacked_merged}")
    if failed:
        listing = "\n  ".join(failed[:15])
        print(f"rewrite: {len(failed)} file(s) could not be written:"
              f"\n  {listing}\nRe-run `rewrite` once the cause is gone.",
              file=sys.stderr)
        return 1
    return 0


# --- verify ----------------------------------------------------------------


@dataclass
class Report:
    violations: list[str] = field(default_factory=list)
    info: list[str] = field(default_factory=list)

    def fail(self, msg: str):
        self.violations.append(msg)


def cmd_verify(args) -> int:
    root = Path(args.vault).resolve()
    manifest_path = find_manifest(root, args.manifest)
    if not manifest_path.exists():
        # Normal after a finished migration: the runbook deletes the manifest.
        print(f"verify: no manifest at {manifest_path} - nothing to compare "
              f"against.", file=sys.stderr)
        print("verify checks the migrated vault against the state `plan` "
              "recorded beforehand. Without that record it cannot tell what "
              "changed. Re-run `plan` before the migration, or point at a kept "
              "copy with --manifest <path>.", file=sys.stderr)
        return 2
    try:
        manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as exc:
        print(f"verify: manifest at {manifest_path} is not valid JSON: {exc}",
              file=sys.stderr)
        return 2
    rep = Report()

    notes = [(p, rel) for p, rel in iter_files(root) if is_note(rel, p)]
    before_notes = manifest["counts"]["notes"]
    print(f"notes before {before_notes}  after {len(notes)}")
    if len(notes) != before_notes:
        rep.fail(f"note count changed: {before_notes} -> {len(notes)}")

    # -- wikilinks
    by_name, all_rel = build_link_index(root)
    total = 0
    broken: list[str] = []
    for path, rel in iter_files(root):
        if not rel.endswith(".md"):
            continue
        text = path.read_text(encoding="utf-8", errors="replace")
        for t in wikilink_targets(text):
            total += 1
            if not resolve_link(t, by_name, all_rel):
                broken.append(f"{rel}: [[{t}]]")
    before_links = manifest["counts"]["wikilinks"]
    before_broken = manifest["counts"]["wikilinks_broken"]
    print(f"wikilinks before {before_links} ({before_broken} unresolved)  "
          f"after {total} ({len(broken)} unresolved)")
    if len(broken) > before_broken:
        rep.fail(f"wikilinks newly broken: {before_broken} -> {len(broken)}")
        for b in broken[:20]:
            rep.fail(f"  broken link {b}")

    # -- permalinks unique + path consistent
    seen: dict[str, str] = {}
    pl_missing = pl_bad = 0
    for path, rel in notes:
        if not fm_migration_target(rel):
            continue
        note = load_note(path, rel)
        pl = fm_get(note.fm_lines, "permalink")
        if not pl:
            pl_missing += 1
            rep.fail(f"permalink missing: {rel}")
            continue
        want = permalink_for(rel)
        if pl != want:
            pl_bad += 1
            rep.fail(f"permalink not path-consistent: {rel}: {pl} != {want}")
        if pl in seen:
            rep.fail(f"permalink collision: {pl} in {seen[pl]} and {rel}")
        seen[pl] = rel
    print(f"permalinks   {len(seen)} unique, {pl_missing} missing, "
          f"{pl_bad} inconsistent")

    # -- ids: exactly one per note, no collisions
    ids: dict[str, str] = {}
    id_missing = id_bad = 0
    for path, rel in notes:
        if not fm_migration_target(rel):
            continue
        text = path.read_text(encoding="utf-8", errors="replace")
        fm, _body, _has = split_frontmatter(text)
        found = [ln for ln in fm
                 if (m := FM_KEY_RE.match(ln)) and m.group(1) == "id"]
        if len(found) != 1:
            id_missing += 1
            rep.fail(f"note has {len(found)} id fields: {rel}")
            continue
        val = fm_get(fm, "id") or ""
        if not ULID_RE.match(val):
            id_bad += 1
            rep.fail(f"id is not a ULID: {rel}: {val!r}")
        if val in ids:
            rep.fail(f"id collision: {val} in {ids[val]} and {rel}")
        ids[val] = rel
    print(f"ids          {len(ids)} unique, {id_missing} missing/duplicate, "
          f"{id_bad} malformed")

    # -- 90-secrets untouched
    before_secrets = manifest.get("secrets_snapshot", {})
    after_secrets = secrets_snapshot(root)
    if before_secrets != after_secrets:
        only_before = set(before_secrets) - set(after_secrets)
        only_after = set(after_secrets) - set(before_secrets)
        changed = {k for k in set(before_secrets) & set(after_secrets)
                   if before_secrets[k] != after_secrets[k]}
        rep.fail(f"90-secrets changed: removed={sorted(only_before)[:5]} "
                 f"added={sorted(only_after)[:5]} modified={sorted(changed)[:5]}")
    print(f"90-secrets   {len(after_secrets)} files, unchanged: "
          f"{before_secrets == after_secrets}")
    for rel in [r for _p, r in iter_files(root)]:
        if SECRETS_DIR in Path(rel).parts and rel not in before_secrets:
            rep.fail(f"new path under {SECRETS_DIR}: {rel}")

    # -- bodies changed outside frontmatter
    before_bodies: dict[str, str] = manifest.get("body_hashes", {})
    moves: dict[str, str] = manifest.get("moves", {})
    changed_bodies: list[str] = []
    for path, rel in notes:
        old_rel = rel
        if rel not in before_bodies:
            for o, n in moves.items():
                if n == rel:
                    old_rel = o
                    break
        if old_rel not in before_bodies:
            rep.fail(f"note not in manifest (new file?): {rel}")
            continue
        if body_hash(path) != before_bodies[old_rel]:
            changed_bodies.append(rel)
    print(f"bodies changed outside frontmatter: {len(changed_bodies)}")
    if args.verbose:
        for c in changed_bodies:
            print(f"  BODY {c}")

    # -- structure landed
    for gone in ("00-inbox", "templates", "tools", "10-global/people"):
        if (root / gone).exists():
            rep.fail(f"old path still present: {gone}")
    for want in ("00-sources", "_meta/templates", "_meta/tools", "40-people"):
        if not (root / want).exists():
            rep.fail(f"new path missing: {want}")

    if rep.violations:
        print(f"\nFAIL: {len(rep.violations)} violation(s)")
        for v in rep.violations[:80]:
            print(f"  - {v}")
        return 1
    print("\nOK: all checks passed")
    return 0


# --- cli -------------------------------------------------------------------


def main(argv=None) -> int:
    common = argparse.ArgumentParser(add_help=False)
    common.add_argument("--vault", default=str(Path.home() / "Knowledge"))
    common.add_argument("--manifest", default=None)
    common.add_argument("-v", "--verbose", action="store_true")
    common.add_argument("--allow-real-vault", action="store_true",
                        help="required to write to ~/Knowledge itself")
    common.add_argument("--snapshot-dir", default=None,
                        help="where `apply` puts the rollback copy "
                             "(default ~/.local/trash-snapshots/<date>-brain4)")
    common.add_argument("--no-snapshot", action="store_true",
                        help="skip the rollback copy `apply` makes; only for "
                             "runs against a throwaway copy")
    common.add_argument("--force", action="store_true",
                        help="let `rewrite` run again on an already migrated "
                             "vault (it will hit prose that names old paths "
                             "on purpose)")
    # NB the common args live on the subparsers only. Declaring them on both
    # would make the subparser's default clobber a value given before the
    # subcommand - which once pointed a --vault run at the real vault.
    ap = argparse.ArgumentParser(prog="brain4",
                                 description=__doc__.split("\n")[0])
    sub = ap.add_subparsers(dest="cmd", required=True)
    for name, fn in (("plan", cmd_plan), ("apply", cmd_apply),
                     ("rewrite", cmd_rewrite), ("verify", cmd_verify)):
        sp = sub.add_parser(name, parents=[common])
        sp.set_defaults(func=fn)
    args = ap.parse_args(argv)
    return args.func(args)


if __name__ == "__main__":
    raise SystemExit(main())
