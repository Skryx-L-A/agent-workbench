#!/usr/bin/env bash
# paket.sh -- builds the package of the Rust workspace (contract rust/spec/paket, part A: the deb for Ubuntu 26.04).
#
#   ./paket.sh deb [--version <a.b.c>] [--ausgabe <folder>] [--geruest]
#
# The host is an Ubuntu 26.04 on x86_64 (the linked libraries must match the target; the deb is built where it will run).
# Steps (contract, "Der Befehl zum Bauen"): 1 tools, 2 cargo build (and the static build of the starter and the forwarder of the enclosure), 3 licence gate and bill of materials, 4 stage the tree and
# check the lists and the profiles, 5 control file, 6 manifest, 7 dpkg-deb, 8 checksum.
# `--geruest` builds the skeleton: it holds the programs that exist on this branch, names the missing ones in the report
# `werkbank_<version>_amd64.bericht.json` and is a test build (`bau: test`); a skeleton is never for the company laptop.
# Without `--geruest` a missing source stops the build (exit 65).
#
# Result in --ausgabe (default target/paket-ausgabe, below the ignored build folder):
#   werkbank_<version>_amd64.deb, .deb.sha256, .manifest.json, .stueckliste.tsv, .bericht.json
#   (with --geruest the version in every name is <a.b.c>~geruest, as in the control file)
# Exit codes: 0 built and checked; 64 usage error; 65 the package violates the contract (a `paket.*` code on stderr);
# 70 build error; 127 a tool is missing or has another version than pinned.

set -u
# The modes of the package do not depend on the umask of the builder: the build runs under 022 and `paket/bau.py` sets and checks
# the mode of every entry (contract, stage 9 of the check of the lists).
umask 022

HERE="$(cd "$(dirname "$0")" && pwd)" || exit 64
cd "$HERE" || exit 64

usage() {
  echo "usage: ./paket.sh deb [--version <a.b.c>] [--ausgabe <folder>] [--geruest]" >&2
  exit 64
}

[ "${1:-}" = "deb" ] || usage
shift
VERSION="0.1.0"
AUSGABE="$HERE/target/paket-ausgabe"
GERUEST=0
while [ $# -gt 0 ]; do
  case "$1" in
    --version) [ $# -ge 2 ] || usage; VERSION="$2"; shift 2 ;;
    --ausgabe) [ $# -ge 2 ] || usage; AUSGABE="$2"; shift 2 ;;
    --geruest) GERUEST=1; shift ;;
    *) usage ;;
  esac
done
case "$VERSION" in
  *[!0-9.]* | "" | .* | *. | *..*) usage ;;
