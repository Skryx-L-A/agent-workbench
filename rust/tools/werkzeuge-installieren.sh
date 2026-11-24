#!/usr/bin/env bash
# werkzeuge-installieren.sh -- installs the tools of the Rust gates (rust/gates.sh) into one folder.
#
#   werkzeuge-installieren.sh [--dry-run] [--target DIR]     default: shows what it would do, writes nothing
#   werkzeuge-installieren.sh --install --target DIR         installs; DIR must be an absolute path
#
# The folder for the real installation is ~/.local/share/wb-rust-werkzeuge (one per machine). Layout:
#   rustup/    RUSTUP_HOME: the toolchain of ../rust-toolchain.toml (profile minimal, rustfmt, clippy)
#   cargo/     CARGO_HOME: cargo, rustc and the other proxies in cargo/bin, the crates.io cache
#   tools/     cargo-deny, cargo-cyclonedx and cargo-mutants (`cargo install --locked --root`), in tools/bin
#   download/  the rustup installer as downloaded and checked
#
# What the script does and does not do:
#   - The rustup installer is never piped into a shell. It is downloaded to download/, its SHA-256 is compared
#     with the value pinned below, and only then is it started. A wrong sum stops the script (exit 2) before the
#     installer runs.
#   - Sources: static.rust-lang.org (the installer and, through rustup, the toolchain) and crates.io (through
#     cargo install). No other host.
#   - The installer runs with --no-modify-path: no shell profile is touched. The Homebrew Rust and any rustup of
#     the user stay as they are; PATH of this script is the folder plus /usr/bin:/bin:/usr/sbin:/sbin.
#   - No sudo; as root the script stops.
#   - It checks that nothing outside the folder was written: ~/.cargo, ~/.rustup and the shell profiles are the
#     same afterwards (a missing one must still be missing; cargo's own cache bookkeeping files .global-cache* and
#     .package-cache* are left out, because any concurrent cargo run changes them), and the folder holds only the
#     entries above.
#   - Idempotent: a part that is already there in the pinned version is skipped; a complete folder gives
#     "nothing to do" and no download.
#   - The final check is `gates.sh tools`, so the tools are accepted by the same code that later uses them.
#
# Exit codes: 0 ok; 1 failure; 2 checksum mismatch; 64 usage error.
# Test seam (only while WB_TESTUMGEBUNG=1): WB_RUSTUP_URL_BASE replaces the installer base URL (file:// allowed),
# WB_RUSTUP_SHA256 the pinned installer checksum.
# Runs on bash 3.2 (macOS) and current bash.

set -u
set -o pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)" || exit 64
RUST_DIR="$(cd "$SCRIPT_DIR/.." && pwd)" || exit 64
GATES="$RUST_DIR/gates.sh"
TOOLCHAIN_FILE="$RUST_DIR/rust-toolchain.toml"

# ---- pins (installer: published at the URL below; copies of the tool versions: gates.sh, README.md) ----
RUSTUP_INIT_VERSION="1.29.3"
RUSTUP_INIT_URL_BASE="https://static.rust-lang.org/rustup/archive"
# Published next to each installer as <url>.sha256; taken from there on 2026-11-18 and, for the Linux one, also
# measured on the rustup 1.29.3 binary that is installed on the Linux build host (same sum).
RUSTUP_INIT_SHA256_aarch64_apple_darwin="63752a1919a3a4793173ec70f8f212621a7ce38ba6a246095984482f4de20f9f"
RUSTUP_INIT_SHA256_x86_64_unknown_linux_gnu="b1c8cb14e30889e7e2999e6b69e62d9e9b9ffc0f4df8180564d306d9680e6c55"
TOOL_DENY="0.21.0"
TOOL_CYCLONEDX="0.5.10"
TOOL_MUTANTS="27.1.0"

die() { echo "werkzeuge-installieren: $*" >&2; exit 1; }
usage() {
  echo "usage: $0 [--dry-run] [--target DIR]" >&2
  echo "       $0 --install --target DIR      (DIR absolute; the real one is \$HOME/.local/share/wb-rust-werkzeuge)" >&2
  exit 64
}

