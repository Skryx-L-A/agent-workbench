#!/bin/bash
# test-instructions.sh -- tests for wb-instructions (SPEC-V3-MODELS.md section E).
#
# ISOLATION (rules of 2026-07-25, both after real incidents -- see test-registry.sh header):
#   * own HOME: mktemp -d. The REAL ~/.claude/CLAUDE.md, ~/.claude/roles/*.md and
#     ~/.claude/skills/ are NEVER read or written by this script -- everything under test is a
#     fixture copy. A test that needs the live files is built wrong.
#   * PATH is pinned to the test bin dir + /usr/bin:/bin, so `wb-state` resolves to whatever
#     (or nothing) this script puts there, never alice's real deployed copy.
#   * no tmux involved at all -- wb-instructions only touches files, so file-level isolation via
#     HOME/PATH is the whole story here.
#
# Run:  shell/tests/test-instructions.sh          (from the repo, or anywhere)
set -uo pipefail

REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"     # …/claude-workbench/shell
TOOL="$REPO/wb-instructions"
PASS=0; FAIL=0

pass() { PASS=$((PASS+1)); echo "PASS: $1"; }
fail() { FAIL=$((FAIL+1)); echo "FAIL: $1"; }

TESTHOME="$(mktemp -d "${TMPDIR:-/tmp}/wb-instructions-test.XXXXXX")"
cleanup() { rm -rf "$TESTHOME"; }
trap cleanup EXIT

export HOME="$TESTHOME"
BIN="$TESTHOME/bin"; mkdir -p "$BIN"
export PATH="$BIN:/usr/bin:/bin"

# --- fixtures: a small stand-in for CLAUDE.md / roles / skills -------------------------------
# CLAUDE.md mirrors the REAL file's shape around the routing table: a hand-curated table
# followed by a blank line, followed by the "- **Effort ist ein Knopf**" bullet the real
# anchor logic looks for. This is deliberate: the 2026-07-28 incident was the marker code
# MATCHING and REPLACING this exact table when a fill command became available -- the fixture
# has to contain a real table at a real anchor point to catch a regression of that bug.
mkdir -p "$TESTHOME/.claude/roles" "$TESTHOME/.claude/skills/fakeskill/scripts"

write_claude_md() {
cat > "$TESTHOME/.claude/CLAUDE.md" <<'EOF'
# Global instructions (test fixture)

Some rule that mentions `/compact` as the compaction keystroke, and points at
`~/.claude/skills/fakeskill/` for a skill. This session runs `claude-opus-5` by default.

## Right-size worker models

  | Aufgabe | Worker |
  |---|---|
  | mechanisch | `haiku45:low` |
  | lang | `opus5:xhigh` |

- **Effort ist ein Knopf** (test-fixture continuation line that must survive).

## After the table

This line must survive byte-for-byte.
EOF
}
write_orchestrator_md() {
cat > "$TESTHOME/.claude/roles/orchestrator.md" <<'EOF'
# ROLE: ORCHESTRATOR (test fixture)

Model policy: `claude-opus-5` @ xhigh.

## Delegation
- Modellwahl (Tabelle und HARD CAPS: CLAUDE.md) — Begründungen und Zusätze:
  - some sub-bullet that must survive.
EOF
}
write_claude_md
write_orchestrator_md
cp "$TESTHOME/.claude/CLAUDE.md" "$TESTHOME/CLAUDE.md.pristine"
cp "$TESTHOME/.claude/roles/orchestrator.md" "$TESTHOME/orchestrator.md.pristine"

cat > "$TESTHOME/.claude/roles/agent.md" <<'EOF'
# ROLE: WORKER AGENT (test fixture)

Follow `~/.claude/skills/fakeskill/` before starting a matching task. Use `/compact` when asked.
EOF