esac
case "$AUSGABE" in /*) ;; *) AUSGABE="$PWD/$AUSGABE" ;; esac

START=$(date +%s)
STUFE="$HERE/target/paket-stufe"
TARGET="x86_64-unknown-linux-gnu"
BINDIR="$HERE/target/$TARGET/release"

# ---- step 1: tools --------------------------------------------------------------------------------------------------
mkdir -p "$HERE/target" || exit 70
./gates.sh tools > "$HERE/target/paket-werkzeuge.txt" 2>&1
rc=$?
if [ "$rc" -ne 0 ]; then
  echo "paket.sh: the tool folder is not complete (gates.sh tools exit $rc); see README.md, section Tools" >&2
  exit 127
fi
for t in python3 dpkg-deb dpkg-shlibdeps sha256sum; do
  command -v "$t" >/dev/null 2>&1 || { echo "paket.sh: $t is missing (packages python3, dpkg, dpkg-dev, coreutils)" >&2; exit 127; }
done
[ "$(uname -m)" = "x86_64" ] || { echo "paket.sh: the host is not x86_64" >&2; exit 127; }

# The gates run with the tools of the fixed folder; the build uses the same folder and the same toolchain.
ORT="${HOME}/.local/share/wb-rust-werkzeuge"
if [ "${WB_TESTUMGEBUNG:-}" = "1" ] && [ -n "${WB_RUST_WERKZEUGE_ORT:-}" ]; then ORT="$WB_RUST_WERKZEUGE_ORT"; fi
unset RUSTUP_TOOLCHAIN CARGO RUSTC RUSTDOC RUSTFMT RUSTC_WRAPPER RUSTC_WORKSPACE_WRAPPER CARGO_INSTALL_ROOT CARGO_BUILD_RUSTC
export RUSTUP_HOME="$ORT/rustup" CARGO_HOME="$ORT/cargo" RUSTUP_AUTO_INSTALL=0
PATH="$ORT/cargo/bin:$ORT/tools/bin:$PATH"
export PATH

# ---- step 2: build --------------------------------------------------------------------------------------------------
# No feature of the workspace is switched on: neither `attrappe` nor a feature that starts with `test`.
CRATES="$(python3 -I paket/bau.py crates --rust "$HERE")"
rc=$?
[ "$rc" -eq 0 ] && [ -n "$CRATES" ] || { echo "paket.sh: no crate to build (rc $rc)" >&2; exit 70; }
# shellcheck disable=SC2086
cargo build --release --locked --target "$TARGET" $CRATES
rc=$?
[ "$rc" -eq 0 ] || { echo "paket.sh: cargo build failed (exit $rc)" >&2; exit 70; }

# The programs of referenz.STATISCH (the fixed starter, einschluss-weiter) are linked static (contract, "Statischer Bau"): a second
# build with `-C target-feature=+crt-static` in its own target folder, so the dynamic build above stays as it is.
STATISCH="$(python3 -I paket/bau.py statisch --rust "$HERE")"
rc=$?
[ "$rc" -eq 0 ] || { echo "paket.sh: the list of static programs failed (rc $rc)" >&2; exit 70; }
BINDIR_STATISCH="$HERE/target/statisch/$TARGET/release"
if [ -n "$STATISCH" ]; then
  # shellcheck disable=SC2086
  RUSTFLAGS="-C target-feature=+crt-static" cargo build --release --locked --target "$TARGET" --target-dir "$HERE/target/statisch" $STATISCH
  rc=$?
  [ "$rc" -eq 0 ] || { echo "paket.sh: the static cargo build failed (exit $rc)" >&2; exit 70; }
fi

# ---- step 3: licences and bill of materials ---------------------------------------------------------------------------
./gates.sh licenses
rc=$?
[ "$rc" -eq 0 ] || exit "$rc"

# ---- steps 4 to 6: stage, check lists, profiles, bill of materials, control file and manifest ---------------------------
mkdir -p "$AUSGABE" || exit 70
OPT=""
[ "$GERUEST" -eq 1 ] && OPT="--geruest"
# shellcheck disable=SC2086
python3 -I paket/bau.py stufen --rust "$HERE" --bindir "$BINDIR" --bindir-statisch "$BINDIR_STATISCH" --stufe "$STUFE" --ausgabe "$AUSGABE" --version "$VERSION" $OPT \
  > "$AUSGABE/bau-bericht.json"
rc=$?
[ "$rc" -eq 0 ] || exit "$rc"

# ---- step 7: dpkg-deb -------------------------------------------------------------------------------------------------
# The file name follows the version of the control file: a skeleton build is `werkbank_<a.b.c>~geruest_amd64.deb`.
DEBVERSION="$(python3 -I paket/bau.py version --version "$VERSION" $OPT)" || exit 70
DEB="werkbank_${DEBVERSION}_amd64.deb"
dpkg-deb --build --root-owner-group "$STUFE" "$AUSGABE/$DEB"
rc=$?
[ "$rc" -eq 0 ] || { echo "paket.sh: dpkg-deb failed (exit $rc)" >&2; exit 70; }

# ---- step 8: checksum -------------------------------------------------------------------------------------------------
(cd "$AUSGABE" && sha256sum "$DEB" > "$DEB.sha256")
rc=$?
[ "$rc" -eq 0 ] || { echo "paket.sh: sha256sum failed (exit $rc)" >&2; exit 70; }
rm -f "$AUSGABE/bau-bericht.json"

echo "$AUSGABE/$DEB"
echo "paket.sh: built in $(( $(date +%s) - START )) s" >&2
exit 0
