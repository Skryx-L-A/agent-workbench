# agent-workbench — development

English | [Deutsch](README.de.md)

**This branch holds the product being built: the agent workbench rebuilt in Rust.** The tested
workbench that is in use today, an Electron window with command-line tools, is on the branch
[`main`](https://github.com/Skryx-L-A/agent-workbench/tree/main).

## What it is

The rebuild turns the workbench into a package that a company can install on its machines. The
Rust workspace in `rust/` holds its building blocks, among them:

- **a core service** (`wb-dienst`) that runs each session in its own terminal and serves a view of
  it to every viewer, with a state core and a screen extract that send only what changed;
- **a browser view** (`ansicht`, built for WebAssembly) and a bridge (`wb-bruecke`) that serves it
  on `127.0.0.1` only;
- **an enclosure per session** on Linux, built from a fixed bubblewrap profile, with a start
  program that checks the start file and the settings before anything runs;
- **a local model path** that lets one route and one model through and keeps the key outside the
  enclosure;
- **a search core** over Markdown notes (BM25 and vectors, no model in the process) with a small
  MCP tool for agents;
- **a package build** (`rust/paket.sh`) for a Debian package and a check command for the installed
  package.

`rust/README.md` describes each crate and its contract.

## Status

Work in progress, **not released for production use.** There is no installable release yet. The
package build targets Ubuntu 26.04 on x86_64 and so far produces a test build only. Some
protections that a company deployment needs are not complete; [SECURITY.md](SECURITY.md) lists
the open points. Do not run this branch on a machine that holds other people's data.

[CHANGELOG.md](CHANGELOG.md) lists the changes of each published state.

## Branches

- `main` is the tested workbench. It is updated only after the full test run of the source tree
  has passed for exactly the state that is published.
- `development` (this branch) is the product being built. It is updated after the Rust gates
  below have passed.
- Each update of either branch is a snapshot. The private working history is not published.

## Building

You need [rustup](https://rustup.rs). The toolchain version is fixed in `rust/rust-toolchain.toml`
(currently 1.99.0 with rustfmt and clippy); rustup installs it on the first `cargo` call.

```bash
git clone --branch development https://github.com/Skryx-L-A/agent-workbench.git
cd agent-workbench/rust
cargo build --workspace --locked
```

The browser view is built for `wasm32-unknown-unknown`; see the section `ansicht` in
`rust/README.md`.

## Gates

Every published state has passed the gates of `rust/gates.sh`. The script runs one gate per call.

**The tool folder.** The gates use their tools only from one folder per machine,
`~/.local/share/wb-rust-werkzeuge`: `rustup/` is `RUSTUP_HOME`, `cargo/` is `CARGO_HOME`, and
`tools/` holds cargo-deny 0.21.0, cargo-cyclonedx 0.5.10 and cargo-mutants 27.1.0. `gates.sh`
sets `PATH`, `RUSTUP_HOME` and `CARGO_HOME` for its own run only and never uses another Rust on the
machine. A missing folder, toolchain or tool version stops a gate with exit 127; `./gates.sh tools`
makes only this check. `rust/tools/werkzeuge-installieren.sh` makes the folder:

```bash
cd rust
bash tools/werkzeuge-installieren.sh                     # dry run: prints what it would do
bash tools/werkzeuge-installieren.sh --install --target "$HOME/.local/share/wb-rust-werkzeuge"
./gates.sh tools
```

It downloads the rustup installer from `static.rust-lang.org`, compares its SHA-256 with the
checksum pinned for the platform (`aarch64-apple-darwin`, `x86_64-unknown-linux-gnu`), runs it
with `--no-modify-path`, builds the three tools with `cargo install --locked`, and checks that
nothing outside the folder changed. It needs no sudo. On other platforms, run the cargo commands
below with the toolchain of `rust/rust-toolchain.toml`. Details: `rust/README.md`.

| Gate | Command |
|---|---|
| `./gates.sh fmt` | `cargo fmt --all --check` |
| `./gates.sh clippy` | `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` |
| `./gates.sh test` | `cargo test --workspace --all-targets --all-features --locked --no-fail-fast` |
| `./gates.sh licenses` | `cargo deny --locked --all-features check licenses bans sources` and one CycloneDX SBOM per crate |
| `./gates.sh notices` | every piece of code taken from another project carries its notice and ships with it |
| `./gates.sh mutants` | `cargo mutants` one crate at a time: no mutant survives in the security crates (`MUTANTS_PFLICHT`); the other crates report theirs |

The continuous integration runs the first five gates through the script on Linux and macOS, and
fmt, clippy, test and cargo-deny with the same commands on Windows. All of them are required. The
mutation gate runs outside the CI; [CONTRIBUTING.md](CONTRIBUTING.md) says how and when.

## Contributing

Pull requests go against this branch. The maintainer takes a change over into the source tree as
a patch and names you in the commit; the pull request is then closed with a note. A pull request
needs the Contributor License Agreement (CLA) to be accepted once. Details:
[CONTRIBUTING.md](CONTRIBUTING.md). Questions go to Discussions, see [SUPPORT.md](SUPPORT.md).

Small, well-described tasks for a first contribution are open issues with the label
[`good first issue`](https://github.com/Skryx-L-A/agent-workbench/issues?q=is%3Aissue+is%3Aopen+label%3A%22good+first+issue%22).

## License

The licensor's own work is licensed under the [Apache License 2.0](LICENSE); third-party parts and
the attribution are listed in [NOTICE.md](NOTICE.md). The names and logos are not licensed; see
[TRADEMARKS.md](TRADEMARKS.md). Setup, support and training are offered as a paid service; see
[SUPPORT.md](SUPPORT.md).

Earlier states keep the license they were published under; see [CHANGELOG.md](CHANGELOG.md).
