#!/bin/bash
# Claude Code statusline — one line, color-coded:
#   <model> · <dir> <branch> · ▓▓▓░░░░░░░ 46k/1.0M · 5h 12%→14:00 · 7d 3% · $0.42
input=$(cat)
J() { echo "$input" | /opt/homebrew/bin/jq -r "$1 // empty" 2>/dev/null; }

DIM=$'\e[2m'; GRN=$'\e[32m'; YLW=$'\e[33m'; RED=$'\e[31m'; CYN=$'\e[36m'; RST=$'\e[0m'
SEP="${DIM} · ${RST}"

fmt() { n=$1
  if [ "$n" -ge 1000000 ]; then printf '%d.%dM' $((n/1000000)) $(((n%1000000)/100000))
  elif [ "$n" -ge 1000 ]; then printf '%dk' $((n/1000))
  else printf '%d' "$n"; fi; }

col_pct() { # color by percentage
  p=${1:-0}
  if [ "$p" -ge 85 ]; then printf '%s' "$RED"
  elif [ "$p" -ge 60 ]; then printf '%s' "$YLW"
  else printf '%s' "$GRN"; fi; }

model=$(J '.model.display_name' | awk '{print $1}')
dir=$(J '.workspace.current_dir'); dir=${dir:-$PWD}
branch=$(git -C "$dir" branch --show-current 2>/dev/null)

effort=$(J '.effort.level')
mline="${model:-Claude}"; [ -n "$effort" ] && mline="$mline ${effort}"
sdir=${dir/#$HOME/~}
if [ ${#sdir} -gt 32 ]; then sdir="…/$(basename "$(dirname "$sdir")")/$(basename "$sdir")"; fi
if [ ${#sdir} -gt 32 ]; then sdir="…/$(basename "$sdir")"; fi
out="${CYN}${mline}${RST}${SEP}${sdir}"
[ -n "$branch" ] && out="$out ${DIM}${branch}${RST}"

used=$(J '.context_window.total_input_tokens'); size=$(J '.context_window.context_window_size')
if [ -n "$used" ] && [ -n "$size" ] && [ "$size" -gt 0 ]; then
  pct=$((used * 100 / size)); c=$(col_pct "$pct")
  filled=$((pct / 10)); [ "$filled" -gt 10 ] && filled=10
  bar=""; i=0
  while [ $i -lt 10 ]; do
    if [ $i -lt $filled ]; then bar="${bar}▓"; else bar="${bar}░"; fi; i=$((i+1))
  done
  out="$out$SEP${c}${bar} $(fmt "$used")/$(fmt "$size")${RST}"
fi

h5=$(J '.rate_limits.five_hour.used_percentage' | cut -d. -f1)
h5r=$(J '.rate_limits.five_hour.resets_at')
if [ -n "$h5" ]; then
  reset=""; [ -n "$h5r" ] && reset="→$(date -r "$h5r" +%H:%M 2>/dev/null)"
  out="$out$SEP$(col_pct "$h5")5h ${h5}%${reset}${RST}"
fi
d7=$(J '.rate_limits.seven_day.used_percentage' | cut -d. -f1)
[ -n "$d7" ] && out="$out$SEP$(col_pct "$d7")7d ${d7}%${RST}"

echo "$out"
