#!/usr/bin/env bash
# gates.sh -- the build gates of the Rust workspace, one script, one exit code per gate.
#
#   ./gates.sh fmt        cargo fmt --check
#   ./gates.sh clippy     cargo clippy, every warning is an error
#   ./gates.sh test       cargo test --no-fail-fast, its test programs side by side (WB_TEST_JOBS, default 4); then the
#                         seam tests (`--test naht`) of the delivered programs without any feature, as the package
#                         builds them (rust/paket.sh)
#   ./gates.sh licenses   cargo-deny (licences, bans, sources) and a CycloneDX SBOM per crate
#   ./gates.sh notices    every row of "Code taken from other projects" in README.md has its notice
#                         (file or file head, names the SPDX id, ships in `cargo package --list`);
#                         a file under crates/*/src with a foreign licence head that no row lists is red;
#                         a crate with a bin target and a row runs each bin with `--licenses`: exit 0, and
#                         each notice file of its rows appears in the output byte for byte
#   ./gates.sh vertraege  the checkers and the case builders of every contract below spec/ (rust/spec/<name>/), one after
#                         the other, in a mktemp copy: `pruefen.py` with jsonschema (uv, offline, pinned version), a
#                         rebuild of the cases must give no difference, the mutation catalogue checks; no uv, no node or
#                         no jsonschema in the uv cache is red (exit 127), never skipped; as a second step the
#                         unit tests of the runner (the previous implementation*.py) under `env -i` with their own
#                         HOME, short TMPDIR and fixed PATH; every skipped test is counted and printed
#   ./gates.sh mutants    cargo-mutants, one run per crate (`-p`), one after the other; red if a mutant of a security
#                         crate (MUTANTS_PFLICHT) survives or times out, a run fails, or a security crate caught no
#                         mutant; the other crates only report (target/mutants/bericht.tsv). A second run covers the
#                         crates with seams (NAHT_CRATES) with the feature `pruefbau`. WB_MUTANTS_BASIS=<git ref> runs
#                         only the crates changed since the merge base with it, each limited to the changed lines
#                         (`--in-diff`); without it every security crate runs, in full
#   ./gates.sh mutants-plan  not a gate: prints which crates `mutants` would run, starts no cargo-mutants
#   ./gates.sh mutants-list  not a gate: `cargo mutants --list` for each run of the plan, with its count
#   ./gates.sh all        every gate in this order; prints one line per gate
#   ./gates.sh tools      not a gate: checks the tool folder (~/.local/share/wb-rust-werkzeuge), the toolchain
#                         and the three tools in their pinned versions; every gate makes the same checks first
#
# Exit codes: 0 green; the exit code of the failing tool for a single gate (at least 1);
# 1 for `all` when any gate is red; 64 usage error; 127 the tool folder, the toolchain or a required tool
# is missing or has another version than pinned.
# The script works from its own directory and writes only below it (target/, sbom/).
# The notices gate writes only its cargo build (target/) there; its scratch files live in a mktemp directory.
# Runs on bash 3.2 (macOS) and current bash; no associative arrays.

set -u
set -o pipefail

HERE="$(cd "$(dirname "$0")" && pwd)" || exit 64
cd "$HERE" || exit 64

GATES="fmt clippy test licenses notices mutants"
# The gate `vertraege` follows `mutants` in `all`; it is added in its own line, so the line above keeps its form.
GATES="$GATES vertraege"

# ---- tool place -----------------------------------------------------------
# The gates run only with the tools of one fixed folder, never with a cargo found in PATH (Homebrew, mise,
# a rustup of the user). The folder is made by tools/werkzeuge-installieren.sh and holds
#   rustup/  RUSTUP_HOME with the toolchain of rust-toolchain.toml
#   cargo/   CARGO_HOME (cargo, rustc and the other proxies in cargo/bin)
#   tools/   `cargo install --root` of cargo-deny, cargo-cyclonedx and cargo-mutants (tools/bin)
# PATH, RUSTUP_HOME and CARGO_HOME are set for this script only; no shell profile is touched.
# The folder is fixed below. WB_RUST_WERKZEUGE_ORT replaces it only while WB_TESTUMGEBUNG=1 (test seam).
# A missing folder, a missing toolchain or a tool of another version stops the gate with exit 127.
# The versions are pinned here, in tools/werkzeuge-installieren.sh and in README.md; the toolchain
# version is the channel of rust-toolchain.toml. test-rust-werkzeuge.sh holds the copies together.
TOOL_DENY="0.21.0"
TOOL_CYCLONEDX="0.5.10"
TOOL_MUTANTS="27.1.0"
TOOLS_ITEMS="toolchain deny cyclonedx mutants"
TOOLS_ORT=""
TOOLS_ENV_READY=0

tools_default_ort() {
  if [ "${WB_TESTUMGEBUNG:-}" = "1" ] && [ -n "${WB_RUST_WERKZEUGE_ORT:-}" ]; then
    printf '%s\n' "$WB_RUST_WERKZEUGE_ORT"
  else
    printf '%s\n' "${HOME:-}/.local/share/wb-rust-werkzeuge"
  fi
}

# Sets TOOLS_ORT, RUSTUP_HOME, CARGO_HOME and PATH, once. Exit 127 with a message when the folder is missing.
tools_prepare() {
  [ "$TOOLS_ENV_READY" -eq 1 ] && return 0
  local ort
  ort="$(tools_default_ort)"
  case "$ort" in
    /*) ;;
    *) echo "gates.sh: the tool folder '$ort' is not an absolute path (HOME not set?)" >&2; return 127 ;;
  esac
  if [ ! -d "$ort" ]; then
    echo "gates.sh: tool folder $ort not found; install it with: bash $HERE/tools/werkzeuge-installieren.sh --install --target $ort (README.md, section Tools)" >&2
    return 127
  fi
  TOOLS_ORT="$ort"
  unset RUSTUP_TOOLCHAIN CARGO RUSTC RUSTDOC RUSTFMT RUSTC_WRAPPER RUSTC_WORKSPACE_WRAPPER CARGO_INSTALL_ROOT CARGO_BUILD_RUSTC
  export RUSTUP_HOME="$ort/rustup" CARGO_HOME="$ort/cargo" RUSTUP_AUTO_INSTALL=0
  PATH="$ort/cargo/bin:$ort/tools/bin:$PATH"
  export PATH
  TOOLS_ENV_READY=1
}

tools_channel() {
  sed -n 's/^channel[[:space:]]*=[[:space:]]*"\([^"]*\)".*/\1/p' rust-toolchain.toml 2>/dev/null | head -n 1
}

# One item of TOOLS_ITEMS: prints the reason to stderr and returns 127 when it is missing or of another version.
tools_item() {
  tools_prepare || return
  local item="$1" ort="$TOOLS_ORT" want out chan found dir resolved
  case "$item" in
    toolchain)
      chan="$(tools_channel)"
      if [ -z "$chan" ]; then
        echo "gates.sh: rust-toolchain.toml names no channel; the toolchain version is unknown" >&2
        return 127
      fi
      found=""
      for dir in "$ort/rustup/toolchains/$chan"-*; do
        [ -d "$dir" ] && found="$dir"
      done
      if [ -z "$found" ]; then
        echo "gates.sh: toolchain $chan not found in $ort/rustup; run tools/werkzeuge-installieren.sh --install --target $ort" >&2
        return 127
      fi
      resolved="$(command -v cargo 2>/dev/null)"
      if [ "$resolved" != "$ort/cargo/bin/cargo" ]; then
        echo "gates.sh: cargo resolves to '${resolved:-nothing}', not to $ort/cargo/bin/cargo" >&2
        return 127
      fi
      out="$(cargo --version 2>&1 | head -n 1)"
      case "$out" in
        "cargo $chan "*) ;;
        *) echo "gates.sh: cargo reports '$out', expected version $chan (rust-toolchain.toml)" >&2; return 127 ;;
      esac
      out="$(rustc --version 2>&1 | head -n 1)"
      case "$out" in
        "rustc $chan "*) ;;
        *) echo "gates.sh: rustc reports '$out', expected version $chan (rust-toolchain.toml)" >&2; return 127 ;;
      esac
      return 0
      ;;
    deny) want="$TOOL_DENY" ;;
    cyclonedx) want="$TOOL_CYCLONEDX" ;;
    mutants) want="$TOOL_MUTANTS" ;;
    *) echo "gates.sh: unknown tool item '$item'" >&2; return 127 ;;
  esac
  tools_item toolchain || return
  resolved="$(command -v "cargo-$item" 2>/dev/null)"
  if [ "$resolved" != "$ort/tools/bin/cargo-$item" ]; then
    echo "gates.sh: cargo-$item not found at $ort/tools/bin/cargo-$item (found: ${resolved:-nothing}); run tools/werkzeuge-installieren.sh --install --target $ort" >&2
    return 127
  fi
  out="$(cargo "$item" --version 2>&1 | head -n 1)"
  case " $out " in
    *" $want "*) ;;
    *) echo "gates.sh: cargo $item reports '$out', expected version $want (README.md, section Tools)" >&2; return 127 ;;
  esac
}

# ./gates.sh tools: checks the folder, the toolchain and the three tools; one line each, exit 127 if one is not ok.
gate_tools() {
  local item rc=0
  if ! tools_prepare; then
    for item in $TOOLS_ITEMS; do echo "tools: $item MISSING (tool folder)"; done
    return 127
  fi
  for item in $TOOLS_ITEMS; do
    if tools_item "$item"; then echo "tools: $item ok"; else echo "tools: $item MISSING"; rc=127; fi
  done
  return "$rc"
}

need() {
  # Every gate needs the toolchain of the tool folder; no other cargo is accepted.
  tools_item toolchain
}

need_subcommand() {
  # cargo-<name> is taken from the tool folder only, in the pinned version.
  tools_item "$1"
}

gate_fmt() {
  need cargo || return
  cargo fmt --all --check
}

gate_clippy() {
  need cargo || return
  cargo clippy --workspace --all-targets --all-features --locked -- -D warnings || return
  # The delivered build has no features: code behind `#[cfg(not(feature = ...))]` is linted only here.
  cargo clippy --workspace --all-targets --locked -- -D warnings
}

# Crates whose program has seams of the test build (feature `pruefbau`); `tests/naht.rs` of each also runs without
# the feature and proves that the delivered program reads no seam. The second mutation run of each has the feature.
# OHNE_NAHTTEST are the crates of this list that have no `tests/naht.rs` (yet); the test gate skips them with a message.
# Any other crate of the list without that file is red.
NAHT_CRATES="anmelden werkbank-pruefen dienst halter lauf"
OHNE_NAHTTEST="lauf"

# The test programs run side by side (25.11.2026). `cargo test` starts its test programs one after the other; that took
# about 7.5 of the 9 to 14 minutes of a gate run (measured on the Linux build host: 175 programs, 449 s in sum, the longest
# lauf/tests/einschluss.rs with 133 s, the build 28 s). Now cargo builds and starts every program as before, but through a
# runner (CARGO_TARGET_<host>_RUNNER) that only writes down what cargo starts: the program, its arguments, its folder and
# its environment. Then up to TEST_JOBS of these programs run at once, each in the folder and with the environment that
# cargo gave it, without the runner variable. Only whole programs run side by side: the tests of one program still share
# one process and its test threads, so a lock inside a program (such as BIND_PLATZ of modellweg) holds as before; a tool
# that starts every test in a process of its own (cargo-nextest) would undo that. The output of each program stands in one
# block, in the order of cargo. WB_TEST_JOBS sets the number (1: one after the other).
TEST_JOBS="${WB_TEST_JOBS:-4}"
# Started first, because they run longest (measured 25.11.2026); each as <folder of the package>/<program without hash>.
TEST_ZUERST="lauf/einschluss"
# Run alone, after the others, with nothing beside them (as before 25.11.2026), same names. Empty: rangkern/embed stood here
# from 25.11.2026 until its port check counted only a listener of its own program (tests/embed.rs, `own_listener`); under the
# parallel run another program had got the port of its stand-in from `bind 0` once (embed.rs:443).
TEST_ALLEIN=""