cat > "$TESTHOME/.claude/skills/fakeskill/SKILL.md" <<'EOF'
---
name: fakeskill
description: >-
  A fixture skill used only by test-instructions.sh, spanning two folded
  lines to exercise the YAML block-scalar parser.
---

# fakeskill

Body content of the fixture skill.
EOF
echo "console.log('fixture asset');" > "$TESTHOME/.claude/skills/fakeskill/scripts/helper.js"

# ================================================================================================
echo "--- 1) dry-run: no files created/changed, exits 0 ---"
OUT="$("$TOOL" sync --dry-run 2>&1)"; RC=$?
if [ "$RC" -eq 0 ]; then pass "dry-run exits 0"; else fail "dry-run exit $RC"; fi
if [ ! -e "$TESTHOME/.codex/AGENTS.md" ]; then pass "dry-run created no target file"; else fail "dry-run wrote a target file"; fi
if echo "$OUT" | grep -qE "^codex\s+new\b"; then pass "dry-run reports codex as new"; else fail "dry-run missing codex/new row: $OUT"; fi
if diff -q "$TESTHOME/CLAUDE.md.pristine" "$TESTHOME/.claude/CLAUDE.md" >/dev/null; then
  pass "dry-run left the real CLAUDE.md fixture untouched"
else
  fail "dry-run modified the CLAUDE.md fixture"
fi

# ================================================================================================
echo "--- 2) real run: all 7 harness files + skill mirror + routing markers ---"
OUT="$("$TOOL" sync 2>&1)"; RC=$?
if [ "$RC" -eq 0 ]; then pass "real run exits 0"; else fail "real run exit $RC: $OUT"; fi

ALL_OK=1
for f in codex/AGENTS.md antigravity/AGENTS.md config/opencode/AGENTS.md config/crush/CRUSH.md \
         config/goose/.goosehints aider/CONVENTIONS.md pi/agent/RULES.md; do
  p="$TESTHOME/.$f"
  [ -f "$p" ] || { ALL_OK=0; fail "missing target ~/.${f}"; }
done
[ "$ALL_OK" -eq 1 ] && pass "all 7 harness target files exist"

CODEX="$TESTHOME/.codex/AGENTS.md"
if grep -q "wb-instructions:generated" "$CODEX"; then pass "codex target carries the marker block"; else fail "codex target missing marker"; fi
if grep -q '`/compact`' "$CODEX"; then pass "codex keeps /compact literal (same command)"; else fail "codex compact substitution wrong"; fi
if grep -q "This line must survive byte-for-byte." "$CODEX"; then pass "codex target preserves unrelated CLAUDE.md content"; else fail "codex target lost unrelated content"; fi
if grep -q "some sub-bullet that must survive." "$CODEX"; then pass "codex target preserves unrelated orchestrator.md content"; else fail "codex target lost orchestrator content"; fi
if grep -q "~/.agent-skills/fakeskill/" "$CODEX"; then pass "codex target rewrote skills path"; else fail "codex target kept ~/.claude/skills/ path"; fi
if ! grep -q "~/.claude/skills/fakeskill" "$CODEX"; then pass "codex target has no leftover ~/.claude/skills/ path"; else fail "codex target still has ~/.claude/skills/"; fi
if grep -q "siehe .wb-state models list" "$CODEX"; then pass "codex target annotates claude-opus-5 with a registry hint"; else fail "codex target missing registry hint"; fi

CRUSH="$TESTHOME/.config/crush/CRUSH.md"
if grep -q '`/summarize`' "$CRUSH"; then pass "crush gets its own compaction command (/summarize)"; else fail "crush compact substitution wrong"; fi

NOCOMPACT="$TESTHOME/.antigravity/AGENTS.md"   # agy hat keinen Kompaktierungsbefehl
if grep -qiE "kein Kompaktierungsbefehl|UNVERIFIZIERT" "$NOCOMPACT"; then pass "agy marks its missing compact command"; else fail "agy: missing-compact note absent"; fi

