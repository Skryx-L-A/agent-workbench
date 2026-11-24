#!/usr/bin/env bash
# gates.sh -- the build gates of the Rust workspace, one script, one exit code per gate.
#
#   ./gates.sh fmt        cargo fmt --check
#   ./gates.sh clippy     cargo clippy, every warning is an error
#   ./gates.sh test       cargo test --no-fail-fast
#   ./gates.sh licenses   cargo-deny (licences, bans, sources) and a CycloneDX SBOM per crate
#   ./gates.sh notices    every row of "Code taken from other projects" in README.md has its notice
#                         (file or file head, names the SPDX id, ships in `cargo package --list`);
#                         a file under crates/*/src with a foreign licence head that no row lists is red;
#                         a crate with a bin target and a row runs each bin with `--licenses`: exit 0, and
#                         each notice file of its rows appears in the output byte for byte
#   ./gates.sh mutants    cargo-mutants; red if a mutant survives or no mutant was caught
#   ./gates.sh all        every gate in this order; prints one line per gate
#   ./gates.sh tools      not a gate: checks the tool folder (~/.local/share/wb-rust-werkzeuge), the toolchain
#                         and the three tools in their pinned versions; every gate makes the same checks first
#
# Exit codes: 0 green; the exit code of the failing tool for a single gate (at least 1);
# 1 for `all` when any gate is red; 64 usage error; 127 the tool folder, the toolchain or a required tool
# is missing or has another version than pinned.
# The script works from its own directory and writes only below it (target/, mutants.out/, sbom/).
# The notices gate writes only its cargo build (target/) there; its scratch files live in a mktemp directory.
# Runs on bash 3.2 (macOS) and current bash; no associative arrays.

set -u
set -o pipefail

HERE="$(cd "$(dirname "$0")" && pwd)" || exit 64
cd "$HERE" || exit 64

GATES="fmt clippy test licenses notices mutants"

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
  cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
}

gate_test() {
  need cargo || return
  cargo test --workspace --all-targets --all-features --locked --no-fail-fast
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
# marker and that no row covers is red. A file whose SPDX-License-Identifier lines all name an own licence
# (OWN_SPDX) is own code and is skipped.
# A missing README.md, a missing section, a malformed header or no data row is red, never green.
# Output direction (notices_bins): a crate that has a bin target (cargo metadata, --locked) and a row is
# built with `cargo build --bins`; each of its bins must exit 0 on `--licenses`, and its output must contain
# every notice file of the crate's rows byte for byte. A library crate gets no run; its check is the listing.

NOTICE_HEAD_LINES=40
OWN_SPDX="BUSL-1.1"
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
  local tmp="$1" rc=0 f hit id own
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
    [ "$own" -eq 1 ] && continue
    echo "notices: $f carries a foreign licence head ($(printf '%s' "$hit" | cut -c1-60)) and no row of \"$NOTICE_SECTION\" in README.md lists it" >&2
    rc=1
  done < "$tmp/srcfiles"
  return "$rc"
}

# Metadata of the workspace: "TARGET<TAB>target directory" and "BIN<TAB>crate directory<TAB>bin name" per bin.
# Paths are relative to the working directory, so they match the crate paths of the table.
NOTICES_BINS_PY='import json, os, sys
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

gate_mutants() {
  need cargo || return
  need_subcommand mutants || return
  local rc=0
  cargo mutants --workspace --no-shuffle --output . || rc=$?
  if [ "$rc" -ne 0 ]; then
    return "$rc"
  fi
  # A green run must have caught at least one mutant; otherwise it proved nothing.
  local caught
  caught="$(grep -c . mutants.out/caught.txt 2>/dev/null)" || caught=0
  if [ "$caught" -lt 1 ]; then
    echo "gates.sh: no mutant caught; the mutation run proved nothing" >&2
    return 1
  fi
  echo "gates.sh: $caught mutants caught, none missed"
}

run_gate() {
  case "$1" in
    fmt) gate_fmt ;;
    clippy) gate_clippy ;;
    test) gate_test ;;
    licenses) gate_licenses ;;
    notices) gate_notices ;;
    mutants) gate_mutants ;;
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