TEST_PARALLEL_PY='import json, os, re, subprocess, sys
art = sys.argv[1]
if art == "merken":
    # The runner: cargo starts one program at a time, so the count of the folder is the place in the order of cargo.
    d = os.path.join(os.environ["WB_GATES_TEST_MERKEN"], "l")
    r = os.path.join(d, "%05d" % len(os.listdir(d)))
    os.mkdir(r)
    with open(os.path.join(r, "lauf.json.neu"), "w", encoding="utf-8") as f:
        json.dump({"argv": sys.argv[2:], "ordner": os.getcwd(), "umgebung": dict(os.environ)}, f)
    os.rename(os.path.join(r, "lauf.json.neu"), os.path.join(r, "lauf.json"))
elif art == "ordnen":
    d = os.path.join(sys.argv[2], "l")
    alle = sorted(os.listdir(d))
    def name(n):
        l = json.load(open(os.path.join(d, n, "lauf.json"), encoding="utf-8"))
        return os.path.basename(l["ordner"]) + "/" + re.sub(r"-[0-9a-f]{16}$", "", os.path.basename(l["argv"][0]))
    namen = {n: name(n) for n in alle}
    # argv: the folder, `parallel` or `allein`, then the names of TEST_ZUERST, `--`, the names of TEST_ALLEIN.
    rest = sys.argv[4:]
    zuerst, allein = (rest[:rest.index("--")], rest[rest.index("--") + 1:]) if "--" in rest else (rest, [])
    if sys.argv[3] == "allein":
        wahl = [n for n in alle if namen[n] in allein]
    else:
        frei = [n for n in alle if namen[n] not in allein]
        vorn = [n for z in zuerst for n in frei if namen[n] == z]
        wahl = vorn + [n for n in frei if n not in vorn]
    for n in wahl:
        print(os.path.join(d, n))
elif art == "spielen":
    ohne, r = sys.argv[2], sys.argv[3]
    l = json.load(open(os.path.join(r, "lauf.json"), encoding="utf-8"))
    env = {k: v for k, v in l["umgebung"].items() if k not in (ohne, "WB_GATES_TEST_MERKEN")}
    with open(os.path.join(r, "aus"), "wb") as aus:
        try:
            rc = subprocess.run(l["argv"], cwd=l["ordner"], env=env, stdin=subprocess.DEVNULL, stdout=aus,
                                stderr=subprocess.STDOUT).returncode
            rc = rc if rc >= 0 else 128 - rc
        except OSError as e:
            aus.write(("gates.sh: test: cannot start %s: %s\n" % (l["argv"][0], e)).encode())
            rc = 127
    with open(os.path.join(r, "rc"), "w") as f:
        f.write("%d\n" % rc)
elif art == "kopf":
    l = json.load(open(os.path.join(sys.argv[2], "lauf.json"), encoding="utf-8"))
    print("     Running %s (folder %s)" % (os.path.relpath(l["argv"][0], sys.argv[3]),
                                         os.path.relpath(l["ordner"], sys.argv[3])))'

# cargo test with the given arguments, its programs side by side (see TEST_JOBS). $1 the scratch folder, then the
# arguments of cargo test. The exit code is that of cargo when it fails (such as a build error), else 101 when a program
# failed, else 0. Red as well: the runner wrote down another number of programs than cargo named.
test_parallel() {
  local tmp="$1" host var py rc=0 r prc gesehen gemerkt kopf
  shift
  if ! command -v python3 >/dev/null 2>&1; then
    echo "gates.sh: python3 not found (see README.md, section Tools)" >&2
    return 127
  fi
  py="$(command -v python3)"
  case "$tmp$py" in *[[:space:]]*) echo "gates.sh: test: a path with a space ($tmp, $py)" >&2; return 64 ;; esac
  host="$(rustc -vV | sed -n 's/^host: //p')"
  [ -n "$host" ] || { echo "gates.sh: test: rustc -vV names no host" >&2; return 127; }
  var="CARGO_TARGET_$(printf '%s' "$host" | tr 'a-z.-' 'A-Z__')_RUNNER"
  printf '%s\n' "$TEST_PARALLEL_PY" > "$tmp/parallel.py"
  mkdir "$tmp/l" || return 64
  # The Running lines of cargo name programs that have not run yet; each block below brings its own.
  env "$var=$py -I $tmp/parallel.py merken" WB_GATES_TEST_MERKEN="$tmp" cargo test "$@" 2>&1 | tee "$tmp/cargo.txt" \
    | sed '/^ *Running /d' || rc=$?
  gesehen="$(grep -c -E '^ +Running ' "$tmp/cargo.txt")"
  gemerkt="$(find "$tmp/l" -name lauf.json | wc -l | tr -d ' ')"
  if [ "$gesehen" != "$gemerkt" ] || [ "$gemerkt" -eq 0 ]; then
    echo "gates.sh: test: cargo named $gesehen programs, the runner wrote down $gemerkt" >&2
    [ "$rc" -ne 0 ] || rc=1
  fi
  echo "gates.sh: test: $gemerkt programs, $TEST_JOBS at once"
  python3 -I "$tmp/parallel.py" ordnen "$tmp" parallel $TEST_ZUERST -- $TEST_ALLEIN \
    | xargs -n 1 -P "$TEST_JOBS" python3 -I "$tmp/parallel.py" spielen "$var"
  if [ -n "$TEST_ALLEIN" ]; then
    python3 -I "$tmp/parallel.py" ordnen "$tmp" allein $TEST_ZUERST -- $TEST_ALLEIN \
      | xargs -n 1 -P 1 python3 -I "$tmp/parallel.py" spielen "$var"
  fi
  for r in "$tmp"/l/*; do
    [ -f "$r/lauf.json" ] || continue
    kopf="$(python3 -I "$tmp/parallel.py" kopf "$r" "$HERE")"
    printf '%s\n' "$kopf"
    cat "$r/aus" 2>/dev/null
    prc="$(cat "$r/rc" 2>/dev/null)"
    if [ "$prc" != 0 ]; then
      echo "gates.sh: test: exit ${prc:-none}: ${kopf#*Running }"
      [ "$rc" -ne 0 ] || rc=101
    fi
  done
  return "$rc"
}

gate_test() {
  need cargo || return
  local rc=0 crate tmp
  tmp="$(mktemp -d "${TMPDIR:-/tmp}/gates-test.XXXXXX")" || return 64
  test_parallel "$tmp" --workspace --all-targets --all-features --locked --no-fail-fast || rc=$?
  case "$tmp" in */gates-test.??????) rm -rf -- "$tmp" ;; esac
  for crate in $NAHT_CRATES; do
    case " $OHNE_NAHTTEST " in
      *" $crate "*)
        if [ ! -f "crates/$crate/tests/naht.rs" ]; then
          echo "gates.sh: test: $crate has no tests/naht.rs; its seam test without the feature is skipped"
          continue
        fi ;;
    esac
    cargo test --locked --no-fail-fast -p "$crate" --test naht || rc=$?
  done
  return "$rc"
}