MODE="dry"
TARGET=""
while [ "$#" -gt 0 ]; do
  case "$1" in
    --install) MODE="install" ;;
    --dry-run) MODE="dry" ;;
    --target)
      [ "$#" -ge 2 ] || usage
      TARGET="$2"; shift ;;
    *) usage ;;
  esac
  shift
done

[ -n "${HOME:-}" ] || [ -n "$TARGET" ] || die "HOME is not set and no --target was given"
DEFAULT_TARGET="${HOME:-}/.local/share/wb-rust-werkzeuge"
if [ "$MODE" = "install" ] && [ -z "$TARGET" ]; then
  echo "werkzeuge-installieren: --install needs --target DIR" >&2
  usage
fi
[ -n "$TARGET" ] || TARGET="$DEFAULT_TARGET"
case "$TARGET" in
  /*) ;;
  *) echo "werkzeuge-installieren: the target '$TARGET' is not an absolute path" >&2; exit 64 ;;
esac
case "$TARGET" in
  / | /usr | /usr/* | /bin | /bin/* | /etc | /etc/* | /opt/homebrew | /opt/homebrew/*) die "refusing the target $TARGET" ;;
esac
HOME_DIR="${HOME:-}"
if [ -n "$HOME_DIR" ]; then
  case "$TARGET/" in
    "$HOME_DIR/.cargo/"* | "$HOME_DIR/.rustup/"*) die "refusing the target $TARGET: it lies in the cargo or rustup folder of the user" ;;
  esac
  [ "$TARGET" != "$HOME_DIR" ] || die "refusing the target $TARGET: that is the home folder"
fi

# ---- platform ---------------------------------------------------------------------------------------------
case "$(uname -s)/$(uname -m)" in
  Darwin/arm64) TRIPLE="aarch64-apple-darwin"; EXPECTED_SHA="$RUSTUP_INIT_SHA256_aarch64_apple_darwin" ;;
  Linux/x86_64) TRIPLE="x86_64-unknown-linux-gnu"; EXPECTED_SHA="$RUSTUP_INIT_SHA256_x86_64_unknown_linux_gnu" ;;
  *) die "no pinned installer checksum for the platform $(uname -s)/$(uname -m) (pinned: aarch64-apple-darwin, x86_64-unknown-linux-gnu)" ;;
esac

CHANNEL="$(sed -n 's/^channel[[:space:]]*=[[:space:]]*"\([^"]*\)".*/\1/p' "$TOOLCHAIN_FILE" 2>/dev/null | head -n 1)"
[ -n "$CHANNEL" ] || die "no channel in $TOOLCHAIN_FILE"
[ -f "$GATES" ] || die "$GATES not found"

URL_BASE="$RUSTUP_INIT_URL_BASE"
CURL_PROTO="=https"
if [ "${WB_TESTUMGEBUNG:-}" = "1" ] && [ -n "${WB_RUSTUP_URL_BASE:-}" ]; then
  URL_BASE="$WB_RUSTUP_URL_BASE"
  CURL_PROTO="=https,file"
  [ -z "${WB_RUSTUP_SHA256:-}" ] || EXPECTED_SHA="$WB_RUSTUP_SHA256"
fi
INIT_URL="$URL_BASE/$RUSTUP_INIT_VERSION/$TRIPLE/rustup-init"

sha256_of() {
  if command -v sha256sum >/dev/null 2>&1; then sha256sum "$1" | cut -d' ' -f1
  elif command -v shasum >/dev/null 2>&1; then shasum -a 256 "$1" | cut -d' ' -f1
  else die "neither sha256sum nor shasum is available"
  fi
}