AIDER="$TESTHOME/.aider/CONVENTIONS.md"
if grep -q -- "--read ~/.aider/CONVENTIONS.md" "$AIDER"; then pass "aider gets its auto-load caveat"; else fail "aider missing auto-load caveat"; fi

if [ -f "$TESTHOME/.agent-skills/fakeskill/SKILL.md" ]; then pass "skill mirrored to ~/.agent-skills/"; else fail "skill not mirrored"; fi
if grep -q "wb-instructions:generated" "$TESTHOME/.agent-skills/fakeskill/SKILL.md"; then pass "mirrored SKILL.md carries a marker"; else fail "mirrored SKILL.md missing marker"; fi
if [ -f "$TESTHOME/.agent-skills/fakeskill/scripts/helper.js" ] && grep -q "fixture asset" "$TESTHOME/.agent-skills/fakeskill/scripts/helper.js"; then
  pass "accompanying skill asset copied verbatim"
else
  fail "accompanying skill asset not copied correctly"
fi
if grep -q "spanning two folded lines to exercise the YAML block-scalar parser." "$CODEX"; then
  pass "skills index in target has the folded description"
else
  fail "skills index missing/garbled folded description"
fi

echo "--- 2a) routing-table markers: ADDITIVE only, curated table untouched (the regression this fixes) ---"
if grep -q "wb:routing-table:start" "$TESTHOME/.claude/CLAUDE.md" && grep -q "wb:routing-table:end" "$TESTHOME/.claude/CLAUDE.md"; then
  pass "CLAUDE.md got the routing-table marker pair"
else
  fail "CLAUDE.md missing routing-table markers"
fi
if grep -q "wb:routing-table:start" "$TESTHOME/.claude/roles/orchestrator.md"; then
  pass "orchestrator.md got the routing-table marker pair"
else
  fail "orchestrator.md missing routing-table markers"
fi
# The curated table's own two rows must be there, UNCHANGED, exactly as many times as before
# (i.e. not consumed/replaced by the marker insertion).
if grep -c '| mechanisch | `haiku45:low` |' "$TESTHOME/.claude/CLAUDE.md" | grep -q '^1$' && \
   grep -c '| lang | `opus5:xhigh` |' "$TESTHOME/.claude/CLAUDE.md" | grep -q '^1$'; then
  pass "curated table rows survive exactly once (not wrapped/replaced)"
else
  fail "curated table rows missing or duplicated -- marker insertion touched the table"
fi
if grep -q '\- \*\*Effort ist ein Knopf\*\* (test-fixture continuation line that must survive)\.' "$TESTHOME/.claude/CLAUDE.md"; then
  pass "the anchor bullet line itself is unchanged"
else
  fail "the anchor bullet line was altered"
fi
if grep -q "wird von \"wb-instructions sync\" automatisch gefuellt" "$TESTHOME/.claude/CLAUDE.md"; then
  pass "CLAUDE.md got the required note above the generated block"
else
  fail "CLAUDE.md missing the note above the generated block"
fi
# No wb-state on PATH yet -> the block between the markers must be EMPTY (nothing invented).
CLAUDE_BLOCK="$(sed -n '/wb:routing-table:start/,/wb:routing-table:end/p' "$TESTHOME/.claude/CLAUDE.md")"
BLOCK_LINES="$(echo "$CLAUDE_BLOCK" | wc -l | tr -d ' ')"
if [ "$BLOCK_LINES" -eq 2 ]; then
  pass "CLAUDE.md routing block is empty (start+end only) when no fill command is available"
else
  fail "CLAUDE.md routing block is not empty without a fill command ($BLOCK_LINES lines): $CLAUDE_BLOCK"
fi