gate_licenses() {
  need cargo || return
  need_subcommand deny || return
  need_subcommand cyclonedx || return
  cargo deny --locked --all-features check licenses bans sources || return

  # One SBOM per workspace crate. cargo-cyclonedx writes next to each Cargo.toml;
  # the files are moved to sbom/ so the source tree stays clean.
  rm -rf sbom && mkdir -p sbom || return
  cargo cyclonedx --all-features --format json --spec-version 1.6 --override-filename sbom.cdx || return
  local count=0 file crate
  for file in crates/*/sbom.cdx.json; do
    [ -f "$file" ] || continue
    crate="${file#crates/}"
    crate="${crate%%/*}"
    mv "$file" "sbom/$crate.cdx.json" || return
    count=$((count + 1))
  done
  local members
  members="$(find crates -mindepth 2 -maxdepth 2 -name Cargo.toml | wc -l | tr -d ' ')"
  if [ "$count" -ne "$members" ]; then
    echo "gates.sh: SBOM for $count of $members crates" >&2
    return 1
  fi
  for file in sbom/*.cdx.json; do
    if ! grep -q '"bomFormat": *"CycloneDX"' "$file"; then
      echo "gates.sh: $file is not a CycloneDX document" >&2
      return 1
    fi
  done
  echo "gates.sh: SBOM written for $count crates in sbom/"
}

# ---- notices gate ---------------------------------------------------------
# Why a gate of its own: deny.toml ignores the workspace crates (`private.ignore`), and cargo-cyclonedx writes
# no licence and no notice for them. Source that was copied or ported into a crate is therefore invisible to
# the licence gate and to the SBOM. README.md keeps the list (section "Code taken from other projects"); this
# gate holds the list to its promises, in both directions.
#
# Per row (columns Where | What | Origin | Licence | Notice):
#   1. the Where file (or directory) exists and lies inside a crate under crates/;
#   2. every file named in backticks in the Notice column exists, names the SPDX id of the Licence column
#      (the text before the first " (") and names the Where file relative to its crate;
#      "head of the file" in the Notice column means the first NOTICE_HEAD_LINES lines of the Where file
#      name the SPDX id;
#   3. each notice file lies inside the crate of the Where file and appears in `cargo package --list` of that
#      crate, as does the Where file itself. Why this proof and not the SBOM: the SBOM that `./gates.sh
#      licenses` writes holds neither a notice nor a licence for a workspace crate (measured 2026-11-18,
#      cargo-cyclonedx 0.5.10), so it cannot prove anything about a notice; the package listing is what cargo
#      ships, and it names the file.
# Reverse direction: a file under crates/*/src whose head (NOTICE_HEAD_LINES lines) carries a foreign licence
# marker and that no row covers is red. A file is own code, and is skipped, only if its head carries the licensor's
# copyright line (after the comment marker exactly OWN_COPYRIGHT, an optional full stop allowed), every line of the head
# that contains "Copyright" is such a line, and every SPDX-License-Identifier line names the own licence
# (OWN_SPDX, at least one). Since 2026-11-24 the own licence is Apache-2.0, the licence of much third-party code; an
# Apache head without the licensor's copyright line, or with a further copyright holder, stays foreign.
# A missing README.md, a missing section, a malformed header or no data row is red, never green.
# Output direction (notices_bins): a crate that has a bin target (cargo metadata, --locked) and a row is
# built with `cargo build --bins`; each of its bins must exit 0 on `--licenses`, and its output must contain
# every notice file of the crate's rows byte for byte. A library crate gets no run; its check is the listing.

NOTICE_HEAD_LINES=40
OWN_SPDX="Apache-2.0"
OWN_COPYRIGHT="Copyright Skryx-L-A"
NOTICE_SECTION="Code taken from other projects"
NOTICE_HEADER="Where|What|Origin|Licence|Notice"
FOREIGN_HEAD_PATTERN='SPDX-License-Identifier|Copyright[[:space:]]*(\(c\)|\(C\)|©|[0-9])|Licensed under|Redistribution and use in source|Permission is hereby granted|This Source Code Form|GNU (Lesser |Affero )?General Public|Apache License|MIT License|BSD [0-9]-Clause'

# Prints the data rows of the table as "where<US>licence<US>notice" (US = 0x1f), one per line.
# A malformed or empty table gives "ERR <text>" lines instead.
notices_rows() {
  awk -v section="$NOTICE_SECTION" -v want="$NOTICE_HEADER" '
    function trim(s) { gsub(/^[ \t]+|[ \t]+$/, "", s); return s }
    BEGIN { insec = 0; state = 0; rows = 0; found = 0 }
    /^#+[ \t]/ {
      insec = 0
      title = $0; sub(/^#+[ \t]+/, "", title)
      if (trim(title) == section) { insec = 1; found = 1 }
      next
    }
    insec && /^\|/ {
      n = split($0, c, "|")
      if (state == 0) {
        h = ""
        for (i = 2; i < n; i++) h = h (i > 2 ? "|" : "") trim(c[i])
        if (n != 7 || h != want) print "ERR the table header is not \"" want "\""
        state = 1; next
      }
      if (state == 1) {
        if ($0 !~ /^\|[ \t:|-]+\|?[ \t]*$/ || $0 !~ /-/) print "ERR the table has no separator row after the header"
        state = 2; next
      }
      rows++
      if (n != 7) { print "ERR row " rows " has " (n - 2) " columns, expected 5"; next }
      printf "%s\037%s\037%s\n", trim(c[2]), trim(c[5]), trim(c[6])
      next
    }
    END {
      if (!found) print "ERR README.md has no section \"" section "\""
      else if (rows == 0) print "ERR the table of section \"" section "\" has no data row"
    }
  ' README.md
}

# First backticked token of a string, without the backticks.
bt_first() { printf '%s\n' "$1" | sed -n 's/^[^`]*`\([^`]*\)`.*/\1/p'; }
# All backticked tokens of a string, one per line.
bt_all() { printf '%s\n' "$1" | grep -o '`[^`]*`' | tr -d '`'; }

# True if a row of the table names a directory that contains the file. $1 scratch dir, $2 file.
notices_dir_covered() {
  local c
  while IFS= read -r c; do
    [ -d "$c" ] || continue
    case "$2" in "${c%/}"/*) return 0 ;; esac
  done < "$1/covered"
  return 1
}

# Is path $3 in `cargo package --list` of crate $2? The list is cached per crate in scratch dir $1.
# $4 and $5 only name the row in the message.
notices_in_package() {
  local tmp="$1" crate="$2" want="$3" rowno="$4" wpath="$5" list
  list="$tmp/pkg.$crate"
  if [ ! -f "$list" ]; then
    if ! cargo package --list --locked --manifest-path "crates/$crate/Cargo.toml" > "$list" 2> "$list.err" < /dev/null; then
      echo "notices: row $rowno ($wpath): cargo package --list failed for crate $crate: $(tail -n 1 "$list.err")" >&2
      rm -f "$list"
      return 1
    fi
  fi
  if [ -d "crates/$crate/$want" ]; then
    grep -q -- "^${want%/}/" "$list" && return 0
  else
    grep -qxF -- "$want" "$list" && return 0
  fi
  echo "notices: row $rowno ($wpath): $want is not in the package of crate $crate (cargo package --list)" >&2
  return 1
}

# One row. $1 scratch dir, $2 row number, $3 Where cell, $4 Licence cell, $5 Notice cell. Returns 1 per defect.
notices_row() {
  local tmp="$1" rowno="$2" where="$3" lic="$4" notice="$5"
  local rc=0 wpath crate="" rel="" nfile nrel has_head=0 has_file=0
  wpath="$(bt_first "$where")"
  lic="${lic%% (*}"
  lic="$(printf '%s' "$lic" | tr -d '`')"
  if [ -z "$wpath" ] || [ -z "$lic" ]; then
    echo "notices: row $rowno: Where (a path in backticks) or Licence is empty" >&2
    return 1
  fi
  case "$wpath" in
    /* | *..*) echo "notices: row $rowno ($wpath): the path must be relative and stay inside the workspace" >&2; return 1 ;;
  esac
  printf '%s\n' "$wpath" >> "$tmp/covered"
  if [ ! -e "$wpath" ]; then
    echo "notices: row $rowno ($wpath): the listed file does not exist" >&2
    return 1
  fi
  case "$wpath" in
    crates/*/*) crate="${wpath#crates/}"; crate="${crate%%/*}"; rel="${wpath#crates/"$crate"/}" ;;
    *) echo "notices: row $rowno ($wpath): not inside a crate under crates/, so no package can carry its notice" >&2; rc=1 ;;
  esac
  if [ -n "$crate" ]; then printf '%s\t%s\t%s\n' "$crate" "$rowno" "$wpath" >> "$tmp/rowcrates"; fi
  if [ -z "$notice" ]; then
    echo "notices: row $rowno ($wpath): the Notice column is empty" >&2
    return 1
  fi
  if grep -qi 'head of the file' <<<"$notice"; then has_head=1; fi
  bt_all "$notice" > "$tmp/nfiles"
  while IFS= read -r nfile; do
    [ -n "$nfile" ] || continue
    has_file=1
    if [ ! -f "$nfile" ]; then
      echo "notices: row $rowno ($wpath): notice file $nfile does not exist" >&2; rc=1; continue
    fi
    if [ -n "$crate" ]; then printf '%s\t%s\t%s\t%s\n' "$crate" "$rowno" "$wpath" "$nfile" >> "$tmp/bin_notices"; fi
    if ! grep -qF -- "$lic" "$nfile"; then
      echo "notices: row $rowno ($wpath): $nfile does not name the licence $lic" >&2; rc=1
    fi
    if [ -n "$rel" ] && ! grep -qF -- "$rel" "$nfile"; then
      echo "notices: row $rowno ($wpath): $nfile does not name the file $rel" >&2; rc=1
    fi
    if [ -n "$crate" ]; then
      case "$nfile" in
        crates/"$crate"/*)
          nrel="${nfile#crates/"$crate"/}"
          notices_in_package "$tmp" "$crate" "$nrel" "$rowno" "$wpath" || rc=1 ;;
        *) echo "notices: row $rowno ($wpath): $nfile lies outside the crate $crate and ships in no package of it" >&2; rc=1 ;;
      esac
    fi
  done < "$tmp/nfiles"
  if [ "$has_head" -eq 1 ]; then
    if [ -d "$wpath" ]; then
      echo "notices: row $rowno ($wpath): \"head of the file\" cannot apply to a directory" >&2; rc=1
    elif ! grep -qF -- "$lic" <<<"$(head -n "$NOTICE_HEAD_LINES" "$wpath")"; then
      echo "notices: row $rowno ($wpath): the first $NOTICE_HEAD_LINES lines do not name the licence $lic" >&2; rc=1
    fi
  fi
  if [ "$has_head" -eq 0 ] && [ "$has_file" -eq 0 ]; then
    echo "notices: row $rowno ($wpath): the Notice column names neither a file in backticks nor the head of the file" >&2; rc=1
  fi
  # The listed file must ship, too.
  if [ -n "$crate" ]; then
    notices_in_package "$tmp" "$crate" "$rel" "$rowno" "$wpath" || rc=1
  fi
  return "$rc"
}

# Reverse direction: files under crates/*/src with a foreign licence head that no row covers.
notices_reverse() {
  local tmp="$1" rc=0 f hit id own line
  find crates -mindepth 3 -path 'crates/*/src/*' -type f 2>/dev/null | LC_ALL=C sort > "$tmp/srcfiles"
  while IFS= read -r f; do
    grep -qI . "$f" 2>/dev/null || continue
    if grep -qxF -- "$f" "$tmp/covered" || notices_dir_covered "$tmp" "$f"; then continue; fi
    head -n "$NOTICE_HEAD_LINES" "$f" > "$tmp/head"
    hit="$(grep -E -m1 -- "$FOREIGN_HEAD_PATTERN" "$tmp/head")" || continue
    own=1
    grep -o 'SPDX-License-Identifier:[[:space:]]*[^[:space:]]*' "$tmp/head" | sed 's/^[^:]*:[[:space:]]*//' > "$tmp/ids"
    [ -s "$tmp/ids" ] || own=0
    while IFS= read -r id; do
      [ "$id" = "$OWN_SPDX" ] || own=0
    done < "$tmp/ids"
    # Copyright lines: each one must be exactly the own line after the comment marker (an optional final full stop).
    grep -F -- "Copyright" "$tmp/head" | sed 's/^[[:space:]/*#!;-]*//; s/[[:space:]]*$//' > "$tmp/copy" || true
    [ -s "$tmp/copy" ] || own=0
    while IFS= read -r line; do
      [ "$line" = "$OWN_COPYRIGHT" ] || [ "$line" = "$OWN_COPYRIGHT." ] || own=0
    done < "$tmp/copy"
    [ "$own" -eq 1 ] && continue
    echo "notices: $f carries a foreign licence head ($(printf '%s' "$hit" | cut -c1-60)) and no row of \"$NOTICE_SECTION\" in README.md lists it" >&2
    rc=1
  done < "$tmp/srcfiles"
  return "$rc"
}

# Metadata of the workspace: "TARGET<TAB>target directory" and "BIN<TAB>crate directory<TAB>bin name" per bin.
# Paths are relative to the working directory, so they match the crate paths of the table.
NOTICES_BINS_PY='import json, os, re, sys
m = json.load(open(sys.argv[1], encoding="utf-8"))
cwd = os.path.realpath(os.getcwd())
print("TARGET\t" + m["target_directory"])
for p in m["packages"]:
    d = os.path.relpath(os.path.realpath(os.path.dirname(p["manifest_path"])), cwd)
    for t in p["targets"]:
        if "bin" in t["kind"]:
            print("BIN\t" + d + "\t" + t["name"])'

# Exit 0 if the notice file (argv 1) is non-empty and occurs in the output file (argv 2), byte for byte.
NOTICES_CONTAINS_PY='import sys
notice = open(sys.argv[1], "rb").read()
out = open(sys.argv[2], "rb").read()
sys.exit(0 if notice and notice in out else 1)'

# Output direction. $1 scratch dir. Returns 1 per defect, 127 when python3 is missing.
notices_bins() {
  local tmp="$1" rc=0 crate bins bin tdir rowno wpath code c nrowno nwpath nfile
  [ -s "$tmp/rowcrates" ] || return 0
  if ! command -v python3 >/dev/null 2>&1; then
    echo "gates.sh: python3 not found (see README.md, section Tools)" >&2
    return 127
  fi
  if ! cargo metadata --no-deps --format-version 1 --locked > "$tmp/meta.json" 2> "$tmp/meta.err"; then
    echo "notices: cargo metadata failed: $(tail -n 1 "$tmp/meta.err")" >&2
    return 1
  fi
  if ! python3 -I -c "$NOTICES_BINS_PY" "$tmp/meta.json" > "$tmp/bins" 2> "$tmp/bins.err"; then
    echo "notices: cargo metadata cannot be read: $(tail -n 1 "$tmp/bins.err")" >&2
    return 1
  fi
  tdir="$(awk -F '\t' '$1 == "TARGET" { print $2; exit }' "$tmp/bins")"
  for crate in $(cut -f1 "$tmp/rowcrates" | LC_ALL=C sort -u); do
    bins="$(awk -F '\t' -v d="crates/$crate" '$1 == "BIN" && $2 == d { print $3 }' "$tmp/bins")"
    [ -n "$bins" ] || continue
    rowno="$(awk -F '\t' -v c="$crate" '$1 == c { print $2; exit }' "$tmp/rowcrates")"
    wpath="$(awk -F '\t' -v c="$crate" '$1 == c { print $3; exit }' "$tmp/rowcrates")"
    if ! cargo build --locked --bins --manifest-path "crates/$crate/Cargo.toml" > "$tmp/build.log" 2>&1; then
      echo "notices: row $rowno ($wpath): cargo build of crate $crate failed: $(tail -n 1 "$tmp/build.log")" >&2
      rc=1
      continue
    fi
    while IFS= read -r bin; do
      [ -n "$bin" ] || continue
      code=0
      "$tdir/debug/$bin" --licenses < /dev/null > "$tmp/out" 2> "$tmp/err" || code=$?
      if [ "$code" -ne 0 ]; then
        echo "notices: row $rowno ($wpath): $bin --licenses exits $code: $(tail -n 1 "$tmp/err")" >&2
        rc=1
        continue
      fi
      while IFS=$'\t' read -r c nrowno nwpath nfile; do
        [ "$c" = "$crate" ] || continue
        if ! python3 -I -c "$NOTICES_CONTAINS_PY" "$nfile" "$tmp/out"; then
          echo "notices: row $nrowno ($nwpath): $bin --licenses does not print $nfile byte for byte" >&2
          rc=1
        fi
      done < "$tmp/bin_notices"
    done <<< "$bins"
  done
  return "$rc"
}

