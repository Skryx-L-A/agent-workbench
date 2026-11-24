#!/usr/bin/env bash
# bauen.sh [--aus <ordner>]: builds the browser view (crate `ansicht`) into a flat folder with exactly
#   index.html  ansicht.js  ansicht_bg.wasm
# Default folder is rust/ansicht/ausgabe/ (checked in). Steps:
#   1 cargo build -p ansicht --target wasm32-unknown-unknown --profile ansicht --locked (own target dir)
#   2 wasm-bindgen --target web --out-name ansicht (the version of wasm-bindgen in Cargo.lock)
#   3 node buendeln.mjs: inline the snippets of wasm-bindgen into ansicht.js, start the module, write index.html
# The tools come from the fixed tool folder (gates.sh): ~/.local/share/wb-rust-werkzeuge, wasm-bindgen from
# ~/.local/share/wb-rust-werkzeuge/wasm-bindgen-<version>/bin. Paths of the build tree are remapped, so that the
# output is the same byte for byte in every folder (test/gleichheit.sh).
# Exit: 0 built; 1 a step failed; 64 usage; 127 a tool is missing.
set -u
set -o pipefail

HIER="$(cd "$(dirname "$0")" && pwd)" || exit 64
RUST="$(cd "$HIER/.." && pwd)" || exit 64
AUS="$HIER/ausgabe"
while [ $# -gt 0 ]; do
  case "$1" in
    --aus) AUS="${2:?--aus braucht einen Ordner}"; shift 2 ;;
    *) echo "bauen.sh: unbekannt: $1" >&2; exit 64 ;;
  esac
done
case "$AUS" in /*) ;; *) AUS="$PWD/$AUS" ;; esac

W="${HOME:?}/.local/share/wb-rust-werkzeuge"
[ -d "$W" ] || { echo "bauen.sh: Werkzeugordner $W fehlt (gates.sh tools)" >&2; exit 127; }
unset RUSTUP_TOOLCHAIN CARGO RUSTC RUSTDOC RUSTFMT RUSTC_WRAPPER RUSTC_WORKSPACE_WRAPPER CARGO_INSTALL_ROOT CARGO_BUILD_RUSTC
export RUSTUP_HOME="$W/rustup" CARGO_HOME="$W/cargo" RUSTUP_AUTO_INSTALL=0
PATH="$W/cargo/bin:$PATH"
export PATH

FASSUNG="$(sed -n '/^name = "wasm-bindgen"$/{n;s/^version = "\(.*\)"$/\1/p;}' "$RUST/Cargo.lock")"
[ -n "$FASSUNG" ] || { echo "bauen.sh: wasm-bindgen fehlt in Cargo.lock" >&2; exit 1; }
WB="$W/wasm-bindgen-$FASSUNG/bin/wasm-bindgen"
[ -x "$WB" ] || { echo "bauen.sh: $WB fehlt (cargo install --locked --root $W/wasm-bindgen-$FASSUNG wasm-bindgen-cli --version =$FASSUNG)" >&2; exit 127; }
command -v node > /dev/null || { echo "bauen.sh: node fehlt" >&2; exit 127; }

# The same output in every folder: the build tree and the registry are remapped to fixed names.
export RUSTFLAGS="--remap-path-prefix=$RUST=/werkbank/rust --remap-path-prefix=$CARGO_HOME=/cargo"
cd "$RUST" || exit 1
cargo build -p ansicht --bin ansicht --target wasm32-unknown-unknown --profile ansicht --locked \
  --target-dir "$RUST/target/ansicht" || exit 1

ROH="$(mktemp -d "${TMPDIR:-/tmp}/ansicht-roh-XXXXXX")" || exit 1
trap 'rm -rf "$ROH"' EXIT
"$WB" --target web --out-name ansicht --no-typescript --out-dir "$ROH" \
  "$RUST/target/ansicht/wasm32-unknown-unknown/ansicht/ansicht.wasm" || exit 1

mkdir -p "$AUS" || exit 1
node "$HIER/buendeln.mjs" "$ROH" "$AUS" || exit 1
ls -l "$AUS"