echo "--- 2b) CLAUDE.md diff against the pristine fixture: only ADDED lines, nothing removed ---"
DIFF_OUT="$(diff "$TESTHOME/CLAUDE.md.pristine" "$TESTHOME/.claude/CLAUDE.md")"
ADDED_LINES="$(echo "$DIFF_OUT" | grep -c '^> ')"
REMOVED_LINES="$(echo "$DIFF_OUT" | grep -c '^< ')"
if [ "$REMOVED_LINES" -eq 0 ] && [ "$ADDED_LINES" -eq 3 ]; then
  pass "CLAUDE.md diff is exactly +3 lines (note+start+end), 0 removed"
else
  fail "CLAUDE.md diff is not a clean +3/-0 (added=$ADDED_LINES removed=$REMOVED_LINES): $DIFF_OUT"
fi
if echo "$DIFF_OUT" | grep -q '^> .*wb:routing-table:start' && echo "$DIFF_OUT" | grep -q '^> .*wb:routing-table:end'; then
  pass "the added lines include the routing-table markers"
else
  fail "added lines are not the expected markers: $DIFF_OUT"
fi

# ================================================================================================
echo "--- 3) idempotency: second real run touches nothing ---"
HASH_BEFORE="$(shasum -a 256 "$CODEX" | awk '{print $1}')"
MTIME_BEFORE="$(stat -f %m "$CODEX" 2>/dev/null || stat -c %Y "$CODEX")"
sleep 1
OUT2="$("$TOOL" sync 2>&1)"; RC2=$?
HASH_AFTER="$(shasum -a 256 "$CODEX" | awk '{print $1}')"
MTIME_AFTER="$(stat -f %m "$CODEX" 2>/dev/null || stat -c %Y "$CODEX")"
if [ "$RC2" -eq 0 ]; then pass "second run exits 0"; else fail "second run exit $RC2"; fi
if [ "$HASH_BEFORE" = "$HASH_AFTER" ]; then pass "second run: codex target byte-identical"; else fail "second run changed codex target content"; fi
if [ "$MTIME_BEFORE" = "$MTIME_AFTER" ]; then pass "second run: codex target not rewritten (mtime untouched)"; else fail "second run rewrote codex target (mtime changed)"; fi
if echo "$OUT2" | grep -qE "^codex\s+unchanged\b"; then pass "second run reports codex unchanged"; else fail "second run did not report unchanged: $OUT2"; fi
if diff -q "$TESTHOME/.claude/CLAUDE.md" <(echo "ref") >/dev/null 2>&1; then :; fi
CLAUDE_HASH_A="$(shasum -a 256 "$TESTHOME/.claude/CLAUDE.md" | awk '{print $1}')"
if echo "$OUT2" | grep -qE "routing-table:CLAUDE\.md\s+unchanged"; then pass "second run: CLAUDE.md routing block reported unchanged"; else fail "second run mis-reported CLAUDE.md routing status: $OUT2"; fi
CLAUDE_HASH_B="$(shasum -a 256 "$TESTHOME/.claude/CLAUDE.md" | awk '{print $1}')"
if [ "$CLAUDE_HASH_A" = "$CLAUDE_HASH_B" ]; then pass "second run: CLAUDE.md byte-identical (no rewrite)"; else fail "second run rewrote CLAUDE.md despite unchanged status"; fi

# ================================================================================================
echo "--- 4) hand-edited harness target is protected, --force overrides it, snapshot is kept ---"
printf '\nHAND EDIT LINE\n' >> "$CODEX"
OUT3="$("$TOOL" sync 2>&1)"; RC3=$?
if [ "$RC3" -ne 0 ]; then pass "sync exits non-zero when a target was hand-edited"; else fail "sync exited 0 despite hand-edited target"; fi
if grep -q "HAND EDIT LINE" "$CODEX"; then pass "hand-edited target left untouched (not overwritten)"; else fail "hand-edited target was overwritten without --force"; fi
if echo "$OUT3" | grep -qE "^codex\s+conflict\b"; then pass "conflict reported for codex"; else fail "conflict not reported: $OUT3"; fi