notices_check() {
  local tmp="$1" rc=0 rowno=0 where lic notice
  if [ ! -f README.md ]; then
    echo "notices: README.md not found next to gates.sh" >&2
    return 1
  fi
  notices_rows > "$tmp/rows"
  if grep -q '^ERR ' "$tmp/rows"; then
    sed -n 's/^ERR /notices: /p' "$tmp/rows" >&2
    return 1
  fi
  : > "$tmp/covered"
  : > "$tmp/rowcrates"
  : > "$tmp/bin_notices"
  while IFS=$'\037' read -r where lic notice; do
    rowno=$((rowno + 1))
    notices_row "$tmp" "$rowno" "$where" "$lic" "$notice" || rc=1
  done < "$tmp/rows"
  notices_reverse "$tmp" || rc=1
  notices_bins "$tmp" || rc=$?
  return "$rc"
}

gate_notices() {
  need cargo || return
  local tmp rc=0
  tmp="$(mktemp -d "${TMPDIR:-/tmp}/gates-notices.XXXXXX")" || return 64
  notices_check "$tmp" || rc=$?
  case "$tmp" in */gates-notices.??????) rm -rf -- "$tmp" ;; esac
  if [ "$rc" -eq 0 ]; then
    echo "gates.sh: notices ok"
  fi
  return "$rc"
}

# ---- mutation gate ----------------------------------------------------------
# The frame of the mutation gate (decision of 25.11.2026):
# - MUTANTS_PFLICHT are the security crates. Each of their runs must end with 0 missed, 0 timeouts and at least one
#   caught mutant; otherwise the gate is red. Every other workspace crate is a report crate: its missed mutants go into
#   target/mutants/bericht.tsv and the log, and the gate stays green. The report crates come from `cargo metadata`, never
#   from a list by hand, so a new crate lands among them and the log names it. A security crate that `cargo metadata`
#   does not know (renamed, removed) is red.
# - Never one run over the whole workspace: one `cargo mutants -p <crate>` per crate, one after the other, each with its
#   own output folder target/mutants/<crate> (the seam run: target/mutants-naht/<crate>). This script never starts two
#   mutation runs at the same time. On peer the board (wb-tafel) holds the one mutation run of the machine; whoever
#   starts the gate there takes the place first.
# - Only changed crates run. WB_MUTANTS_BASIS names a git ref; the crates are those whose folder holds a file that
#   differs from that ref (working tree against the ref, untracked files included), together with every crate that uses
#   one of them (a path dependency in its Cargo.toml, or a text `../<folder>/` in one of its files, as with
#   `include_str!`). A changed file below this folder but outside every crate folder (spec/, ansicht/, README.md, ...)
#   selects the crates whose files (source, tests, build.rs, benches, README, case files) name its path relative to this
#   folder, or name a folder above it as a whole path (`../../spec/start` for spec/start/README.md; a crate that loops
#   over the folder reads the file), together with the crates that use them; the log gives each crate the first place
#   (`reads spec/start/ at crates/start/tests/faelle.rs:109`). Comments and prose count, because the search cannot tell
#   them from code: when in doubt, more crates run. A single word (`spec`, `README.md`) is a path only after `../` or
#   `rust/`, or in quotes on a line that builds a path. When no crate names the file or a folder above it, every
#   security crate runs (fail closed). Cargo.lock, Cargo.toml, gates.sh, deny.toml, rust-toolchain*, everything below
#   .cargo/ and everything below crates/ that no crate of `cargo metadata` owns select every security crate at once,
#   whatever a crate names. Files outside this folder are not in the copy that cargo-mutants tests, so they select
#   nothing. A changed list of exclusions mutants/<folder>.liste selects the crate of that folder, in full (a list
#   with no crate of its name selects every security crate).
# - The base is meant as a fork point: the change is the working tree against `git merge-base HEAD <base>`, so newer
#   commits of the base branch do not count. When there is no merge base, every security crate runs in full (fail closed).
# - With a base, each chosen crate runs `cargo mutants -p <crate> --in-diff target/mutants-diff.patch`: only the mutants on
#   the changed lines (the diff of the same change, untracked files included), also for the second run with the feature
#   `pruefbau`. A crate chosen only because it reads or uses a changed file has no mutant in the diff and ends green with
#   a note. A crate chosen by a shared file (Cargo.lock, gates.sh, ...) or because no crate names the file runs in full:
#   a diff of such a file holds no mutant, and that case fails closed.
# - A TIMEOUT counts as caught only when it is listed in MUTANTS_TIMEOUT_BEGRUENDET (exact mutant name, with its reason)
#   and the log of that mutant (mutants.out/log/) holds the line of a failed test (`test ... FAILED`): the mutant makes the
#   test process hang after a test has already failed. An entry whose name no longer fits (line moved) counts for nothing.
# - Without WB_MUTANTS_BASIS every security crate runs: this is the run before a state is published. When the change
#   cannot be determined (no git, an unknown ref, a failing diff), every security crate runs as well (fail closed).
#   Report crates run only when they changed against a base.
# - Windows code is left out in .cargo/mutants.toml; its mutants run on the Windows test machine, before every delivery
#   under Windows.
MUTANTS_PFLICHT="anmelden einschluss stelle datentor modellweg halter dienst peerkanal werkbank-pruefen lauf vorgabe-helfer"

# The merge base of HEAD and WB_MUTANTS_BASIS, set by mutants_plan; empty means every run is a full run.
MUTANTS_GABEL=""

# TIMEOUTs that count as caught (see the header above). Each entry is the exact name of the mutant as cargo-mutants prints it
# in timeout.txt, with its reason and the measurement. A line that moved makes the entry useless: the TIMEOUT is red again;
# the entry is then moved to the new line (`./gates.sh mutants-list`). Add an entry only with the proof of a run.
MUTANTS_TIMEOUT_BEGRUENDET=(
  # halter, `Dienst::anfrage -> bool with true`: the service then accepts every request and keeps the connections open, so
  # the test process does not end; its log already holds failed tests (annahme_bedient_haelt_die_grenze_und_endet_mit_halt
  # and five more). The hang is the right catch; the failed tests prove it. Measured in the full run of halter without the
  # feature (results of the halter mutant run, section "TIMEOUT mit Grund"); the line is that of the list at the state of
  # 25.11.2026.
  'crates/halter/src/dienst.rs:378:9: replace Dienst::anfrage -> bool with true'
)

# argv: cargo metadata (JSON), then `alle` or a file with the changed paths (relative to this folder), then
# MUTANTS_PFLICHT. Prints FEHLT <crate> per unknown security crate, LISTE <crate> per report crate, then
# WAHL <crate> <reason> per crate to run: security crates in the order of MUTANTS_PFLICHT, then report crates.
MUTANTS_PLAN_PY='import json, os, re, sys
m = json.load(open(sys.argv[1], encoding="utf-8"))
mode = sys.argv[2]
pflicht = sys.argv[3].split()
root = os.path.realpath(m["workspace_root"])
ids = set(m["workspace_members"])
pakete = [p for p in m["packages"] if p["id"] in ids]
crates = {}
for p in pakete:
    crates[p["name"]] = os.path.relpath(os.path.realpath(os.path.dirname(p["manifest_path"])), root)
for n in pflicht:
    if n not in crates:
        print("FEHLT\t" + n)
bericht = sorted(n for n in crates if n not in pflicht)
for n in bericht:
    print("LISTE\t" + n)
reihe = [n for n in pflicht if n in crates] + bericht
if mode == "alle":
    for n in pflicht:
        if n in crates:
            print("WAHL\t" + n + "\tno base: every security crate")
    sys.exit(0)
def immer_alle(f):
    # Files that steer the build or this gate itself: no text search, whatever a crate names.
    name = f.split("/")[-1]
    if f.startswith(".cargo/") or f.startswith("crates/") or name in ("Cargo.toml", "Cargo.lock"):
        return True
    return "/" not in f and (name in ("gates.sh", "deny.toml") or name.startswith("rust-toolchain"))
grund = {}
alle = None
fremd = []
for f in open(mode, encoding="utf-8").read().splitlines():
    if not f:
        continue
    # The exclusions of one crate (mutants/<folder>.liste): that crate runs, in full; an unknown name selects every
    # security crate (gate_mutants then refuses the file).
    liste = re.fullmatch(r"mutants/([A-Za-z0-9_-]+)\.liste", f)
    if liste:
        eigen = [n for n, d in crates.items() if d == "crates/" + liste.group(1)]
        for n in eigen:
            grund.setdefault(n, "exclusion list changed: " + f)
        if not eigen and alle is None:
            alle = "shared file changed: " + f + " (no crate of this name)"
        continue
    treffer = [n for n, d in crates.items() if f.startswith(d + "/")]
    for n in treffer:
        grund.setdefault(n, "changed: " + f)
    if not treffer:
        if immer_alle(f):
            if alle is None:
                alle = "shared file changed: " + f
        else:
            fremd.append(f)
texte = {}
for n, d in crates.items():
    texte[n] = []
    for ordner, unter, dateien in os.walk(os.path.join(root, d)):
        unter[:] = sorted(u for u in unter if u != "target")
        for datei in sorted(dateien):
            try:
                text = open(os.path.join(ordner, datei), encoding="utf-8", errors="replace").read()
            except OSError:
                continue
            texte[n].append((os.path.relpath(os.path.join(ordner, datei), root), text))
def nadeln(f):
    # The file itself, then every folder above it (below rust/), the nearest first.
    teile = f.split("/")
    return [(f, True)] + [("/".join(teile[:i]), False) for i in range(len(teile) - 1, 0, -1)]
def muster(nadel, datei):
    # A path is named when it is not the tail of a longer name (no name character before it), and for a folder
    # when no further name follows it. A single word (spec, README.md) is a path only after ../ or rust/, or in
    # quotes on a line that builds a path (see suche); otherwise a JSON key or a word of prose would count.
    vor = "(?<![A-Za-z0-9_\\-])" if "/" in nadel else "(?:(?<=\\.\\./)|(?<=rust/)|(?<=[\"\\x27`]))"
    nach = "(?![A-Za-z0-9_\\-])" if datei else "(?!/?[A-Za-z0-9_\\-])"
    return re.compile(vor + re.escape(nadel) + nach)
def suche(mu, text, einzeln):
    for m in mu.finditer(text):
        vorn = text[max(0, m.start() - 5):m.start()]
        if not einzeln or vorn.endswith("../") or vorn.endswith("rust/"):
            return m
        a = text.rfind("\n", 0, m.start()) + 1
        b = text.find("\n", m.end())
        zeile = text[a:] if b < 0 else text[a:b]
        if any(k in zeile for k in ("join(", "Path", "push(", "MANIFEST_DIR")):
            return m
    return None
gelesen = {}
def leser(f):
    treffer = {}
    for nadel, datei in nadeln(f):
        if nadel not in gelesen:
            mu = muster(nadel, datei)
            gelesen[nadel] = {}
            for n in crates:
                for pfad, text in texte[n]:
                    if nadel not in text:
                        continue
                    m = suche(mu, text, "/" not in nadel)
                    if m:
                        gelesen[nadel][n] = "reads " + nadel + ("" if datei else "/") + " at " + pfad + ":" + str(text.count("\n", 0, m.start()) + 1)
                        break
        for n, g in gelesen[nadel].items():
            treffer.setdefault(n, g)
    return treffer
for f in fremd:
    gefunden = leser(f)
    if not gefunden and alle is None:
        alle = "shared file changed: " + f + " (no crate names it)"
    for n, g in gefunden.items():
        grund.setdefault(n, g)
uses = {n: set() for n in crates}
for p in pakete:
    for d in p["dependencies"]:
        if d.get("path"):
            dp = os.path.relpath(os.path.realpath(d["path"]), root)
            for q, qd in crates.items():
                if qd == dp and q != p["name"]:
                    uses[p["name"]].add(q)