# Which items of the folder are ok? Asks gates.sh, the code that uses them later. Prints "toolchain deny ..." of
# the items that are missing or of another version (nothing printed: complete).
missing_items() {
  local out item miss=""
  out="$(env WB_TESTUMGEBUNG=1 WB_RUST_WERKZEUGE_ORT="$TARGET" bash "$GATES" tools 2>/dev/null)"
  for item in toolchain deny cyclonedx mutants; do
    if ! grep -qx "tools: $item ok" <<<"$out"; then miss="$miss $item"; fi
  done
  printf '%s\n' "${miss# }"
}

# ---- guard: nothing outside the folder -------------------------------------------------------------------
GUARD_PATHS=""
if [ -n "$HOME_DIR" ]; then
  GUARD_PATHS="$HOME_DIR/.cargo $HOME_DIR/.rustup $HOME_DIR/.profile $HOME_DIR/.bash_profile $HOME_DIR/.bashrc $HOME_DIR/.zshrc $HOME_DIR/.zshenv $HOME_DIR/.zprofile $HOME_DIR/.config/fish/config.fish"
fi
guard_state() {  # $1 marker file; prints one line per guarded path
  local p
  for p in $GUARD_PATHS; do
    if [ -d "$p" ]; then
      # .global-cache* and .package-cache* are the bookkeeping of any cargo run that uses this folder at the same
      # time (measured on the Linux build host 2026-11-18: another user of ~/.cargo changed .global-cache during the install);
      # folders are not compared, because a changed file also changes the time of its folder.
      echo "dir $p changed=$(find "$p" -mindepth 1 ! -type d -newer "$1" ! -name '.global-cache*' ! -name '.package-cache*' 2>/dev/null | head -n 1 | wc -l | tr -d ' ')"
    elif [ -e "$p" ]; then
      echo "file $p sum=$(cksum < "$p" 2>/dev/null)"
    else
      echo "missing $p"
    fi
  done
}

# ---- plan --------------------------------------------------------------------------------------------------
echo "werkzeuge-installieren: mode $MODE, target $TARGET, platform $TRIPLE"
if [ -d "$TARGET" ]; then
  MISS="$(missing_items)"
else
  MISS="toolchain deny cyclonedx mutants"
fi
if [ -z "$MISS" ]; then
  echo "werkzeuge-installieren: toolchain $CHANNEL, cargo-deny $TOOL_DENY, cargo-cyclonedx $TOOL_CYCLONEDX and cargo-mutants $TOOL_MUTANTS are in place: nothing to do"
  exit 0
fi
echo "werkzeuge-installieren: missing or of another version: $MISS"
for item in $MISS; do
  case "$item" in
    toolchain)
      echo "  would download $INIT_URL to $TARGET/download/"
      echo "  would compare its SHA-256 with $EXPECTED_SHA and stop on a mismatch"
      echo "  would run it: RUSTUP_HOME=$TARGET/rustup CARGO_HOME=$TARGET/cargo rustup-init -y --no-modify-path --profile minimal --default-toolchain $CHANNEL -c rustfmt -c clippy" ;;
    deny) echo "  would run: cargo install --locked --root $TARGET/tools cargo-deny --version =$TOOL_DENY" ;;
    cyclonedx) echo "  would run: cargo install --locked --root $TARGET/tools cargo-cyclonedx --version =$TOOL_CYCLONEDX" ;;
    mutants) echo "  would run: cargo install --locked --root $TARGET/tools cargo-mutants --version =$TOOL_MUTANTS" ;;
  esac
done
if [ "$MODE" = "dry" ]; then
  echo "werkzeuge-installieren: dry run, nothing written. To install: $0 --install --target $TARGET"
  exit 0
fi

# ---- install -----------------------------------------------------------------------------------------------
[ "$(id -u)" -ne 0 ] || die "refusing to run as root (no sudo, no root-owned tool folder)"

mkdir -p "$TARGET" || die "cannot create $TARGET"
TARGET="$(cd "$TARGET" && pwd)" || die "cannot enter $TARGET"
MARKER="$TARGET/.guard-marker"
: > "$MARKER" || die "cannot write $MARKER"
BEFORE="$(guard_state "$MARKER")"