OUT4="$("$TOOL" sync --force 2>&1)"; RC4=$?
if [ "$RC4" -eq 0 ]; then pass "--force run exits 0"; else fail "--force run exit $RC4: $OUT4"; fi
if ! grep -q "HAND EDIT LINE" "$CODEX"; then pass "--force overwrote the hand-edited target"; else fail "--force did not overwrite the hand-edited target"; fi
SNAP_DIR="$(ls -d "$TESTHOME"/.local/trash-snapshots/*-wb-instructions 2>/dev/null | head -1)"
if [ -n "$SNAP_DIR" ] && grep -rq "HAND EDIT LINE" "$SNAP_DIR" 2>/dev/null; then
  pass "hand-edited version was snapshotted before the forced overwrite"
else
  fail "no snapshot of the hand-edited version found under $SNAP_DIR"
fi

# ================================================================================================
echo "--- 5) --harness filter: only the requested harness is touched ---"
rm -rf "$TESTHOME/.antigravity"
"$TOOL" sync --harness codex >/dev/null 2>&1
if [ ! -e "$TESTHOME/.antigravity/AGENTS.md" ]; then pass "--harness codex left the agy target untouched"; else fail "--harness codex also wrote the agy target"; fi

# ================================================================================================
echo "--- 6) fill command available: routing block filled ADDITIVELY, curated table still untouched ---"
cat > "$BIN/wb-state" <<'STUB'
#!/bin/bash
if [ "$1" = "models" ] && [ "$2" = "table" ]; then
  echo "| Aufgabe | Modell | Harness | Effort | Eignung |"
  echo "|---|---|---|---|---|"
  echo "| mechanisch | \`stub-model\` | stub | low | Stub row from the test's fake wb-state. |"
  echo "| reasoning | \`claude-opus-5\` | claude | xhigh | Same model id the M1 regression exploded. |"
  exit 0
fi
exit 1
STUB
chmod +x "$BIN/wb-state"
# fresh fixtures: markers not inserted yet, so this exercises "empty block created + filled
# in the same run" AND -- critically -- must still leave the curated table's own rows intact.
write_claude_md
write_orchestrator_md
"$TOOL" sync >/dev/null 2>&1
if grep -q "stub-model" "$TESTHOME/.claude/CLAUDE.md"; then
  pass "routing block filled from wb-state models table when the command is available"
else
  fail "routing block was not filled even though a working wb-state was on PATH"
fi
if grep -c '| mechanisch | `haiku45:low` |' "$TESTHOME/.claude/CLAUDE.md" | grep -q '^1$' && \
   grep -c '| lang | `opus5:xhigh` |' "$TESTHOME/.claude/CLAUDE.md" | grep -q '^1$'; then
  pass "curated table rows still intact after a real fill (the exact regression this fixes)"
else
  fail "REGRESSION: curated table rows were lost/replaced when the fill command ran"
fi
if grep -q "some sub-bullet that must survive." "$TESTHOME/.claude/roles/orchestrator.md"; then
  pass "orchestrator.md unrelated content still preserved after fill"
else
  fail "orchestrator.md lost unrelated content after fill"
fi

echo "--- 6a) M1 regression: substitutions must never touch the routing-table block ---"
# The stub table has its own `claude-opus-5` cell (the exact repro from the review). Inside the
# marker block it must stay a plain, untouched table cell -- NOT exploded into the registry-hint
# sentence, and NOT nested-backtick-broken.
ROUTING_BLOCK="$(sed -n '/wb:routing-table:start/,/wb:routing-table:end/p' "$TESTHOME/.claude/CLAUDE.md")"
if echo "$ROUTING_BLOCK" | grep -q '| reasoning | `claude-opus-5` | claude | xhigh |'; then
  pass "M1: claude-opus-5 table cell inside the routing block is untouched"