for n in crates:
    marken = [(q, "../" + os.path.basename(qd) + "/") for q, qd in crates.items() if q != n]
    for pfad, text in texte[n]:
        for q, marke in marken:
            if marke in text:
                uses[n].add(q)
neu = True
while neu:
    neu = False
    for n in crates:
        if n in grund:
            continue
        for q in sorted(uses[n]):
            if q in grund:
                grund[n] = "uses " + q
                neu = True
                break
if alle is not None:
    for n in pflicht:
        if n in crates:
            grund.setdefault(n, alle)
for n in reihe:
    if n in grund:
        print("WAHL\t" + n + "\t" + grund[n])'

# Writes the plan to $1/plan (lines of MUTANTS_PLAN_PY). Returns 1 when the crates cannot be listed.
mutants_plan() {
  local tmp="$1" basis="${WB_MUTANTS_BASIS:-}" mode=alle gabel
  MUTANTS_GABEL=""
  if ! command -v python3 >/dev/null 2>&1; then
    echo "gates.sh: python3 not found (see README.md, section Tools)" >&2
    return 127
  fi
  if ! cargo metadata --no-deps --format-version 1 --locked > "$tmp/meta.json" 2> "$tmp/meta.err"; then
    echo "gates.sh: cargo metadata failed: $(tail -n 1 "$tmp/meta.err")" >&2
    return 1
  fi
  if [ -z "$basis" ]; then
    echo "gates.sh: mutants: no WB_MUTANTS_BASIS, every security crate runs"
  elif ! git rev-parse --verify --quiet "$basis^{commit}" > /dev/null 2>&1; then
    echo "gates.sh: mutants: WB_MUTANTS_BASIS '$basis' does not resolve to a commit; every security crate runs (fail closed)"
  elif ! gabel="$(git merge-base HEAD "$basis" 2> "$tmp/gabel.err")" || [ -z "$gabel" ]; then
    echo "gates.sh: mutants: HEAD and '$basis' have no merge base ($(tail -n 1 "$tmp/gabel.err")); every security crate runs, in full (fail closed)"
  elif ! { git diff --name-only --no-renames --relative "$gabel" -- && git ls-files --others --exclude-standard; } \
      > "$tmp/changed" 2> "$tmp/changed.err"; then
    echo "gates.sh: mutants: the change against '$basis' cannot be determined ($(tail -n 1 "$tmp/changed.err")); every security crate runs, in full (fail closed)"
  else
    mode="$tmp/changed"
    MUTANTS_GABEL="$gabel"
    echo "gates.sh: mutants: base $basis, merge base ${gabel:0:12}, $(grep -c . "$tmp/changed") changed files below rust/"
  fi
  if ! python3 -I -c "$MUTANTS_PLAN_PY" "$tmp/meta.json" "$mode" "$MUTANTS_PFLICHT" > "$tmp/plan" 2> "$tmp/plan.err"; then
    echo "gates.sh: cargo metadata cannot be read: $(tail -n 1 "$tmp/plan.err")" >&2
    return 1
  fi
}

# Writes the diff of the change (merge base against the working tree below this folder, untracked files as new files) to $1.
# The paths are relative to this folder, as cargo-mutants names its mutants. The prefixes are fixed to a/ and b/: cargo-mutants
# strips only those, and a git configuration such as diff.mnemonicPrefix (prefixes c/, w/, i/) or diff.noprefix would leave a
# path that names no file, so that --in-diff finds no mutant and the run ends green without testing anything (measured on
# 25.11.2026: 0 instead of 33 mutants of dienst). External diff programs and colour are switched off for the same reason.
# Afterwards every header of a file must carry these prefixes (or /dev/null); otherwise the diff is refused (fail closed).
mutants_diff() {
  local out="$1" f
  local fest=(--no-ext-diff --no-color --no-textconv --src-prefix=a/ --dst-prefix=b/)
  mkdir -p "$(dirname "$out")" || return
  git diff "${fest[@]}" --no-renames --relative "$MUTANTS_GABEL" -- . > "$out" || return
  while IFS= read -r f; do
    [ -n "$f" ] || continue
    git diff "${fest[@]}" --no-index -- /dev/null "$f" >> "$out" || [ "$?" -eq 1 ] || return
  done < <(git ls-files --others --exclude-standard)
  mutants_diff_pruefen "$out"
}

# Checks the headers of the diff in $1: in each file section (from `diff --git` to its first `@@`), a line `--- ` must name
# a/<path> or /dev/null and a line `+++ ` must name b/<path> or /dev/null. Lines inside the hunks are not headers (a removed
# line `-- x` reads `--- x` there) and are not checked.
mutants_diff_pruefen() {
  local schlecht
  schlecht="$(awk '
    /^diff --git / { kopf = 1; next }
    /^@@/ { kopf = 0; next }
    kopf && /^--- / && $0 !~ /^--- (a\/|\/dev\/null$)/ { print NR ": " $0; next }
    kopf && /^\+\+\+ / && $0 !~ /^\+\+\+ (b\/|\/dev\/null$)/ { print NR ": " $0 }
  ' "$1")" || return
  if [ -n "$schlecht" ]; then
    echo "gates.sh: mutants: the diff $1 has a header without the prefix a/ or b/; cargo-mutants would find no mutant (fail closed):" >&2
    printf '%s\n' "$schlecht" | head -n 5 >&2
    return 1
  fi
}

# Counts the changed lines of code of crate $1 in the diff $2: added lines in a .rs file below the folder of the crate, outside
# the exclude_globs of .cargo/mutants.toml, that are not blank, not a comment, not an attribute, not a `use` or `mod` line and
# not only brackets or punctuation. Prints the number (0 when the crate cannot be found: the caller then treats it as changed).
mutants_code_zeilen() {
  cargo metadata --no-deps --format-version 1 2>/dev/null | python3 -I -c '
import fnmatch, json, os, re, sys
crate, patch = sys.argv[1], sys.argv[2]
try:
    meta = json.load(sys.stdin)
except Exception:
    print(-1); sys.exit(0)
wurzel = os.getcwd()
ordner = None
for p in meta.get("packages", []):
    if p.get("name") == crate:
        ordner = os.path.relpath(os.path.dirname(p["manifest_path"]), wurzel)
if ordner is None:
    print(-1); sys.exit(0)
globs = []
try:
    import tomllib
    with open(".cargo/mutants.toml", "rb") as f:
        globs = tomllib.load(f).get("exclude_globs", [])
except Exception:
    pass
leer = re.compile(r"^\s*$|^\s*(//|/\*|\*|#!?\[)|^\s*(pub(\([^)]*\))?\s+)?(use|mod)\s|^[\s{}()\[\];,]*$")
n, datei, kopf = 0, None, False
for zeile in open(patch, encoding="utf-8", errors="replace"):
    if zeile.startswith("diff --git "):
        kopf, datei = True, None
        continue
    if kopf and zeile.startswith("+++ "):
        pfad = zeile[4:].rstrip("\n")
        datei = pfad[2:] if pfad.startswith("b/") else None
        continue
    if zeile.startswith("@@"):
        kopf = False
        continue
    if kopf or datei is None or not datei.endswith(".rs"):
        continue
    if not (datei + "/").startswith(ordner.rstrip("/") + "/"):
        continue
    if any(fnmatch.fnmatch(datei, g) for g in globs):
        continue
    if zeile.startswith("+") and not leer.match(zeile[1:]):
        n += 1
print(n)
' "$1" "$2"
}

# Keeps the first non-zero exit code in rc of gate_mutants.
mutants_keep() {
  [ "$rc" -ne 0 ] || rc="$1"
}

mutants_is_pflicht() {
  case " $MUTANTS_PFLICHT " in *" $1 "*) return 0 ;; esac
  return 1
}

mutants_is_naht() {
  case " $NAHT_CRATES " in *" $1 "*) return 0 ;; esac
  return 1
}

# One run: $1 crate, $2 output folder, $3 mode (run|list), then the arguments for cargo-mutants. Returns its exit code.
mutants_one() {
  local crate="$1" out="$2" mode="$3"
  shift 3
  # cargo-mutants does not create the parent of its output folder, and target/ may not exist yet.
  mkdir -p "$out" || return
  if [ "$mode" = list ]; then
    cargo mutants -p "$crate" --no-shuffle "$@" --list > "$out/liste.txt" || return
    echo "gates.sh: list: $crate $out $(grep -c . "$out/liste.txt") mutants"
    return 0
  fi
  cargo mutants -p "$crate" --no-shuffle "$@" --output "$out"
}

# Counts the TIMEOUTs of the run in $1 that count as caught: each one is in MUTANTS_TIMEOUT_BEGRUENDET, and the log of its
# mutant holds the line of a failed test. Prints the number; returns 1 when any TIMEOUT does not count.
mutants_timeouts_gefangen() {
  local out="$1" name eintrag log gelistet n=0
  while IFS= read -r name; do
    [ -n "$name" ] || continue
    gelistet=0
    for eintrag in "${MUTANTS_TIMEOUT_BEGRUENDET[@]}"; do
      [ "$eintrag" = "$name" ] && gelistet=1
    done
    [ "$gelistet" -eq 1 ] || return 1
    # The first line of the log of a mutant is `*** <name>`; the file name does not carry the whole name.
    log="$(grep -l -F -x -e "*** $name" -- "$out"/mutants.out/log/*.log 2>/dev/null | head -n 1)"
    [ -n "$log" ] || return 1
    grep -q -E '^test .* \.\.\. FAILED$' "$log" || return 1
    n=$((n + 1))
  done < "$out/mutants.out/timeout.txt"
  echo "$n"
}

# The result of the run of a security crate: $1 crate, $2 output folder, $3 exit code of the run, $4 label, $5 `diff` when
# the run was limited to the diff.
mutants_pflicht_result() {
  local crate="$1" out="$2" code="$3" label="$4" diff="${5:-}" caught missed timeout gefangen note=""
  caught="$(grep -c . "$out/mutants.out/caught.txt" 2>/dev/null)" || caught=0
  missed="$(grep -c . "$out/mutants.out/missed.txt" 2>/dev/null)" || missed=0
  timeout="$(grep -c . "$out/mutants.out/timeout.txt" 2>/dev/null)" || timeout=0
  # Exit 3 is the exit of timeouts without a missed mutant. When every timeout is listed and its log holds a failed test,
  # the run counts as green and those mutants as caught.
  if [ "$code" -eq 3 ] && [ "$missed" -eq 0 ] && [ "$timeout" -gt 0 ] && gefangen="$(mutants_timeouts_gefangen "$out")"; then
    code=0
    caught=$((caught + gefangen))
    note=" ($gefangen timeouts count as caught: listed, a failed test in the log)"
  fi
  if [ "$code" -ne 0 ]; then
    echo "gates.sh: mutants: security crate $crate$label red: exit $code, $missed missed, $timeout timeouts ($out/mutants.out)" >&2
    return "$code"
  fi
  # A green run must have caught at least one mutant; otherwise it proved nothing. Limited to the diff, a crate can have
  # no mutant on the changed lines (it only reads or uses a changed file); that ends green with a note.
  if [ "$caught" -lt 1 ]; then
    if [ -n "$diff" ]; then
      # No mutant although the crate has changed lines of code: the diff did not reach cargo-mutants, or every mutant was
      # unviable. Either way the run proved nothing about the change (fail closed); a full run (without WB_MUTANTS_BASIS)
      # decides. A crate without changed code (it only uses a changed crate, or only comments changed) ends green.
      local zeilen
      zeilen="$(mutants_code_zeilen "$crate" "$diff_datei")" || zeilen=-1
      if [ "$zeilen" != 0 ]; then
        echo "gates.sh: mutants: security crate $crate$label red: no mutant tested in the diff, but ${zeilen/#-1/an unknown number of} changed lines of code; run it in full (without WB_MUTANTS_BASIS)" >&2
        return 1
      fi
      echo "gates.sh: mutants: security crate $crate$label: no mutant caught in the diff, none missed (no changed line of code)"
      return 0
    fi
    echo "gates.sh: mutants: security crate $crate$label: no mutant caught; the run proved nothing" >&2
    return 1
  fi
  echo "gates.sh: mutants: security crate $crate$label: $caught mutants caught, none missed$note"
}