# Environment of every step: the folder and a PATH without the Homebrew Rust or anything else of the user.
unset RUSTUP_TOOLCHAIN CARGO RUSTC RUSTDOC RUSTFMT RUSTC_WRAPPER RUSTC_WORKSPACE_WRAPPER CARGO_INSTALL_ROOT CARGO_BUILD_RUSTC CARGO_TARGET_DIR
export RUSTUP_HOME="$TARGET/rustup" CARGO_HOME="$TARGET/cargo" RUSTUP_INIT_SKIP_PATH_CHECK=yes RUSTUP_AUTO_INSTALL=0
export PATH="$TARGET/cargo/bin:$TARGET/tools/bin:/usr/bin:/bin:/usr/sbin:/sbin"

if grep -qw toolchain <<<"$MISS"; then
  mkdir -p "$TARGET/download" || die "cannot create $TARGET/download"
  INIT_FILE="$TARGET/download/rustup-init-$RUSTUP_INIT_VERSION-$TRIPLE"
  rm -f "$INIT_FILE"
  echo "werkzeuge-installieren: downloading $INIT_URL"
  curl --fail --silent --show-error --location --proto "$CURL_PROTO" --max-time 300 --output "$INIT_FILE" "$INIT_URL" \
    || die "download of $INIT_URL failed"
  GOT="$(sha256_of "$INIT_FILE")"
  if [ "$GOT" != "$EXPECTED_SHA" ]; then
    rm -f "$INIT_FILE"
    echo "werkzeuge-installieren: CHECKSUM MISMATCH for $INIT_URL: got $GOT, pinned $EXPECTED_SHA; the installer was not started" >&2
    exit 2
  fi
  echo "werkzeuge-installieren: installer checksum ok ($GOT)"
  chmod 700 "$INIT_FILE" || die "chmod failed"
  "$INIT_FILE" -y --no-modify-path --profile minimal --default-toolchain "$CHANNEL" -c rustfmt -c clippy \
    || die "rustup-init failed"
  # The tools may already be there; ask again now that the toolchain is.
  MISS="$(missing_items)"
fi

for item in $MISS; do
  case "$item" in
    toolchain) die "the toolchain $CHANNEL is still missing after rustup-init (see gates.sh tools)" ;;
    deny) VER="$TOOL_DENY" ;;
    cyclonedx) VER="$TOOL_CYCLONEDX" ;;
    mutants) VER="$TOOL_MUTANTS" ;;
  esac
  # Not inside rust/: no rust-toolchain.toml may pick another toolchain; the default one is the pinned channel.
  ( cd "$TARGET" && cargo install --locked --root "$TARGET/tools" --target-dir "$TARGET/build" "cargo-$item" --version "=$VER" ) \
    || die "cargo install cargo-$item $VER failed"
done

# The build directory is scratch: removed (exactly this path).
[ -d "$TARGET/build" ] && rm -rf -- "$TARGET/build"

# ---- checks -------------------------------------------------------------------------------------------------
STILL="$(missing_items)"
[ -z "$STILL" ] || die "after the installation these items are still missing or of another version: $STILL (see gates.sh tools)"

AFTER="$(guard_state "$MARKER")"
rm -f "$MARKER"
if [ "$BEFORE" != "$AFTER" ]; then
  echo "werkzeuge-installieren: something outside $TARGET changed:" >&2
  diff <(printf '%s\n' "$BEFORE") <(printf '%s\n' "$AFTER") >&2
  exit 1
fi
for entry in "$TARGET"/* "$TARGET"/.[!.]*; do
  [ -e "$entry" ] || continue
  case "${entry##*/}" in
    rustup | cargo | tools | download) ;;
    *) die "unexpected entry in the tool folder: $entry" ;;
  esac
done
echo "werkzeuge-installieren: done: toolchain $CHANNEL, cargo-deny $TOOL_DENY, cargo-cyclonedx $TOOL_CYCLONEDX, cargo-mutants $TOOL_MUTANTS in $TARGET; nothing outside it changed"
