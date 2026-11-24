#!/usr/bin/env bash
# bauhost.sh -- prepares an Ubuntu 26.04 build host and builds the deb there (contract rust/spec/paket, part A, "Der Bauhost").
#
#   rust/paket/bauhost.sh [--version <a.b.c>] [--ausgabe <folder>] [--geruest] [--nur-vorbereiten]
#
# Run it as a normal user with sudo on a fresh Ubuntu 26.04 (x86_64), in a checkout of the repository. A second person builds the
# same deb with the same steps (the list of packages and the toolchain are fixed below and in rust/rust-toolchain.toml).
#
# What it does:
#   1. checks the host: Ubuntu 26.04, x86_64, not root;
#   2. installs the build packages from the Ubuntu archive (sudo apt-get): build-essential, pkg-config, libssl-dev, dpkg-dev, python3,
#      curl, ca-certificates. `libssl-dev` gives the headers for the crate openssl; werkbank-lauf links the OpenSSL of the distribution
#      dynamically (the package depends on libssl3t64), so a security update of OpenSSL reaches it through apt;
#   3. installs the Rust toolchain and the gate tools into ~/.local/share/wb-rust-werkzeuge (tools/werkzeuge-installieren.sh, pinned versions);
#   4. writes `bauhost-stand.txt` into the output folder: the host (release, kernel, image serial when there is one), the version of every
#      build package and of the toolchain;
#   5. runs `rust/paket.sh deb`.
#
# Exit codes: 0 built; 64 usage error; 127 the host is not suitable; otherwise the exit code of the failed step (paket.sh: 65, 70, 127).
set -u
HERE="$(cd "$(dirname "$0")" && pwd)" || exit 64
RUST="$(cd "$HERE/.." && pwd)" || exit 64

VERSION="0.1.0"
AUSGABE="$RUST/target/paket-ausgabe"
OPT=""
NUR_VORBEREITEN=0
usage() { echo "usage: $0 [--version <a.b.c>] [--ausgabe <folder>] [--geruest] [--nur-vorbereiten]" >&2; exit 64; }
while [ $# -gt 0 ]; do
  case "$1" in
    --version) [ $# -ge 2 ] || usage; VERSION="$2"; shift 2 ;;
    --ausgabe) [ $# -ge 2 ] || usage; AUSGABE="$2"; shift 2 ;;
    --geruest) OPT="--geruest"; shift ;;
    --nur-vorbereiten) NUR_VORBEREITEN=1; shift ;;
    *) usage ;;
  esac
done
case "$AUSGABE" in /*) ;; *) AUSGABE="$PWD/$AUSGABE" ;; esac

# ---- 1: the host ---------------------------------------------------------------------------------------------------
[ "$(id -u)" -ne 0 ] || { echo "bauhost.sh: run it as a normal user (it calls sudo itself)" >&2; exit 127; }
[ "$(uname -m)" = "x86_64" ] || { echo "bauhost.sh: the host is not x86_64" >&2; exit 127; }
# /etc/os-release defines VERSION itself: read it in a subshell.
OS_ID="$(. /etc/os-release 2>/dev/null; echo "${ID:-}")"
OS_VERSION_ID="$(. /etc/os-release 2>/dev/null; echo "${VERSION_ID:-}")"
OS_NAME="$(. /etc/os-release 2>/dev/null; echo "${PRETTY_NAME:-unknown}")"
[ "$OS_ID" = "ubuntu" ] && [ "$OS_VERSION_ID" = "26.04" ] || { echo "bauhost.sh: the host is not Ubuntu 26.04 (found $OS_NAME)" >&2; exit 127; }

# ---- 2: build packages from the Ubuntu archive ---------------------------------------------------------------------------
PAKETE="build-essential pkg-config libssl-dev dpkg-dev python3 curl ca-certificates"
# shellcheck disable=SC2086
sudo DEBIAN_FRONTEND=noninteractive apt-get install -y $PAKETE || { echo "bauhost.sh: apt-get install failed" >&2; exit 70; }

# ---- 3: toolchain and gate tools -------------------------------------------------------------------------------------------
ORT="$HOME/.local/share/wb-rust-werkzeuge"
bash "$RUST/tools/werkzeuge-installieren.sh" --install --target "$ORT" || { echo "bauhost.sh: the tool installation failed" >&2; exit 70; }

# ---- 4: the state of the host --------------------------------------------------------------------------------------------
mkdir -p "$AUSGABE" || exit 70
{
  echo "host: $OS_NAME"
  echo "kernel: $(uname -r)"
  [ -r /etc/cloud/build.info ] && sed 's/^/image: /' /etc/cloud/build.info
  echo "toolchain-file: $(sed -n 's/^channel[[:space:]]*=[[:space:]]*"\([^"]*\)".*/\1/p' "$RUST/rust-toolchain.toml")"
  echo "cargo: $(PATH="$ORT/cargo/bin:$PATH" RUSTUP_HOME="$ORT/rustup" CARGO_HOME="$ORT/cargo" cargo --version 2>&1)"
  echo "rustc: $(PATH="$ORT/cargo/bin:$PATH" RUSTUP_HOME="$ORT/rustup" CARGO_HOME="$ORT/cargo" rustc --version 2>&1)"
  for t in deny cyclonedx mutants; do
    echo "cargo-$t: $(PATH="$ORT/cargo/bin:$ORT/tools/bin:$PATH" RUSTUP_HOME="$ORT/rustup" CARGO_HOME="$ORT/cargo" cargo "$t" --version 2>&1 | head -n 1)"
  done
  # shellcheck disable=SC2086
  dpkg-query -W -f='package: ${Package} ${Version}\n' $PAKETE gcc binutils libc6-dev libssl3t64 2>&1
  echo "gcc: $(gcc --version | head -n 1)"
  echo "python3: $(python3 --version 2>&1)"
} > "$AUSGABE/bauhost-stand.txt"

[ "$NUR_VORBEREITEN" -eq 0 ] || exit 0

# ---- 5: the build ---------------------------------------------------------------------------------------------------------
# shellcheck disable=SC2086
"$RUST/paket.sh" deb --version "$VERSION" --ausgabe "$AUSGABE" $OPT
exit $?