# The result of the run of a report crate: missed mutants and timeouts go into the report; only a failed run is red.
mutants_bericht_result() {
  local crate="$1" out="$2" code="$3" bericht="$4" art datei zeile n=0
  case "$code" in
    0|2|3) ;;
    *) echo "gates.sh: mutants: the run of report crate $crate failed: exit $code ($out/mutants.out)" >&2; return "$code" ;;
  esac
  for art in missed timeout; do
    datei="$out/mutants.out/$art.txt"
    [ -f "$datei" ] || continue
    while IFS= read -r zeile; do
      [ -n "$zeile" ] || continue
      printf '%s\t%s\t%s\n' "$crate" "$art" "$zeile" >> "$bericht"
      echo "gates.sh: mutants: report $crate $art: $zeile"
      n=$((n + 1))
    done < "$datei"
  done
  echo "gates.sh: mutants: report crate $crate: $n missed or timed out (not blocking)"
}

# Sets DARGS (an array of gate_mutants' scope, read by name) to the arguments that limit a run of crate $1 to the diff:
# empty without a base and for a crate in voll_wahl.
mutants_diff_args() {
  DARGS=()
  [ -n "$MUTANTS_GABEL" ] || return 0
  case " $voll_wahl " in *" $1 "*) return 0 ;; esac
  DARGS=(--in-diff "$diff_datei")
}