else
  fail "M1 REGRESSION: claude-opus-5 table cell was rewritten inside the routing block: $ROUTING_BLOCK"
fi
if echo "$ROUTING_BLOCK" | grep -q "wb-state models list"; then
  fail "M1 REGRESSION: the registry-hint sentence leaked into the routing-table block"
else
  pass "M1: no registry-hint sentence leaked into the routing-table block"
fi
if echo "$ROUTING_BLOCK" | grep -qE '\`\`'; then
  fail "M1 REGRESSION: nested backticks found inside the routing-table block"
else
  pass "M1: no nested backticks inside the routing-table block"
fi
# fill twice more: must stabilize (idempotent refresh), not keep rewriting
HASH1="$(shasum -a 256 "$TESTHOME/.claude/CLAUDE.md" | awk '{print $1}')"
"$TOOL" sync >/dev/null 2>&1
HASH2="$(shasum -a 256 "$TESTHOME/.claude/CLAUDE.md" | awk '{print $1}')"
if [ "$HASH1" = "$HASH2" ]; then pass "filled routing block is idempotent on a third run"; else fail "filled routing block kept changing on a third run"; fi

# ================================================================================================
echo "--- 7) foreign content between the markers (e.g. leftover from the old buggy version) is a conflict ---"
write_claude_md
write_orchestrator_md
python3 - "$TESTHOME/.claude/CLAUDE.md" <<'PY'
import sys
p = sys.argv[1]
text = open(p, encoding="utf-8").read()
anchor = "- **Effort ist ein Knopf**"
idx = text.find(anchor)
foreign = ("  <!-- wb:routing-table:start -->\n"
           "  | Aufgabe | Modell |\n  |---|---|\n  | UNTAGGED | `foreign-row` |\n"
           "  <!-- wb:routing-table:end -->\n")
open(p, "w", encoding="utf-8").write(text[:idx] + foreign + text[idx:])
PY
OUT7="$("$TOOL" sync 2>&1)"; RC7=$?
if [ "$RC7" -ne 0 ]; then pass "sync exits non-zero when the routing block has foreign content"; else fail "sync exited 0 despite foreign routing-block content"; fi
if grep -q "foreign-row" "$TESTHOME/.claude/CLAUDE.md"; then pass "foreign routing-block content left untouched (not overwritten)"; else fail "foreign routing-block content was overwritten without --force"; fi
if grep -c '| mechanisch | `haiku45:low` |' "$TESTHOME/.claude/CLAUDE.md" | grep -q '^1$'; then
  pass "curated table still intact while the foreign block is in conflict"
else
  fail "curated table was touched during the foreign-block conflict"
fi
if echo "$OUT7" | grep -qE "routing-table:CLAUDE\.md\s+conflict\b"; then pass "conflict reported for CLAUDE.md routing block"; else fail "routing-block conflict not reported: $OUT7"; fi

OUT8="$("$TOOL" sync --force 2>&1)"; RC8=$?
if [ "$RC8" -eq 0 ]; then pass "--force run over a foreign routing block exits 0"; else fail "--force run exit $RC8: $OUT8"; fi
if ! grep -q "foreign-row" "$TESTHOME/.claude/CLAUDE.md"; then pass "--force replaced the foreign routing-block content"; else fail "--force did not replace the foreign content"; fi
if grep -c '| mechanisch | `haiku45:low` |' "$TESTHOME/.claude/CLAUDE.md" | grep -q '^1$'; then
  pass "curated table still intact after forcing the routing-block conflict"
else
  fail "curated table was lost when forcing the routing-block conflict"
fi
SNAP_DIR2="$(ls -d "$TESTHOME"/.local/trash-snapshots/*-wb-instructions 2>/dev/null | head -1)"
if [ -n "$SNAP_DIR2" ] && grep -rq "foreign-row" "$SNAP_DIR2" 2>/dev/null; then
  pass "foreign routing-block version was snapshotted before the forced overwrite"