# Reads every mutants/<crate>.liste into the lists erster, gleichwertig and testhilfe of gate_mutants (arrays of its
# scope, extended by name). One entry per line, `<list> <kind> <pattern>`; `#` starts a comment line, empty lines are
# skipped (mutants/README.md). Red (1), with file and line: a file whose name is no crate folder below crates/, an unknown
# list or kind, an empty pattern, a pattern that does not start with crates/<crate>/ of its own file (also after `^`), or a
# carriage return.
mutants_listen_lesen() {
  local datei crate nr zeile wahl rest art muster opt rc=0
  for datei in mutants/*.liste; do
    [ -f "$datei" ] || continue
    crate="$(basename "$datei" .liste)"
    if [ ! -d "crates/$crate" ]; then
      echo "gates.sh: mutants: $datei names no crate folder crates/$crate" >&2
      rc=1
      continue
    fi
    nr=0
    while IFS= read -r zeile || [ -n "$zeile" ]; do
      nr=$((nr + 1))
      case "$zeile" in '' | '#'*) continue ;; *$'\r'*) echo "gates.sh: mutants: $datei:$nr: carriage return" >&2; rc=1; continue ;; esac
      wahl="${zeile%% *}"; rest="${zeile#* }"; art="${rest%% *}"; muster="${rest#* }"
      if [ "$wahl" = "$zeile" ] || [ "$art" = "$rest" ] || [ -z "$muster" ]; then
        echo "gates.sh: mutants: $datei:$nr: not <list> <kind> <pattern>" >&2; rc=1; continue
      fi
      case "$muster" in
        crates/"$crate"/* | '^crates/'"$crate"/*) ;;
        *) echo "gates.sh: mutants: $datei:$nr: the pattern does not start with crates/$crate/" >&2; rc=1; continue ;;
      esac
      case "$art" in
        re) opt=--exclude-re ;;
        datei) opt=--exclude ;;
        *) echo "gates.sh: mutants: $datei:$nr: unknown kind '$art' (re, datei)" >&2; rc=1; continue ;;
      esac
      case "$wahl" in
        erster) erster+=("$opt" "$muster") ;;
        gleichwertig) gleichwertig+=("$opt" "$muster") ;;
        testhilfe) testhilfe+=("$opt" "$muster") ;;
        *) echo "gates.sh: mutants: $datei:$nr: unknown list '$wahl' (erster, gleichwertig, testhilfe)" >&2; rc=1 ;;
      esac
    done < "$datei"
  done
  return "$rc"
}

# $1 is `run` (the gate), `plan` (prints the plan, starts no cargo-mutants) or `list` (cargo mutants --list per run).
gate_mutants() {
  local mode="${1:-run}"
  need cargo || return
  if [ "$mode" != plan ]; then
    need_subcommand mutants || return
  fi
  local rc=0
  # Three lists of exclusions. Their entries stand in mutants/<crate>.liste, one file per crate (since 25.11.2026: three
  # branches met in this file on one day); mutants/README.md has the format. Each entry has its reason in its file.
  # - erster: the first run builds the delivered state, without the feature `pruefbau`. cargo-mutants reads the source
  #   as text and does not evaluate `cfg`, so a mutant in code that stands only behind `cfg(feature = "pruefbau")` is
  #   not compiled here, no test can fail for it, and it would always survive. These places are left out of this run,
  #   and of this run only; the second run below covers them with the feature (no exclusion there). Code behind
  #   `cfg(not(feature = "pruefbau"))` and code without `cfg` stays in this run.
  # - gleichwertig: equivalent mutants, left out of both runs (decision of 25.11.2026, list per place).
  # - testhilfe: test help that is never delivered, left out of every run (also the second one).
  # A pattern with a line number that no longer fits leaves its mutant in the run, which then reports it as missed (the
  # run fails closed); the entry is then moved to the new line.
  local erster=() gleichwertig=() testhilfe=()
  mutants_listen_lesen || return 1
  local tmp code crate grund bericht=target/mutants/bericht.tsv pflicht_wahl="" bericht_wahl="" liste="" aus=""
  local voll_wahl="" diff_datei=target/mutants-diff.patch DARGS=() art_diff="" naht_extra=() weiter=""
  tmp="$(mktemp -d "${TMPDIR:-/tmp}/gates-mutants.XXXXXX")" || return 64
  mutants_plan "$tmp" || code=$?
  if [ "${code:-0}" -ne 0 ]; then
    case "$tmp" in */gates-mutants.??????) rm -rf -- "$tmp" ;; esac
    return "$code"
  fi
  while IFS=$'\t' read -r art crate grund; do
    case "$art" in
      FEHLT)
        echo "gates.sh: mutants: security crate $crate is not in the workspace (cargo metadata); red" >&2
        rc=1 ;;
      LISTE) liste="$liste $crate" ;;
      WAHL)
        # With a base, a crate runs limited to the diff; one chosen by a shared file or because no crate names the changed
        # file runs in full (a diff of such a file holds no mutant).
        art_diff=""
        if [ -n "$MUTANTS_GABEL" ]; then
          case "$grund" in
            "shared file changed"* | "exclusion list changed"*) voll_wahl="$voll_wahl $crate"; art_diff=" [full]" ;;
            *) art_diff=" [in diff]" ;;
          esac
        fi
        echo "gates.sh: mutants: run $crate ($grund)$art_diff"
        if mutants_is_pflicht "$crate"; then pflicht_wahl="$pflicht_wahl $crate"; else bericht_wahl="$bericht_wahl $crate"; fi ;;
    esac
  done < "$tmp/plan"
  case "$tmp" in */gates-mutants.??????) rm -rf -- "$tmp" ;; esac
  echo "gates.sh: mutants: security crates:$(printf ' %s' $MUTANTS_PFLICHT)"
  echo "gates.sh: mutants: report crates (cargo metadata):${liste:- none}"
  for crate in $MUTANTS_PFLICHT $liste; do
    case " $pflicht_wahl $bericht_wahl " in *" $crate "*) ;; *) aus="$aus $crate" ;; esac
  done
  echo "gates.sh: mutants: not run:${aus:- none}"
  if [ "$mode" = plan ]; then
    for crate in $pflicht_wahl; do
      echo "plan: security $crate"
      if mutants_is_naht "$crate"; then echo "plan: security $crate --features pruefbau"; fi
    done
    for crate in $bericht_wahl; do echo "plan: report $crate"; done
    if [ -n "$MUTANTS_GABEL" ]; then
      echo "plan: merge base $MUTANTS_GABEL; every run limited to the diff (--in-diff) except:${voll_wahl:- none}"
    else
      echo "plan: every run in full (no base, or no merge base)"
    fi
    return "$rc"
  fi
  mkdir -p target/mutants || return
  if [ -n "$MUTANTS_GABEL" ]; then
    mutants_diff "$diff_datei" || { echo "gates.sh: mutants: the diff against the merge base cannot be written" >&2; return 1; }
    echo "gates.sh: mutants: diff $diff_datei: $(grep -c '^+++ ' "$diff_datei") files, $(grep -c . "$diff_datei") lines"
  fi
  : > "$bericht" || return
  # One run after the other; never two at the same time.
  for crate in $pflicht_wahl; do
    code=0
    mutants_diff_args "$crate"
    mutants_one "$crate" "target/mutants/$crate" "$mode" ${erster[@]+"${erster[@]}"} ${gleichwertig[@]+"${gleichwertig[@]}"} ${testhilfe[@]+"${testhilfe[@]}"} ${DARGS[@]+"${DARGS[@]}"} || code=$?
    if [ "$mode" = list ]; then
      [ "$code" -eq 0 ] || rc="$code"
    else
      mutants_pflicht_result "$crate" "target/mutants/$crate" "$code" "" "${DARGS[0]:+diff}" || mutants_keep "$?"
    fi
    mutants_is_naht "$crate" || continue
    # The run above has no features, so the seam tests of the crate are not built. A second run with the feature
    # `pruefbau` lets its seam tests catch mutants too (decision of 24.11.2026).
    code=0
    # lauf: tests/einschluss.rs and tests/paket.rs start the program `einschluss-weiter` of the crate einschluss, which
    # `cargo test -p lauf` does not build (measured on peer: 15 failed tests, the baseline of the run would be red). The
    # `--test-package` arguments alone do not help: cargo-mutants 27.1.0 builds its baseline with `cargo test --no-run
    # --package=lauf@0.1.0 --features=pruefbau`, and 7 tests in tests/einschluss.rs end with "build einschluss-weiter first"
    # (measured on peer). So the gate builds the program first, in its own target folder, and names its absolute path in
    # LAUF_TEST_WEITER, which both test files read when the program is not next to the test program (the copy of
    # cargo-mutants). A failed build or a missing program is red, and the run does not start.
    naht_extra=()
    weiter=""
    case "$crate" in lauf) naht_extra=(--test-package lauf --test-package einschluss) ;; esac
    if [ "$crate" = lauf ] && [ "$mode" != list ]; then
      if ! cargo build --locked -p einschluss --bin einschluss-weiter; then
        echo "gates.sh: mutants: security crate lauf with the feature pruefbau red: cargo build of einschluss-weiter failed" >&2
        mutants_keep 1
        continue
      fi
      weiter="${CARGO_TARGET_DIR:-target}/debug/einschluss-weiter"
      case "$weiter" in /*) ;; *) weiter="$HERE/$weiter" ;; esac
      if [ ! -x "$weiter" ]; then
        echo "gates.sh: mutants: security crate lauf with the feature pruefbau red: $weiter is missing after the build" >&2
        mutants_keep 1
        continue
      fi
      echo "gates.sh: mutants: lauf with the feature pruefbau: LAUF_TEST_WEITER=$weiter"
    fi
    # The variable stands only in the subshell of this one run.
    ( if [ -n "$weiter" ]; then export LAUF_TEST_WEITER="$weiter"; fi
      mutants_one "$crate" "target/mutants-naht/$crate" "$mode" --features pruefbau ${gleichwertig[@]+"${gleichwertig[@]}"} ${testhilfe[@]+"${testhilfe[@]}"} ${naht_extra[@]+"${naht_extra[@]}"} ${DARGS[@]+"${DARGS[@]}"} ) || code=$?
    if [ "$mode" = list ]; then
      [ "$code" -eq 0 ] || rc="$code"
    else
      mutants_pflicht_result "$crate" "target/mutants-naht/$crate" "$code" " with the feature pruefbau" "${DARGS[0]:+diff}" || mutants_keep "$?"
    fi
  done
  for crate in $bericht_wahl; do
    code=0
    mutants_diff_args "$crate"
    mutants_one "$crate" "target/mutants/$crate" "$mode" ${erster[@]+"${erster[@]}"} ${gleichwertig[@]+"${gleichwertig[@]}"} ${testhilfe[@]+"${testhilfe[@]}"} ${DARGS[@]+"${DARGS[@]}"} || code=$?
    if [ "$mode" = list ]; then
      [ "$code" -eq 0 ] || rc="$code"
    else
      mutants_bericht_result "$crate" "target/mutants/$crate" "$code" "$bericht" || mutants_keep "$?"
    fi
  done
  if [ "$mode" = run ]; then
    echo "gates.sh: mutants: report of the report crates: $bericht ($(grep -c . "$bericht") lines)"
    if [ "$rc" -eq 0 ]; then
      echo "gates.sh: mutants green: security crates run:${pflicht_wahl:- none}"
    fi
  fi
  return "$rc"
}

# ---- vertraege gate ---------------------------------------------------------
# The checkers and the case builders of every contract below spec/ (rust/spec/<name>/), one after the other, in the
# order of the folder names (LC_ALL=C). Everything runs in a mktemp copy of rust/spec and spec; the work tree is never
# written. What runs, found by file name in each folder:
#   pruefen.py                 with jsonschema: `uv run --offline --no-project --with jsonschema==<pin> python -I -B`;
#                              exit not 0 is red, and so is an output that says the schema was NOT checked
#   bau-faelle.py, faelle_bauen.py, bau-faelle.mjs
#                              rebuild in the copy; any difference to the folder is red (`diff -r`). A bau-faelle.mjs
#                              that knows `--pruefen` is run with it instead (it compares byte for byte, exit 0 equal)
#   katalog-pruefen.py, gegenprobe.sh   read-only checks of the mutation catalogue against the sources
# No uv, no node, a jsonschema that is not in the uv cache (offline) or another version than the pin: red with exit 127,
# never skipped. VERTRAEGE_AUSGENOMMEN names the folders that are not run, each with its reason; the gate prints them.
# The .mjs builders import ../../../spec/protokoll: spec/ must lie next to rust/ (a copy of rust/ alone makes them red).
VERTRAEGE_JSONSCHEMA="4.26.0"
VERTRAEGE_AUSGENOMMEN="such-socket"

vertraege_ausnahme() {
  case "$1" in
    such-socket) echo "needs the built program (RANGKERN_BIN) and the library braincli; the runner tests come as a second step" ;;
    *) return 1 ;;
  esac
}

vertraege_wurzel() {
  (cd "$HERE/.." && pwd)
}

# vertraege_schritt <kind> <contract> <folder> <command ...>: runs the command in the folder, prints one line,
# shows the last lines of the output when it is red, returns the exit code (1 when a zero exit says "schema not checked").
vertraege_schritt() {
  local art="$1" v="$2" dir="$3" log="$VT_TMP/schritt.log" rc=0 t0=$SECONDS
  shift 3
  ( cd "$dir" && "$@" ) > "$log" 2>&1 || rc=$?
  if [ "$rc" -eq 0 ] && grep -q 'NICHT gepr' "$log"; then rc=1; echo "vertraege: $v: the output says the schema was not checked" >&2; fi
  if [ "$rc" -eq 0 ]; then
    echo "vertraege: $art $v ok ($((SECONDS - t0)) s)"
  else
    echo "vertraege: $art $v RED exit $rc ($((SECONDS - t0)) s)"
    tail -n 15 "$log" | sed 's/^/    | /'
    VT_ROT="$VT_ROT $art:$v"
  fi
  return "$rc"
}

vertraege_lauf() {
  local root spec v d probe_out probe_ver t0=$SECONDS nmal=0 nbau=0 naus=0 grund
  root="$(vertraege_wurzel)" || return 64
  spec="$root/rust/spec"
  if [ ! -d "$spec" ]; then
    echo "gates.sh: vertraege: $spec not found" >&2
    return 1
  fi
  if ! command -v uv > /dev/null 2>&1; then
    echo "gates.sh: vertraege: uv not found in PATH; the checkers run with 'uv run --offline --no-project --with jsonschema==$VERTRAEGE_JSONSCHEMA'. Red, not skipped." >&2
    return 127
  fi
  if ! command -v python3 > /dev/null 2>&1; then
    echo "gates.sh: vertraege: python3 not found in PATH. Red, not skipped." >&2
    return 127
  fi
  VT_TMP="$(mktemp -d "${TMPDIR:-/tmp}/gates-vertraege.XXXXXX")" || return 64
  case "$VT_TMP" in */gates-vertraege.??????) ;; *) echo "gates.sh: vertraege: unexpected scratch folder $VT_TMP" >&2; return 64 ;; esac
  VT_ROT=""
  # The package must be in the uv cache in exactly the pinned version; one probe, before any checker.
  probe_out="$( { uv run --offline --no-project --with "jsonschema==$VERTRAEGE_JSONSCHEMA" python -I -B -c 'import importlib.metadata as m; print(m.version("jsonschema"))' 2> "$VT_TMP/probe.err"; } )" || {
    echo "gates.sh: vertraege: jsonschema==$VERTRAEGE_JSONSCHEMA is not available offline (uv cache). Fill the cache once with network: uv run --no-project --with jsonschema==$VERTRAEGE_JSONSCHEMA python -c pass. Red, not skipped." >&2
    sed 's/^/    | /' "$VT_TMP/probe.err" >&2
    rm -rf -- "$VT_TMP"
    return 127
  }
  probe_ver="$(printf '%s\n' "$probe_out" | tail -n 1)"
  if [ "$probe_ver" != "$VERTRAEGE_JSONSCHEMA" ]; then
    echo "gates.sh: vertraege: uv gave jsonschema '$probe_ver', the pin is $VERTRAEGE_JSONSCHEMA. Red, not skipped." >&2
    rm -rf -- "$VT_TMP"
    return 127
  fi
  echo "vertraege: jsonschema $probe_ver from the uv cache, $(uv --version 2>&1 | head -n 1)"
  # The copy: the checkers and the builders run here, never in the work tree.
  mkdir -p "$VT_TMP/kopie/rust" || return 64
  cp -R "$spec" "$VT_TMP/kopie/rust/spec" || return 64
  [ -d "$root/spec" ] && { cp -R "$root/spec" "$VT_TMP/kopie/spec" || return 64; }
  local kopie="$VT_TMP/kopie"
  # Checkers first (all of them), then the rebuilds.
  for d in "$spec"/*/; do
    v="$(basename "$d")"
    if grund="$(vertraege_ausnahme "$v")"; then
      echo "vertraege: $v excluded, not run: $grund"; naus=$((naus + 1)); continue
    fi
    [ -f "$d/pruefen.py" ] || continue
    nmal=$((nmal + 1))
    vertraege_schritt pruefen "$v" "$kopie" uv run --offline --no-project --with "jsonschema==$VERTRAEGE_JSONSCHEMA" python -I -B "rust/spec/$v/pruefen.py" || true
  done
  for d in "$spec"/*/; do
    v="$(basename "$d")"
    vertraege_ausnahme "$v" > /dev/null && continue
    if [ -f "$d/bau-faelle.mjs" ] && [ ! -d "$root/spec/protokoll" ]; then
      echo "vertraege: $v RED: $root/spec/protokoll is missing, the builder imports it (archive rust and spec together)"
      VT_ROT="$VT_ROT bau:$v"; nbau=$((nbau + 1)); continue
    fi
    if [ -f "$d/bau-faelle.mjs" ] && ! command -v node > /dev/null 2>&1; then
      echo "gates.sh: vertraege: node not found in PATH; $v/bau-faelle.mjs needs it. Red, not skipped." >&2
      VT_ROT="$VT_ROT bau:$v"; nbau=$((nbau + 1)); continue
    fi
    if [ -f "$d/bau-faelle.mjs" ] && grep -q -e '--pruefen' "$d/bau-faelle.mjs"; then
      nbau=$((nbau + 1))
      vertraege_schritt bau-pruefen "$v" "$kopie" node "rust/spec/$v/bau-faelle.mjs" --pruefen || true
    elif [ -f "$d/bau-faelle.mjs" ] || [ -f "$d/bau-faelle.py" ] || [ -f "$d/faelle_bauen.py" ]; then
      nbau=$((nbau + 1))
      if [ -f "$d/bau-faelle.py" ]; then
        vertraege_schritt bau "$v" "$kopie" python3 -I -B "rust/spec/$v/bau-faelle.py" || true
      elif [ -f "$d/bau-faelle.mjs" ]; then
        vertraege_schritt bau "$v" "$kopie" node "rust/spec/$v/bau-faelle.mjs" || true
      else
        vertraege_schritt bau "$v" "$kopie/rust/spec/$v" python3 -I -B faelle_bauen.py || true
      fi
      vertraege_schritt neubau-diff "$v" "$VT_TMP" diff -r -q -x __pycache__ "$spec/$v" "$kopie/rust/spec/$v" || true
      # The next builder starts from the original again.
      rm -rf -- "${kopie:?}/rust/spec/$v"
      cp -R "$d" "$kopie/rust/spec/$v" || return 64
    fi
    if [ -f "$d/katalog-pruefen.py" ] && [ -f "$d/katalog.json" ]; then
      nbau=$((nbau + 1))
      vertraege_schritt katalog "$v" "$VT_TMP" python3 -I -B "$d/katalog-pruefen.py" "$d/katalog.json" --wurzel "$root" || true
    fi
    if [ -f "$d/gegenprobe.sh" ]; then
      nbau=$((nbau + 1))
      vertraege_schritt gegenprobe "$v" "$VT_TMP" bash "$d/gegenprobe.sh" || true
    fi
  done
  # Second step: the unit tests of the runner, in the work tree (they read paths outside spec/), checked unchanged.
  vertraege_laeufer "$spec" || true
  # A builder that wrote outside its own folder shows here.
  vertraege_schritt gesamt-diff rust/spec "$VT_TMP" diff -r -q -x __pycache__ "$spec" "$kopie/rust/spec" || true
  if [ -d "$root/spec" ]; then
    vertraege_schritt gesamt-diff spec "$VT_TMP" diff -r -q -x __pycache__ "$root/spec" "$kopie/spec" || true
  fi
  rm -rf -- "$VT_TMP"
  if [ -z "$VT_ROT" ]; then
    echo "gates.sh: vertraege green: $nmal checkers, $nbau rebuilds or checks, $naus not run (named above), runner tests: $LT_ZAHLEN, $((SECONDS - t0)) s"
    return 0
  fi
  echo "gates.sh: vertraege RED:$VT_ROT ($((SECONDS - t0)) s)" >&2
  return 1
}

# ---- vertraege, second step: the unit tests of the runner ------------------------------------------------
# Each file rust/spec/vorfuehrung/laeufer/test_*.py runs on its own, `python3 -I -B -m unittest discover -v -s .
# -p <file>`, VERTRAEGE_LAEUFER_JOBS (4) files side by side (xargs -P). The tests read files outside spec/ (paket/,
# seite/, crates/), so they run in the work tree, not in the copy; a checksum list of the runner folder before and
# after must be equal (a test that writes there is red). Nothing is inherited: `env -i` with PATH, HOME, TMPDIR and
# the locale only, and each file gets its own HOME and TMPDIR.
#   PATH    a fresh bin/ with links to the python3 and the node found in the caller's PATH, then /usr/bin:/bin
#           (fixed). The mock-ups start `#!/usr/bin/env python3`; test_sitzung_klick drives Playwright with node.
#           openssl, sh and bash must be found in /usr/bin:/bin.
#   HOME    a fresh folder per file, mode 0700; no test sees the caller's home. The exception are two links, made only
#           when the caller has them: the Playwright package named by browser.playwright_pfad of konfig.attrappe.json
#           (a path below ~) and the browser cache of Playwright (PLAYWRIGHT_BROWSERS_PATH of the caller, else macOS
#           ~/Library/Caches/ms-playwright, Linux ~/.cache/ms-playwright), at the place Playwright looks in the fresh
#           home. test_sitzung_klick passes only PATH, HOME and TMPDIR on, so a variable would not reach the browser.
#           Without them that test is red.
#   TMPDIR  a fresh folder per file below /tmp with a short path (/tmp/wbl.XXXXXX/<n>/t): the mock-ups bind Unix
#           sockets below it, and sun_path holds about 104 bytes (macOS); drehbuch.py refuses a work folder deeper
#           than 60 characters.
# A trap removes the scratch folder. No python3, node, openssl, sh or bash is red with a reason, never skipped. One
# line per file and a sum; a skipped test (unittest skip) is counted and printed with its reason. A file without the
# line `Ran N tests`, with N = 0, with a summary that is not OK or with an exit code not 0 is red, and so is the step.
# Tests that only mean something on Linux skip on macOS and are counted there.
# mutationsprobe.py is not part of this step: it runs the whole script once per built-in fault (about 50 runs,
# minutes in all); it is run by hand, as the runner README says.
VERTRAEGE_LAEUFER="vorfuehrung/laeufer"
VERTRAEGE_LAEUFER_JOBS=4
# The longest files start first (peer, 26.11.2026: 124, 60, 57 and 52 s of about 550 s in all); a name that is no
# longer there is ignored, every other file follows in name order.
VERTRAEGE_LAEUFER_ZUERST="test_positivkontrollen.py test_sitzung_grund.py test_diagnose_ende.py test_sitzung_klick.py"
LT_ZAHLEN="not run"

# One file, called by xargs: <scratch> <runner folder> <PATH> <n> <file>. Writes <scratch>/<n>.log and
# <scratch>/<n>.rc ("<exit code> <seconds>"); always exits 0, so xargs runs every file.
LT_EINZEL='
tmp="$1"; dir="$2"; fest="$3"; n="$4"; datei="$5"; w="$tmp/$n"
if ! mkdir -m 700 "$w" "$w/t" || ! cp -R -P "$tmp/h" "$w/h" || ! chmod 700 "$w/h"; then
  echo "64 0" > "$tmp/$n.rc"; exit 0
fi
t0=$(date +%s); rc=0
( cd "$dir" && env -i PATH="$fest" HOME="$w/h" TMPDIR="$w/t" LANG=C.UTF-8 LC_ALL=C.UTF-8 \
    python3 -I -B -m unittest discover -v -s . -p "$datei" ) > "$tmp/$n.log" 2>&1 || rc=$?
echo "$rc $(( $(date +%s) - t0 ))" > "$tmp/$n.rc"
exit 0
'

vertraege_laeufer_summe() {
  find "$1" -name __pycache__ -prune -o -type f -exec cksum {} + | LC_ALL=C sort
}

vertraege_laeufer() {
  local spec="$1" dir fest t0=$SECONDS rc=0 log werkzeug pfad ran zeile skips pw rel cache
  local f b n frc fs ok nfiles=0 nrot=0 sran=0 sskip=0 liste="" vorn=""
  dir="$spec/$VERTRAEGE_LAEUFER"
  LT_ZAHLEN="RED"
  if [ ! -d "$dir" ]; then
    echo "vertraege: laeufer RED: $dir not found"; VT_ROT="$VT_ROT laeufer:fehlt"; return 1
  fi
  LT_TMP="$(mktemp -d /tmp/wbl.XXXXXX)" || { VT_ROT="$VT_ROT laeufer:mktemp"; return 64; }
  case "$LT_TMP" in /tmp/wbl.??????) ;; *) echo "vertraege: laeufer RED: unexpected scratch folder $LT_TMP"; VT_ROT="$VT_ROT laeufer:mktemp"; return 64 ;; esac
  trap 'rm -rf -- "$LT_TMP"' EXIT
  trap 'rm -rf -- "$LT_TMP"; exit 130' INT TERM HUP
  mkdir -m 700 "$LT_TMP/h" "$LT_TMP/bin" || { VT_ROT="$VT_ROT laeufer:mktemp"; rm -rf -- "$LT_TMP"; return 64; }
  fest="$LT_TMP/bin:/usr/bin:/bin"
  for werkzeug in python3 node; do
    pfad="$(command -v "$werkzeug" 2> /dev/null)" || pfad=""
    case "$pfad" in
      /*) ln -s "$pfad" "$LT_TMP/bin/$werkzeug" ;;
      *) echo "vertraege: laeufer RED: $werkzeug not found in PATH (as a file path). Red, not skipped."
         VT_ROT="$VT_ROT laeufer:$werkzeug"; rm -rf -- "$LT_TMP"; return 127 ;;
    esac
  done
  for werkzeug in openssl sh bash; do
    if ! PATH="$fest" command -v "$werkzeug" > /dev/null 2>&1; then
      echo "vertraege: laeufer RED: $werkzeug not found in the fixed PATH $fest. Red, not skipped."
      VT_ROT="$VT_ROT laeufer:$werkzeug"; rm -rf -- "$LT_TMP"; return 127
    fi
  done
  # The template of each HOME: the Playwright package below ~ as a link, the browsers from the caller's cache.
  pw="$(python3 -I -B -c 'import json, sys; print(json.load(open(sys.argv[1]))["browser"]["playwright_pfad"])' "$dir/konfig.attrappe.json" 2> /dev/null)" || pw=""
  case "$pw" in
    "~/"*)
      rel="${pw#\~/}"
      if [ -n "${HOME:-}" ] && [ -d "$HOME/$rel" ]; then
        mkdir -p "$LT_TMP/h/$(dirname "$rel")" && ln -s "$HOME/$rel" "$LT_TMP/h/$rel"
      fi ;;
  esac
  case "$(uname -s)" in
    Darwin) rel="Library/Caches/ms-playwright" ;;
    *) rel=".cache/ms-playwright" ;;
  esac
  cache="${PLAYWRIGHT_BROWSERS_PATH:-}"
  if [ -z "$cache" ] && [ -n "${HOME:-}" ]; then
    case "$(uname -s)" in
      Darwin) cache="$HOME/$rel" ;;
      *) cache="${XDG_CACHE_HOME:-$HOME/.cache}/ms-playwright" ;;
    esac
  fi
  if [ -n "$cache" ] && [ -d "$cache" ]; then
    mkdir -p "$LT_TMP/h/$(dirname "$rel")" && ln -s "$cache" "$LT_TMP/h/$rel"
  fi
  # The files, numbered in name order (LC_ALL=C); the queue starts with VERTRAEGE_LAEUFER_ZUERST.
  n=0
  for f in "$dir"/test_*.py; do
    [ -f "$f" ] || continue
    n=$((n + 1)); b="$(basename "$f")"
    case " $VERTRAEGE_LAEUFER_ZUERST " in
      *" $b "*) vorn="$vorn $n $b" ;;
      *) liste="$liste $n $b" ;;
    esac
  done
  liste="$vorn$liste"
  nfiles=$n
  if [ "$nfiles" -eq 0 ]; then
    echo "vertraege: laeufer RED: no file test_*.py in $dir"; VT_ROT="$VT_ROT laeufer:leer"; rm -rf -- "$LT_TMP"; trap - EXIT INT TERM HUP; return 1
  fi
  vertraege_laeufer_summe "$dir" > "$LT_TMP/vorher"
  # shellcheck disable=SC2086 # $liste is "<n> <file> ..." without blanks inside a field
  printf '%s\n' $liste | xargs -n 2 -P "$VERTRAEGE_LAEUFER_JOBS" bash -c "$LT_EINZEL" _ "$LT_TMP" "$dir" "$fest" || rc=$?
  if [ "$rc" -ne 0 ]; then
    echo "vertraege: laeufer: xargs ended with exit $rc"
  fi
  vertraege_laeufer_summe "$dir" > "$LT_TMP/nachher"
  n=0
  for f in "$dir"/test_*.py; do
    [ -f "$f" ] || continue
    n=$((n + 1)); f="$(basename "$f")"; log="$LT_TMP/$n.log"
    frc=1; fs="?"
    [ -f "$LT_TMP/$n.rc" ] && read -r frc fs < "$LT_TMP/$n.rc"
    ran=""; zeile=""; skips=0
    if [ -f "$log" ]; then
      ran="$(sed -n 's/^Ran \([0-9][0-9]*\) tests\{0,1\} in .*/\1/p' "$log" | tail -n 1)"
      zeile="$(grep -E '^(OK|FAILED)( \(.*\))?$' "$log" | tail -n 1)"
      skips="$(grep -c -E '\.\.\. skipped ' "$log")"
      grep -E '\.\.\. skipped ' "$log" | sed "s/^/vertraege: laeufer skipped: $f: /"
    fi
    ok=0
    if [ -n "$ran" ] && [ "$ran" -gt 0 ] && [ "$frc" = 0 ]; then
      case "$zeile" in OK|OK\ \(*) ok=1 ;; esac
    fi
    if [ "$ok" -eq 1 ]; then
      echo "vertraege: laeufer $f ok: $ran run, $skips skipped ($fs s)"
    else
      nrot=$((nrot + 1))
      echo "vertraege: laeufer $f RED exit $frc: ${ran:-no line Ran} run, $skips skipped, ${zeile:-no summary line} ($fs s)"
      if [ -f "$log" ]; then
        # The report blocks of unittest (each failing test with its traceback), then the last lines.
        awk '/^==========/ { an = 1 } /^Ran [0-9]+ tests? in / { an = 0 } an' "$log" | head -n 120 | sed 's/^/    | /'
        tail -n 5 "$log" | sed 's/^/    | /'
      else
        echo "    | no output of this file"
      fi
    fi
    sran=$((sran + ${ran:-0})); sskip=$((sskip + skips))
  done
  if ! cmp -s "$LT_TMP/vorher" "$LT_TMP/nachher"; then
    echo "vertraege: laeufer: the runner folder changed during the tests:"
    diff "$LT_TMP/vorher" "$LT_TMP/nachher" | head -n 20 | sed 's/^/    | /'
    nrot=$((nrot + 1))
  fi
  [ "$rc" -ne 0 ] && nrot=$((nrot + 1))
  LT_ZAHLEN="$sran run, $sskip skipped in $nfiles files"
  rm -rf -- "$LT_TMP"
  trap - EXIT INT TERM HUP
  if [ "$nrot" -eq 0 ]; then
    echo "vertraege: laeufer ok: $LT_ZAHLEN, $VERTRAEGE_LAEUFER_JOBS side by side ($((SECONDS - t0)) s)"
    return 0
  fi
  echo "vertraege: laeufer RED: $nrot red ($LT_ZAHLEN, $((SECONDS - t0)) s)"
  VT_ROT="$VT_ROT laeufer"
  return 1
}

gate_vertraege() {
  ( export LC_ALL=C; vertraege_lauf )
}

run_gate() {
  case "$1" in
    fmt) gate_fmt ;;
    clippy) gate_clippy ;;
    test) gate_test ;;
    licenses) gate_licenses ;;
    notices) gate_notices ;;
    mutants) gate_mutants run ;;
    vertraege) gate_vertraege ;;
    mutants-plan) gate_mutants plan ;;
    mutants-list) gate_mutants list ;;
    tools) gate_tools ;;
    *) echo "gates.sh: unknown gate '$1' (gates: $GATES all)" >&2; return 64 ;;
  esac
}

if [ "$#" -ne 1 ]; then
  echo "usage: $0 {$(echo "$GATES" | tr ' ' '|')|all}" >&2
  exit 64
fi

if [ "$1" != "all" ]; then
  run_gate "$1"
  exit $?
fi

summary=""
failed=0
for gate in $GATES; do
  echo "==> gate $gate"
  rc=0
  run_gate "$gate" || rc=$?
  summary="${summary}gate $gate exit $rc"$'\n'
  [ "$rc" -eq 0 ] || failed=1
done
echo "==> summary"
printf '%s' "$summary"
exit "$failed"