else
  fail "no snapshot of the foreign routing-block version found under $SNAP_DIR2"
fi

# ================================================================================================
echo "--- 8) M1 regression, checked in a generated HARNESS TARGET (not just CLAUDE.md itself) ---"
# section 7's --force run just re-filled CLAUDE.md's routing block with the stub table (which
# still has the claude-opus-5 row) and regenerated every harness target from it. The routing
# block that ends up embedded inside e.g. ~/.codex/AGENTS.md must show the exact same guarantees.
CODEX_BLOCK="$(sed -n '/wb:routing-table:start/,/wb:routing-table:end/p' "$CODEX")"
if echo "$CODEX_BLOCK" | grep -q '| reasoning | `claude-opus-5` | claude | xhigh |'; then
  pass "M1 (harness target): claude-opus-5 table cell survives untouched inside codex's AGENTS.md"
else
  fail "M1 REGRESSION (harness target): claude-opus-5 table cell was rewritten in codex's AGENTS.md: $CODEX_BLOCK"
fi
if echo "$CODEX_BLOCK" | grep -q "wb-state models list"; then
  fail "M1 REGRESSION (harness target): registry-hint sentence leaked into codex's routing block"
else
  pass "M1 (harness target): no registry-hint sentence leaked into codex's routing block"
fi
NESTED_BACKTICKS="$(grep -c '``' "$CODEX" "$NOCOMPACT" "$CRUSH" "$AIDER" 2>/dev/null | awk -F: '{s+=$2} END{print s+0}')"
if [ "$NESTED_BACKTICKS" -eq 0 ]; then
  pass "M1: no double-backtick (nested code span) sequence in any generated harness target"
else
  fail "M1 REGRESSION: found $NESTED_BACKTICKS line(s) with nested backticks across generated targets"
fi
if grep -qE '\[keine /compact-Entsprechung fuer ' "$NOCOMPACT" && ! grep -qE '\`\[keine' "$NOCOMPACT"; then
  pass "M1: the missing-/compact note replaces the whole backtick-wrapped token, not just the bare word"
else
  fail "M1 REGRESSION: the missing-/compact note is absent or still wrapped in a leftover backtick"
fi

# ================================================================================================
echo "--- 9) H3 nacharbeit: warn on drift between the installed and the repo wb-state ---"
mkdir -p "$TESTHOME/AI/claude-workbench/shell" "$TESTHOME/.local/bin"
printf '#!/bin/bash\necho repo-version\n' > "$TESTHOME/AI/claude-workbench/shell/wb-state"
printf '#!/bin/bash\necho installed-version-DIFFERENT\n' > "$TESTHOME/.local/bin/wb-state"
chmod +x "$TESTHOME/AI/claude-workbench/shell/wb-state" "$TESTHOME/.local/bin/wb-state"
OUT9="$("$TOOL" sync --harness codex 2>&1 1>/dev/null)"
if echo "$OUT9" | grep -q "WARNUNG" && echo "$OUT9" | grep -q "wb-state"; then
  pass "H3: drift warning printed when installed and repo wb-state differ"
else
  fail "H3: no drift warning printed despite differing wb-state copies: $OUT9"
fi

cp "$TESTHOME/AI/claude-workbench/shell/wb-state" "$TESTHOME/.local/bin/wb-state"
OUT10="$("$TOOL" sync --harness codex 2>&1 1>/dev/null)"
if echo "$OUT10" | grep -q "WARNUNG"; then
  fail "H3: drift warning still printed after the two wb-state copies were made identical: $OUT10"
else
  pass "H3: no drift warning once installed and repo wb-state are identical"
fi
rm -rf "$TESTHOME/AI" "$TESTHOME/.local/bin/wb-state"

# ================================================================================================
echo
echo "RESULT: $PASS passed, $FAIL failed"
[ "$FAIL" -eq 0 ]
