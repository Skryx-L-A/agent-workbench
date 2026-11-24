# Rust workspace

This folder holds the Cargo workspace of the Rust rebuild: 21 crates under `crates/`, one per building
block, with their cases and check scripts under `spec/`. The crate `crates/placeholder` is left over from
the first scaffold; it has one function and two tests and is part of no program.

## Layout

- `Cargo.toml`: workspace manifest, shared package fields and shared lints (`[workspace.lints]`).
- `Cargo.lock`: committed; every gate runs with `--locked`.
- `rust-toolchain.toml`: fixed toolchain (1.99.0, profile minimal, with rustfmt and clippy).
- `rustfmt.toml`: formatting settings.
- `deny.toml`: licence, ban and source rules for cargo-deny.
- `gates.sh`: the build gates.
- `crates/*`: workspace members. A new crate opts into the shared lints with `[lints] workspace = true`.

## Gates

All gates run through one script. Each gate has its own exit code.

| Command | Checks | Red when |
| --- | --- | --- |
| `./gates.sh fmt` | `cargo fmt --all --check` | a file is not formatted |
| `./gates.sh clippy` | `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` | any lint warns |
| `./gates.sh test` | `cargo test --workspace --all-targets --all-features --locked --no-fail-fast` | a test fails |
| `./gates.sh licenses` | `cargo deny check licenses bans sources`, then one CycloneDX SBOM per crate in `sbom/` | a licence is not allowed, a source is not crates.io, a crate is duplicated, or an SBOM is missing |
| `./gates.sh notices` | every row of "Code taken from other projects" below: the notice file or file head exists, names the licence and the source file, and both are in `cargo package --list`; a file under `crates/*/src` with a foreign licence head must have a row; a crate with a bin target and a row builds its bins and runs each with `--licenses`: exit 0, and each notice file of its rows is in the output byte for byte | a notice is missing, names another licence, is not packaged, or foreign code has no row; a program exits non-zero on `--licenses` or its output lacks a notice |
| `./gates.sh mutants` | `cargo mutants --workspace` (one exclusion in `.cargo/mutants.toml`, see `rangkern serve` below) | a mutant survives, times out, the baseline fails, or no mutant was caught |
| `./gates.sh all` | all six in this order | any gate is red (exit 1) |

Exit codes: 0 green; for a single gate the exit code of the failing tool; 64 usage error; 127 a required
tool is missing. Build output stays below this folder (`target/`, `mutants.out/`, `sbom/`, all ignored by git).

`cargo test` runs with `--no-fail-fast`. Without it, a flaky test in one crate hides real failures in
the crates after it.

The limits of the notices gate: the reverse direction searches only `crates/*/src` and only the first 40
lines of a file (`NOTICE_HEAD_LINES`). The output check runs only for crates with a bin target, and only
for the notice files named in backticks in a row (a "head of the file" notice is not a file to compare).
The output check needs `python3` (the system python3 on the measuring Mac) to read the cargo metadata and
to compare the bytes.

## Licence list

The allowed licences stand in `deny.toml` between `BEGIN LICENCE LIST` and `END LICENCE LIST`.
The current list is a placeholder (MIT, Apache-2.0). The final list replaces only these lines; no gate
changes. Workspace crates carry `publish = false` and are not checked as dependencies.

The licence gate does not check the advisory database (`cargo deny check advisories`), because that
check fetches the database from the network on every run. Whether it becomes a gate is open.

## Dependencies

Runtime dependencies, as `Cargo.toml` of each crate lists them (state of 19.11.2026):

| Crate | Dependencies |
| --- | --- |
| `rangkern` | `notiz-kern` (workspace), `serde`, `serde_json`, `regex`, `rusqlite` (with the feature `bundled`, which compiles SQLite into the program) |
| `notiz-kern` | `caseless`, `serde_json`, `unicode-normalization` |
| `zustandskern` | `serde_json` (default features) |
| `formate` | `serde_json` (with the feature `float_roundtrip`: a decimal number is read to the same double as JavaScript, which RFC 8785 needs for the bytes of a line; no new crate). SHA-256 is an own implementation (FIPS 180-4 vectors in the tests), the date needs no crate |
| `traeger` | `alacritty_terminal` (pinned `=0.26.0`), `portable-pty` (pinned `=0.9.0`), `serde_json` |
| `rasterauszug` | `serde_json` (default features), `flate2` (no default features, backend `miniz_oxide` only), `miniz_oxide` |
| `modellweg` | `formate` (workspace; the line `modellweg.aufruf` through its general append), `serde_json` (feature `float_roundtrip`), `serde` (no default features; the visitor that refuses duplicate keys), `rustix` (features `fs`, `process`; prctl, getuid, umask and `O_NOFOLLOW` without `unsafe`), `signal-hook` (SIGTERM and SIGINT), `native-tls` (TLS with the truststore of the system: OpenSSL on Linux, Security.framework on macOS; the licences of its graph are all MIT or Apache-2.0, measured with cargo-deny on 20.11.2026). `openssl` (Linux only: the TLS server and the leaf certificates of mode 2 `kopf-einsetzen`; measured the same day, no licence outside the list) |
| `dienst` | `traeger`, `zustandskern`, `rasterauszug`, `start`, `einstellungen` (workspace), `serde_json`, `rustix` (pinned `=1.1.6`, features `process`, `event`), `signal-hook` (pinned `=0.4.6`) |
| `stelle` | `formate` (workspace: canonical form, `sha256_hex` and the time text of the chain; HMAC-SHA256 of the checker `h1` is built on `sha256_hex`, no new crate), `serde_json` (default features) |
| `bruecke` | `serde_json` (default features), `signal-hook` (default features) and `rustix` (feature `process`). The last two are already in `Cargo.lock` and in the build through `alacritty_terminal`, `polling` and `portable-pty`; no new package enters the graph. `std` has no signal handler and no effective user id, and both need `unsafe` through `libc` |
| `ansicht` | `dioxus` 0.7.10 (no default features; `macro`, `html`, `signals`, `hooks`), `dioxus-web` 0.7.10 (only for `wasm32`, no default features, `mounted`), `wasm-bindgen`, `wasm-bindgen-futures`, `js-sys`, `web-sys` (the versions `dioxus-web` brings), `rasterauszug` (workspace: the decoder of the raster frames), `serde_json`. 105 new packages in `Cargo.lock`, 79 of them in the wasm build; licences outside MIT and Apache-2.0 only with the named exceptions in `deny.toml` (Zlib: `const_format`, `const_format_proc_macros`, `konst`, `konst_macro_rules`, `slotmap`; ISC: `libloading`; decision of 21.11.2026) |
| `kernabbild` | `rustix` (pinned `=1.1.6`, no default features, features `std` and `process`: `set_dumpable_behavior`, `getrlimit`, `setrlimit`); the package is in `Cargo.lock` already, no new package enters the graph |
| `placeholder` | none |

The reason for each dependency of `rangkern` stands in "Code taken from other projects" below. The HTTP call of
`--embed` needs no crate (it uses `std::net`). Every new dependency needs a reason in the commit that adds it,
and the licence gate rejects anything outside the list.

Tools used by the gates (installed by `tools/werkzeuge-installieren.sh`, see below):

| Tool | Version | Licence | Purpose | Rejected alternative |
| --- | --- | --- | --- | --- |
| rustup toolchain | 1.99.0 | MIT OR Apache-2.0 | compiler, cargo, rustfmt, clippy | 1.100.0: released 2026-11-09, one week old at the time of pinning |
| cargo-deny | 0.21.0 | MIT OR Apache-2.0 | licences, bans and sources of the dependency graph | cargo-license: lists licences but enforces no policy and checks no sources |
| cargo-cyclonedx | 0.5.10 | Apache-2.0 | CycloneDX SBOM per crate | cargo-sbom 0.10.0 (MIT): smaller project; cargo-cyclonedx belongs to the CycloneDX standard project |
| cargo-mutants | 27.1.0 | MIT | mutation run | mutagen: needs nightly Rust and attribute macros in the source code |

The gates use these tools only from one folder per machine, `~/.local/share/wb-rust-werkzeuge` (`rustup/` is
`RUSTUP_HOME`, `cargo/` is `CARGO_HOME`, `tools/` is the `--root` of the three `cargo install` calls).
`gates.sh` sets `PATH`, `RUSTUP_HOME` and `CARGO_HOME` for its own run only; no shell profile changes, and the
Homebrew Rust or a rustup of the user stays untouched and is never used. If the folder, the toolchain or a
tool of another version is missing, a gate stops with exit 127 and says which. `./gates.sh tools` makes only
this check. A test seam moves the folder (`WB_RUST_WERKZEUGE_ORT`) and works only inside the test environment
(`WB_TESTUMGEBUNG=1`).

`tools/werkzeuge-installieren.sh` makes the folder. Without arguments it is a dry run: it prints what it would do
and writes nothing. `--install --target DIR` installs (DIR absolute; the real one is
`~/.local/share/wb-rust-werkzeuge`). It downloads the rustup installer (rustup 1.29.3, from
`static.rust-lang.org`, never piped into a shell), compares its SHA-256 with the pinned one for the platform
(`aarch64-apple-darwin`, `x86_64-unknown-linux-gnu`) and stops before starting it on a mismatch; it runs the
installer with `--no-modify-path`, builds the three tools with `cargo install --locked` in the pinned version,
checks the result with `gates.sh tools`, and checks that nothing outside the folder changed. It needs no sudo
and no other host than `static.rust-lang.org` and crates.io; a second run does nothing. To change a version,
change it in `gates.sh`, in the script and in the table above together; a test of the workbench tree
compares them.

## Code taken from other projects

The bill of materials of the building blocks lists, besides the crates of the dependency graph
(`./gates.sh licenses` writes one CycloneDX file per crate), source that was copied or ported into
the tree. A licence that asks for its notice to stay with the source is listed here and in the crate.

| Where | What | Origin | Licence | Notice |
| --- | --- | --- | --- | --- |
| `crates/rangkern/src/stem.rs` | Snowball German stemmer, ported by hand from the generated `german_stemmer.py` | `snowballstemmer` 3.1.1 (Snowball project) | BSD-3-Clause | head of the file and `crates/rangkern/NOTICE` |
| `crates/rangkern/src/tables.rs` | character classes of Python `str` (generated by `tools/gen_tables.py`) | the Unicode Character Database as shipped with Python 3.15.2 (Unicode 17.0.0) | Unicode-3.0 (tables derived from the Data Files) | `crates/rangkern/NOTICE` |
| `crates/rangkern/src/vdb.rs` | SQLite, compiled into the program to read the databases of the model | SQLite 3.54.1, `sqlite3.c` of the crate `libsqlite3-sys` 0.40.0 (feature `bundled` of `rusqlite` 0.43.0) | Public domain | `crates/rangkern/NOTICE` |

Dependencies of `rangkern` (a reason for each): `notiz-kern` (path; the corpus and exclusion rules, one
source), `regex` (the full-text fallback uses the engine `rg` uses, no process is started), `serde` and
`serde_json` (the vector file is read with a visitor that keeps the key order of the notes; with the feature
`float_roundtrip`, so that a decimal number is read to the same double as Python's `float()`, bit for bit), `rusqlite` 0.43.0
(MIT; decision of 18.11.2026; the note vectors are read straight from `gardener.db` and `extra-embeddings.db`).
`rusqlite` is used with `default-features = false` and the feature `bundled`: it compiles SQLite 3.54.1 (public
domain, from `libsqlite3-sys` 0.40.0, MIT) into the program, so that Windows needs no system SQLite. Without
default features the statement cache (`hashlink`, `hashbrown`, `foldhash` under Zlib) is not in the graph; the
program runs one statement per database and needs no cache. The licences of both crates are on the allowed list
and need no exception in `deny.toml`; the row above carries the public-domain notice of SQLite.

## `dienst` (core service and check program)

`crates/dienst` is the service of the new session view and the program `wb-dienst`. **`wb-dienst` is the core service of the trial run**: the
package ships it as `/usr/lib/werkbank/werkbank-kern`, it runs as a service of the user (`systemd --user`, `RuntimeDirectory=werkbank` with mode 0700,
`NoNewPrivileges=yes`, start file `/etc/werkbank/kern.json` from `postinst`); it makes no check of the peer, the boundary is the folder with mode 0700.
The same program is the check program of the contract cases (`--kind`). One session per service, or with the key `lauf` in the start file
several (at most 8 at the same time, DI12): per session a child in a PTY (`traeger`) and a state core (`zustandskern`), per viewer
one extract (`rasterauszug`), plus a room core without a session, a clock and one Unix socket. `Starten` and `Beenden` are events of
the service itself. It only translates between the blocks and the socket; the structure of a start is checked by the crate `start` and the settings file is
read by the crate `einstellungen` (both of the workspace, no new package). Contract: `rust/spec/VERTRAEGE.md`
(rules DI1 to DI12); details in `crates/dienst/README.md`.
The optional key `lauf.sitzungen` (an absolute path) makes `Starten` skip every number whose entry `<lauf.sitzungen>/s-<n>` still exists, so a restarted service
does not hand out `s-1` again while its folder is left (DI12 point 13); the service creates and removes nothing there.
The optional keys `lauf.ziellisten` and `lauf.laufzeit` (absolute paths) make `Starten` skip a number also while the target list `<ziellisten>/s-<n>.zielliste.json`, the socket folder `<laufzeit>/s-<n>` or a count file `<marker_ordner>/s-<n>.suche.json` or `.modellweg.json` of a cloud session still lies (DI12 point 15). An own regular target list that alone takes a number of an earlier run is removed by `Starten` through the descriptor of the folder before the number is handed out (DI12 point 15a).
The optional keys `lauf.marker_datei` (a file that only a trial run makes, read once at the start of the service) and `lauf.marker_ordner` (the folder of the count, from the start file)
make the service write `marker` into every `start.json` of a trial run (DI12 point 14); the file holds only the list, a request of the view can never set `marker`.

| Dependency | Version | Purpose | Security point |
| --- | --- | --- | --- |
| `traeger`, `zustandskern`, `rasterauszug` | workspace | child and screen, rules of the view, extract and frames | none of its own |
| `serde_json` | 1 | start file, events and messages as JSON values | the start file is read with the depth limit of 128 and a size limit of 1 048 576 bytes |
| `rustix` | `=1.1.6`, features `process`, `event` | `geteuid` (DI2 point 2: the owner of the socket folder must be the effective user), `kill_process` (SIGHUP and SIGKILL to the child), `kill_process_group` (SIGTERM to the process groups named in the pid files at the end of a session, DI12 point 12), `poll` (the loop waits for the listener, the connections and the wake socket and reads the ready connections in a fixed order) | DI2: `src/rechte.rs`; the loop: `src/dienst.rs`; the end of a session: `src/aufraeumen.rs` |
| `signal-hook` | `=0.4.6` | SIGTERM and SIGINT end the service in an orderly way (DI2 point 7) | none; safe API only |

`std` has no call for the effective user id and no signal handler, and `unsafe` is forbidden in this workspace; that is why both
crates are direct dependencies. Both packages were in `Cargo.lock` before (through `portable-pty` and `alacritty_terminal`); the
only new entry of the lock file is the crate itself. The security points: DI2 (folder 0700 by `lstat`, owner the effective user,
socket path must not exist, socket 0600, no network socket) is one small file, `src/rechte.rs`; the environment of the child is
exactly the one of the start file (`src/sitzung.rs` `kind_starten` and the cover `src/huelle.rs`); the log line of the hard bound
of the queue holds no content of the session. DI12 adds: the check of the structure of a start in one place (`src/starten.rs`
`start_form`), the folder `s-<n>` (0700) with `start.json` (0600) per started session, the paths `lauf.programm` and
`lauf.einstellungen` only from the start file, and `Beenden` and the end of the service leave no child (SIGHUP, 1 s, SIGKILL).

The bill of materials is `sbom/dienst.cdx.json` (written by `./gates.sh licenses`); it lists the crate and its dependency graph
(`traeger`, `zustandskern`, `rasterauszug`, `rustix`, `signal-hook` and what they bring).

## `rangkern serve` (warm process)

`rangkern serve <vault> [--vectors <file> | --index-db <db> [--extra-db <db>]]` keeps the notes, the lexical index and the note vectors in
memory and answers one JSON line per request on standard input (`search`, `ping`, `quit`). Every
answer is the one `rangkern search` gives at that moment on the same files; the contract lists the
protocol, the invalid lines and the exit codes. The code is in `crates/rangkern/src/serve.rs` (protocol),
`warm.rs` (state and refresh), `stamp.rs` (change detection) and `bm25.rs` (`LexKorpus::rebuild`).

- **Finding changes.** Before every search the process lists the corpus with the rules of `notiz-kern`
  and stamps every file. On Unix a stamp is device, inode, size, `mtime` and `ctime`. `ctime` moves with
  every write, rename, `chmod` and `utime`, so a rewrite of the same size with the old `mtime` still shows;
  size and `mtime` alone would miss it. A stamp that is younger than two seconds is *racy* (two writes can
  share one tick of a coarse clock): the next look compares the content. Windows has no safe way to read a
  change time (`unsafe` is forbidden in this workspace), so there every stamp is racy and every note is
  read again on each search; nothing stays open between two requests, because a read closes the file.
  There is no file watcher and no waiting.
- **Partial rebuild.** Only a note whose stamp differs, or is in doubt, is read; only a note whose text
  differs is analysed again. The statistics of the index (idf, average length, the average idf that
  replaces negative values) are then computed again from the stored analysis of all notes with the same
  code that builds an index from scratch, in the same order, so a rebuilt index gives the same bits.
  Terms of notes that are gone stay in the vocabulary without a document; they change no number.
- **Not cached.** `.brain/spaces.lock`, `_meta/state/contradictions.json`, `.space.yml` and the
  full-text fallback read their files on every search that needs them (small files, or the fallback
  only); the note vectors are read again when the stamp of the vector file differs.
- **Exception in the mutation gate.** `.cargo/mutants.toml` excludes `portable_key`
  (`crates/rangkern/src/stamp.rs`), the stamp of a platform without `ctime`. That code is only built under
  `cfg(not(unix))`, so on the Linux gate machine no test can fail for a mutant of it; `cargo-mutants` does
  not evaluate `cfg`. Decision of 18.11.2026: the exception stands only together with the Windows
  check of `rangkern` in the Windows test VM (requirement A61). That check must cover `portable_key`
  explicitly: the visible `serve` set runs under Windows, including the cases that change a note with the
  same size and an `mtime` set back, and their result is recorded. Without that check the exception is open.
- **Timing.** `crates/rangkern/tools/serve_messung.py` builds an invented vault (`--notes`, `--words`,
  `--dim`, `--long`) and measures the program at the caller: start to `ping`, 200 searches with a question
  vector, the first search after a note changed, memory. The numbers are rough (one run, one machine).

Tests: `tests/differenz.rs` (random changes, server against a fresh `search`, bit equal), `tests/serve.rs`
(the program on pipes, every change of the contract list), unit tests in `src/tests/{serve,warm,stamp}.rs`.

## Note vectors from the databases (`--index-db`)

`rangkern search <vault> <question> --index-db <gardener.db> [--extra-db <extra-embeddings.db>] [--query-vector <file>]`
and `rangkern serve <vault> --index-db <gardener.db> [--extra-db <extra-embeddings.db>]` read the note vectors
themselves instead of taking a vector file. The answer is the one of `search --vectors F`, where `F` holds what
`load_merged_embeddings` of the model reads from the same databases. The code is `crates/rangkern/src/vdb.rs`;
`cli.rs` (`db_paths`, `search_vectors`) holds the option rules.

- **Options.** `--index-db` and `--vectors` exclude each other; `--extra-db` and `--query-vector` need
  `--index-db`; `serve` has no `--query-vector` (the request carries `query_vector`). Any other combination is
  exit 2. Without `--query-vector` (or without `query_vector` in a request) the databases are not read and the
  lexical way is taken, as without `--vectors`.
- **Merge.** `SELECT rel, vector FROM embeddings` on each database, in the order SQLite gives the rows; the
  first database, then the second: a path that is there takes the new vector and keeps its place, a new path
  goes last. A missing second database is empty (`Path::exists`, as the model). A `vector` is parsed like the
  vector file (a JSON list of numbers; `NaN` and `Infinity` are unreadable, a named deviation).
- **Unreadable (exit 3, `{"error": "unreadable"}`; in `serve` the answer to a search with `query_vector`).**
  The first database is missing, a file is no SQLite database, the table `embeddings` or a column is missing,
  a `rel` is not text, a `vector` is neither text nor blob or no JSON list of numbers, the query vector file is
  missing, no list of numbers, or empty.
- **Read only, no side effect.** `Connection::open_with_flags(path, SQLITE_OPEN_READ_ONLY)`: the path is taken
  as written (no URI flag, so `?`, `#` and `%` in a path need no escaping; the effect is that of the model's
  `file:<path>?mode=ro`). The process creates no journal, no `-wal`, no `-shm` and no folder; the tests compare
  the folder before and after. One statement copies the rows out (text only, no parsing), the connection is
  closed, and only then the vectors are parsed, so a writer waits no longer than the copy. The busy timeout is
  5 s, the default of Python's `sqlite3`. `serve` keeps no connection and no file between two requests; a
  second process can write right after every answer (tests: `src/tests/vdb.rs`, `tests/vdb.rs`).
- **State in `serve`.** Per database one stamp (as for the vector file, see `stamp.rs`); the second database is
  `None` while it is not there. A search with `query_vector` reads the databases again when a stamp differs,
  a file appeared or vanished, or a stamp is in doubt (younger than two seconds, and always on Windows) and the
  bytes of the files differ from the ones kept. Bytes are kept only while a stamp is in doubt.
- **WAL databases are unreadable (contract addendum of 18.11.2026).** A database in WAL mode gets the files
  `-wal` and `-shm` on a read-only open, and they stay (measured on the Linux build host 18.11.2026). The program creates no file,
  so `is_wal` reads bytes 18 and 19 of the header before opening: a 2 in either makes the database unreadable and
  nothing is opened. The two databases of the model use the rollback journal (`PRAGMA journal_mode=WAL` stands
  only in `gardener/lauf/buch.py`, another database).
- **Tests.** Unit tests in `src/tests/vdb.rs`; `tests/vdb.rs`: `search --index-db` against `search --vectors F`
  in 600 invented states (overlap, order, missing second database, relics, excluded folders, damaged files,
  repeated paths) equal to the byte, a server against a fresh read after random changes of the databases on
  three clocks, and the program on pipes with a second process that writes after every answer. The visible set of
  the contract (`tests/faelle-vdb/{search,serve}`, `ziele.json`) is a copy from the vault; the runners
  `test_aussen_vdb.py` and `test_serve_vdb.py` stay in the vault and run through `vdb-sichtbar-<linux-host>.sh`.

## The question embedded in the process (`--embed`)

`rangkern search <vault> <question> --index-db <db> --embed <url>` and `rangkern serve <vault> --index-db <db>
--embed <url>` fetch the vector of the question themselves from the local Ollama, as
`gardener.ollama.OllamaClient.embed` does, instead of taking it from `--query-vector` or `query_vector`. The code is
`crates/rangkern/src/embed.rs` (the call), `search.rs` (`rank_embedding`, `search_embedding`) and `cli.rs`
(`embedder_of`, the option rules).

- **Options.** `--embed <url>` needs `--index-db` and excludes `--query-vector` (and `--vectors`). The other three
  need `--embed`: `--embed-model` (default `embeddinggemma:latest`), `--keep-alive` (default `30m`),
  `--embed-timeout-ms` (default 180000; a whole number of 1 or more). Any other combination is exit 2, as is a
  `serve` that gets one of them twice. In `serve` a request with `query_vector` takes that vector; a request
  without it is embedded.
- **Address.** Only `http://127.0.0.1:<port>`, `http://localhost:<port>` and `http://[::1]:<port>` (port 1 to
  65535, no path, not even `/`, no user, no other spelling). Everything else is exit 2. `localhost` is the two
  fixed addresses `127.0.0.1` then `::1`; the program never asks a resolver, never reads an environment variable
  (no proxy variable counts) and starts no process.
- **The call.** `POST /api/embed` with the body `{"model": M, "input": T, "keep_alive": K}`, written the way
  Python's `json.dumps` writes it (`ensure_ascii`), `T` cut to 8,000 code points. HTTP/1.1 over `TcpStream`
  with `Connection: close`; the answer may carry `Content-Length` or `Transfer-Encoding: chunked` (an answer with
  neither is read to the end of the stream). One deadline covers one try from the connect to the last byte, so a
  slow trickle of bytes does not extend it. A failure is: no connection, deadline over, a status other than 200
  (a redirect is a failure, not followed), a reply that is no valid HTTP/1.x, no valid JSON, no `embeddings`, or a
  first entry that is empty or no list of finite numbers. After a failure exactly one more try follows; if it
  fails too, the search takes the lexical way (`"fallback": true`), as the model does on `OllamaError`. The
  connection is closed after every answer, so `serve` holds none between two requests.
- **No call without a note vector of the corpus.** Ollama is asked only if at least one note vector belongs to a
  note of the search corpus. Vectors of excluded or vanished notes count for nothing (the finding rule), so
  databases that hold only those give the hybrid way with no semantic list, and no call.
- **Numbers.** The numbers of the answer are read by the standard library, which rounds them as Python's `float()`
  does. The note vectors and the query vector file go through `serde_json` with the feature `float_roundtrip`, which
  reads a decimal number to the same double as Python's `float()`, bit for bit. Without the feature `serde_json`
  read 3.93 % of the numbers of the real databases one ULP differently (measured 19.11.2026).

The visible set of the contract (`tests/faelle-emb/{search,serve}`, `ziele.json`) is a copy from the vault; the
runners `test_aussen_emb.py` and `test_serve_emb.py` and the stand-in `ollama_attrappe.py` stay in the vault and run
through `emb-sichtbar-<linux-host>.sh`.

Named readings: the deadline of `--embed-timeout-ms` covers one whole try (connect to last byte), where the model's
`urllib` limits each socket operation; the two differ only for a server that sends its answer in a slow trickle.
The connect to each address has a deadline of its own, 250 ms (`CONNECT_MS`), and never more than what is left of the
deadline of the try; there is no option for it. `localhost` tries `127.0.0.1` and then `[::1]`, each with the 250 ms.
A connect that does not finish in time is a failure of the try, and the usual rule follows (one more try, then the
lexical way). Reason: Windows refuses a connect to a closed loopback port only after 2.0 s, so a search with Ollama
off cost 4.05 s there (two tries); the Mac refuses in 4 ms. The kernel completes the connect to a listening loopback
port at once, so the 250 ms do not cut a slow answer. The model has no such limit; the output differs only if a connect
takes longer than 250 ms and would have worked (contract `rangkern-einbettung`, named deviation 5).
Status other than 200 is a failure (the model accepts every 2xx). A first entry of `embeddings` that is empty is a failure.

Tests: `tests/embed.rs` (a stand-in for Ollama on a free loopback port: the request to the byte, every kind of
failure, the retry, the deadline, the connect deadline (a full listener queue as a connect that gets no answer),
`chunked`, a long question, no connection left open; `search --embed` against `--query-vector` in 320 states; `serve`; the option rules; the program on pipes with proxy variables set),
`src/tests/embed.rs` (address, body, first vector, the limit of the connect).

## `zustandskern` (state core of the session view)

`crates/zustandskern` is the state core of the session view: events go in, effects come out. It holds the
rules R1 to R15 of the contract `rust/spec/VERTRAEGE.md` (for example R1 the bytes written to the PTY for text, keys and pasted text, R12
the settings with their levels company, group and user, R14 the backlog limit per viewer, R15 the cursor). The reference
implementation and the hidden cases are kept outside the repository; the visible cases below were made from them. The
library (`src/lib.rs`) reads no file, starts no process, opens no socket and asks for neither the time nor a random
number: the only clock is the field `t` of each event. Only `src/main.rs` touches the outside world.

- **The program `zk`** (`src/main.rs`): `zk [case.json | -]` reads one case from a file, from `-` or, without an
  argument, from standard input, and writes the effects as one JSON line to standard output. A case is a JSON
  object with `ereignisse` (a list of events, each an object with a whole number `t`) and an optional `anfang`
  (the start state, an object). The line holds the keys `nachrichten`, `pty`, `groessen`, `ablehnungen`,
  `protokoll`, `an_stelle`, `abfragen` and `zustand`. The program is a pure function of its input.
- **The cursor (R15).** The core keeps one cursor per session, `[row, column, visible]`, and passes it on. The `Ausgabe`
  of the carrier may carry `cursor`; it is checked before the stand: row below the number of rows and column below the
  number of columns of the size of the session (not the size a viewer reported), whole numbers, a truth value for
  `visible`, else `ereignis_ungueltig` and the whole event is dropped, also when its stand is old. `anfang.sitzungen[].cursor`
  gives a core the cursor it had before (a service builds a new core after an error with the cursor of `zustand`); a
  wrong form there, `null` included, or a place outside the size is an invalid case (exit 2). `Abzug`, `Rasteraenderung`
  and `zustand.sitzungen[]` carry `cursor` in every message as soon as the core knows one; before that the field is
  absent, so every case without a cursor gives the output it gave before the cursor. A pure cursor step (an `Ausgabe`
  without rows) goes through the same distance as a line (R5): at most one `Rasteraenderung` per distance and session,
  `bildrate_max` included, and the latest cursor of a bundle counts. A cursor waits like the lines inside a synchronised
  update (DEC 2026) and is clamped against the size that holds at the release. A size change clamps the cursor to the
  last row and column before the full picture goes out; the smallest size is 2 columns by 1 row, the largest 1000 by 1000.
  The catch-up of a viewer (R14) carries the present cursor as well, also when only the cursor was missed. All of it
  holds for the program `zk` and for the step API alike: both run the same core.
- **Sizes.** Columns 2 to 1000 and rows 1 to 1000 in `anfang.sitzungen[].groesse` (outside: invalid case, exit 2; the size is
  checked there since 20.11.2026), in `Abo` and in `Groesse` (outside: rejected with `groesse_ungueltig`, like 0 or a size over
  1000). One column is refused because `alacritty_terminal` 0.26.0 has two faults that occur only at one column: a character
  of width 2 panics, and `Term::resize` never ends (alacritty/alacritty issue 9112; the library names `term::MIN_COLUMNS = 2`
  but does not check it). So no part of the session takes one column. `GroessenFolge` is never executed, whatever its
  numbers (`groesse_nur_kern`). This holds for the program `zk` and for the step API `Sitzungskern`.
- **The step API** (`src/schritt.rs`): a service leads one core through a session event by event. `Sitzungskern::neu`
  builds the core from an `anfang`; `schritt` applies one event and returns only its `Wirkungen` (the keys of the
  output of `zk` except `zustand`; `als_wert` shows them as that JSON, `anhaengen` joins the effects of two steps);
  `naechste_frist` gives the smallest `t` at which the core does something without a new event (a dispatch after
  the distance of R5, a catch-up of R14, the end of a synchronised update or of a request), so the service sends a
  `Takt` with exactly that `t`; `zustand` gives the end state. `laufen` is built on it and its output is
  byte-equal to the output before the step API. The limit of `NACHRICHTEN_MAX` counts per step here; a step over it
  is an error and the core is spent. The clock stays outside: the only time is the `t` of the events.
- **Environment, working folder and writing (TR15, 20.11.2026).** The child sees exactly the variables of `Start::env`.
  `portable-pty` 0.9.0 always adds `SHELL` (`cmdbuilder.rs`, `as_command`: `cmd.env("SHELL", shell)`) and has no call that
  leaves it out, so without `SHELL` in `env` the child starts as `/usr/bin/env -u SHELL <program> <arguments>`; `env`
  replaces itself by the program (same process id, session and PTY). The program is searched in `src/programm.rs` the way
  `portable-pty` searches it (absolute name, `./` and `../` from the working folder, else `PATH` of `env`), so that a missing
  program is still an error of `Kind::starten`; `argv[0]` of the child is then its absolute path. A `cwd` that is no folder
  is `Fehler::Start` (`portable-pty` would start the child in the home folder instead). `Kind::schreibe` puts the bytes in a
  queue (`src/schlange.rs`) that a writing thread empties in pieces of at most 4096 bytes; it returns at once. The queue
  holds at most `SCHLANGE_MAX` = 262,144 bytes (256 KiB, the bytes in flight counted): 64 times the PTY buffer of Linux (about
  4 KiB), far above a large paste, and at most 256 KiB bound by a stuck child against the 11.4 MiB of one session (P7). A write
  that does not fit is refused whole with `WouldBlock` (the program `tr`: `"fehler": "schlange_voll"`); after a failed write
  to the PTY the queue answers `BrokenPipe`. The writing thread is not joined at the drop (a grandchild that keeps the PTY
  open could hold it in `write`); the drop closes the queue and ends the child.
- **Limits.** At most 4,194,304 bytes of input (`EINGABE_MAX`; the program reads one byte more than that and
  stops, so a longer input is not read to the end). JSON nested deeper than 128 levels (the default of
  `serde_json`). `t` must be a whole number, the first at least 0 and none below the one before. The serialised
  messages to the viewers may hold at most 33,554,432 bytes (`NACHRICHTEN_MAX`): a small case can make a message
  to many viewers per event, so the output is not bounded by the input, and a case beyond the limit is invalid.
  A rejected event is not an error: it goes into `ablehnungen` and the run goes on.
- **Exit codes.** 0: the effects are on standard output. 2: the case is invalid (not JSON, not an object, over
  the input limit, nested too deep, `ereignisse` missing or not a list, `t` missing, not whole or falling, a wrong
  type in `anfang`, messages over the limit, an unreadable file or folder, more than one argument); standard output
  stays empty and one line goes to standard error. 1: the effects could not be written to standard output.
- **Cases.** `tests/faelle/` holds 529 case files and `tests/erwartet/` the expected result of each, with the same
  name (`{"exit": 0, "ausgabe": ...}` or `{"exit": 2}`). `r*` and `x_*` (86) are the visible outside cases of the
  contract, 11 of them `r15_*` for the cursor; `g_2000` to `g_2199` (200) come from a random generator, `c_3000` to
  `c_3149` (150) are random cases of the same kind with cursors added by a second generator (so the older cases did not
  change), `h_*` (93) are written by hand, among them 18 `h_r14_*` for the backlog limit and 10 `h_r15_*` for the cursor.
  The expected output comes from the reference. All data are invented. The
  test `tests/faelle.rs` runs every case through the library and compares the whole output.
- **Other tests.** `tests/grenzen.rs` starts the built `zk` (the only test that does): the input limit to the byte,
  deep nesting, invalid UTF-8, truncated or empty input, bad `t`, a wrong start, a case file as argument, a missing
  file, too many arguments, a folder; each invalid case must give exit 2, empty standard output and one line on
  standard error. `tests/nebenwirkung.rs` reads the sources (`include_str!`) and fails on any forbidden pattern
  (`std::fs`, `std::io`, `std::process`, `std::net`, `std::env`, `std::thread`, `std::time` and others in the
  library; `Command`, `env::var`, `File::create` and others in `main.rs`) and on a source file missing from its
  list. `tests/schritt.rs` runs every case event by event through the step API and compares with `laufen`, and checks
  `naechste_frist` on cases of its own and by a `Takt` at every deadline of every case; it also checks the cursor
  through the step API (a storm of pure cursor steps through the distance, the cursor of the end state as the start of a
  new core, an invalid cursor at the start and in an event, the clamp at a size change, the wait in a synchronised
  update). Unit tests are in
  `src/tests/{base64,eingabe,lauf,regel}.rs`.
- **The generator** `tests/werkzeug/erzeuge_faelle.py` writes `tests/faelle/` and `tests/erwartet/`. It is not part
  of any gate. It needs Python 3.15 (Python 3.9 reads Base64 differently; run it as
  `python3 -I -B erzeuge_faelle.py <contract package> <tests folder>`) and the contract package: the folder with
  `referenz/zk_ref.py` (the reference) and `aussentests/{faelle,erwartet}/` (the outside cases), which is not in
  the repository. It uses no clock and no randomness besides `random.Random(seed)` with the case number as the
  seed, so the same call writes the same files. The standard library is all it imports.
- **Dependencies.** One: `serde_json` 1.0.154, with default features. It brings `itoa` 1.0.19, `memchr` 2.8.3,
  `serde_core` 1.0.229 and `zmij` 1.0.23 (`cargo tree -p zustandskern --locked -e all`; versions from
  `Cargo.lock`). `serde_json` reads the case and builds the effects as JSON values. No derive is used, so no
  proc-macro crate is in the graph of the program. `rangkern` already depends on `serde_json`, so the crate adds
  no package to the workspace graph; no source of the crate has a row in "Code taken from other projects".

## `traeger` (carrier of the session view)

`crates/traeger` is the carrier: one PTY with one child and one screen grid per session. It turns the bytes of the child
into the events of the state core (`Ausgabe`, `Modus`, `SyncBeginn`, `SyncEnde`, `Zwischenablage`, `Titel`,
`GroessenFolge`, `Kindende`) and the effects of the core (`pty` bytes and `groessen`) into bytes and a size for the
child. It holds the rules TR1 to TR15 of the contract. Every `Ausgabe` carries the cursor, `[row, column, visible]` on the
visible screen (TR2); an `Ausgabe` goes out when a row or the cursor differs from the last report, so a pure cursor step is
an `Ausgabe` without rows and with a rising `stand`. The cursor `[0, 0, true]` of an empty grid counts as reported (the
service gives it to the core as its start). At a size change the carrier clamps the cursor it reported to the new size
like the core does and compares the cursor of the new grid with that one (TR3); `abbild_gleich` includes the cursor. The
carrier answers the cursor query `ESC[6n` itself (TR14, requirement A109), on every platform and whatever the screen store
does: `ESC [ <row> ; <column> R`, counted from 1, from the cursor of its grid at the last byte of the query, exactly once
per query and in the order of the queries; the same answer of `alacritty_terminal` is dropped (`src/raster.rs`,
`cursor_antworten` and `Zuhoerer`). Under ConPTY the child does not start without it: `portable-pty` 0.9.0 opens the
pseudo console with `PSEUDOCONSOLE_INHERIT_CURSOR`, and ConPTY asks for the cursor before the first byte of the child.
The reference and the contract are kept outside the repository; the
cases below were made from them. It knows no time, no viewer, no setting except the scrollback, no session name and no
socket.

- **The library.** `Raster::neu(columns, rows, scrollback)` (a `Result`), `Raster::bytes` (one chunk of child bytes, the
  events in the order of TR2), `Raster::groesse` (a `Result`), `Raster::antworten` (the bytes to write back to the child,
  TR10) and the state accessors (`zeilen`, `cursor`, `ruecklauf_zahl`, `abbild_gleich`, `stand`). `Kind::starten(Start {
  argv, env, cwd, columns, rows, scrollback })` opens the PTY, starts the child with exactly the given environment and reads
  the PTY in one thread; `Kind::lies` waits for the next piece and hands out its events, `Kind::schreibe` queues bytes
  for the PTY and never waits (TR15, below), `Kind::groesse` sets the size of the PTY and of the grid. A size outside 2 to 1000 columns and 1 to 1000 rows
  is refused: `Raster::neu`, `Raster::groesse` and `Kind::groesse` return `Err(GroesseUngueltig)`, `Kind::starten` returns
  `Err(Fehler::GroesseUngueltig)`, and nothing changes (grid, cursor, scrollback, `stand` and the size of the PTY).
  Dropping a `Kind` ends a child that still runs and collects it.
  `traeger::laufen` runs one case (the code of the program `tr`).
- **The program `tr`** (`src/main.rs`): `tr [case.json | -]` reads one case and writes one JSON object. Exit code 0: the
  result is on stdout. Exit code 2: the case is invalid (not JSON, not an object, over 4,194,304 bytes, nested deeper than
  128 levels, a wrong `art`, `groesse`, step, `hex`, `start`, `env` or `argv`); stdout stays empty and one line goes to
  stderr. A step `Groesse` has the optional field `versuch` (a truth value, default false): without it a size outside the
  limits is an invalid case (exit 2); with `versuch: true` the step takes whole numbers from 0 to 100,000 and a size outside
  the limits is refused (`"ok": false`, `"fehler": "groesse_ungueltig"`, nothing changes) and the case goes on. `tr --kind <name> [arguments]` runs one fixed child program of the tests (`ausgabe`, `ende`, `zeilen`,
  `groesse`, `eingabe`, `umgebung`, `umgebung_alle`, `ordner`, `stumm`, `cursor <hex>`, `titel` and `signal <n> [<hex>]` of the contract; `abfrage` only for the tests
  of this crate); `cursor` writes the bytes and `ESC[5n` as an end mark in raw mode, reads the answers
  for 1 s at most and writes `ANTWORT <hex>` (or `ZEIT <hex>` when the second ran out); `signal` writes the bytes if given, sends itself the signal with `sh -c 'kill -<n> <pid>'` and waits; the child cases start `tr` itself in the PTY. An unknown name ends with 64.
- **The measurement aid `start_messen`** (`examples/start_messen.rs`, not part of the contract): starts one program in the
  carrier (`Kind::starten`, under Windows ConPTY) with exactly the environment of a file, measures the time from the start
  to the input field (`?forshortcuts`, `❯` or `>Try` in the screen text without white space), then sends the given keys
  (hex) and prints the screen. Used for A109 (Claude Code under ConPTY) and A114 (Shift+Enter as `ESC CR`) in the Windows VM.
- **Limits.** Sizes 2 to 1000 columns and 1 to 1000 rows (the smallest size is 2 columns by 1 row; one column, 0 and more
  than 1000 are refused, see the size check below), a scrollback up to 100,000 lines (default 2000), an OSC of more than
  100,000 bytes and a title of 3000 characters go through whole. No input panics.
- **Security places.** Three places are small and marked `SECURITY` in the source. TR7: a clipboard read (OSC 52 with `?`)
  is reported and never answered, the data are never decoded, and the real clipboard is never touched
  (`src/scan.rs`, `osc_dispatch`). TR10: only `PtyWrite` goes back to the child; the event listener in `src/raster.rs`
  drops every other event of the grid, so colour, text area size and clipboard requests are never answered.
  TR9: a size sequence `ESC [ 8 ; rows ; cols t` is reported and never executed (`src/scan.rs`, `csi_dispatch`); the grid
  and the PTY change size only through `groesse`. TR13 (the size check): the smallest size is 2 columns by 1 row. The
  check stands in front of every call into `alacritty_terminal` and in front of every size of the PTY: `Raster::neu`
  (before `Term::new`), `Raster::groesse` (before `Term::resize`), `Kind::starten` (before `openpty` and before the grid) and
  `Kind::groesse` (before the PTY resize); all four in `src/raster.rs` and `src/kind.rs`, marked `SECURITY (TR13)`.
- **The smallest size is 2 columns (decision of 20.11.2026).** `alacritty_terminal` 0.26.0 has two faults that occur only in
  a grid of one column: a character of width 2 panics (`grid/mod.rs:439`), and `Term::resize` to one column never ends when a
  wide character is in the grid (`grid/resize.rs`; it allocates until the allocation fails). The library names
  `term::MIN_COLUMNS = 2` but does not check it in `Term::new` and `Term::resize` (alacritty/alacritty issue 9112). So the
  minimum replaces the protection of the first build: the carrier refuses one column, never clamps it, and the former special
  paths (`schmal`, `schmal_rest`, `breite_entfernen`, `entbreiten`, which turned wide characters into U+FFFD at one column)
  are gone. At 2 columns the library is safe (measured with the reference without any protection: 13,000 random cases on
  the Linux build host and 3000 on the Mac, no panic, no hang). A character of width 2 fills a whole row at 2 columns; in the last column it
  wraps (TR2).
- **Named differences from `alacritty_terminal` 0.26.0.** (1) Skin tone modifiers (U+1F3FB to U+1F3FF) are zero-width
  (decision of 18.11.2026). (2) Variation selectors (U+FE0F, U+FE0E) and the zero-width joiner (U+200D) behave as in
  `alacritty_terminal` (decision of 19.11.2026): the carrier has no table of characters with an emoji presentation. What is
  promised is that nothing crashes and that carrier and view use the same width; a character such as `U+263A U+FE0F` stays
  one cell wide in the carrier. (3) An incomplete UTF-8 character at the end of a chunk is held back until the next chunk
  completes it, at every number of columns (TR2; `src/raster.rs`, `umschreiben`). `vte` does not do this itself: without it
  `65 cc | 81 65 cc 81` gives another result than `65 cc 81 65 cc 81`.
- **Signals.** A child ended by a signal reports `Kindende` with 128 plus the signal number (decision of 19.11.2026). The
  number comes from the wait status (`WIFSIGNALED`, `WTERMSIG`, through `std::process::ExitStatus`), not from the text
  `portable-pty` makes of it. `portable-pty` keeps only that text, so `src/kind.rs` takes the child back as the
  `std::process::Child` it is on Unix (`downcast_mut`) and waits for it there; there is no `unsafe` and no new
  dependency. Where the child is no `std::process::Child` (Windows) the exit code is used. A wait that fails reports 255.
- **One `openpty` at a time.** `Kind::starten` takes a lock around `openpty`: on macOS the call fails with an unknown error when
  several threads make it at once (a test binary with 14 sessions started together lost about a third of its runs).
- **Cases.** `tests/faelle/` holds the 133 visible outside cases of the contract and `tests/erwartet/` the expected result of
  each (`{"exit": 0, "ausgabe": ...}` or `{"exit": 2}`). The expectations come from the reference; all data are invented.
  `tests/faelle.rs` runs every case through the library and through the program `tr`. Two optional fields of a case belong to
  the runner, not to `tr` (TR16): `system` (`linux`, `macos`, `windows`: where the case applies; elsewhere it is not run and
  counts as not applicable, never as green; the child programs with `stty` or `sh` carry `["linux", "macos"]`, the two cases of
  W7 carry `["windows"]`) and `last` (1 to 64 busy threads while the case runs, for the case under load of A109; only with
  `TR_LAST=1`, which is set only on the Linux build host under the board: without it the case does not apply, never green, never red).
  A wait of a child case names a state, never a time: after the text `BEREIT` the cases wait for the cursor `[1, 0, true]` (the step
  `Warte` with `bis` `cursor`), because the text of a line is on the screen before the line feed behind it is read.
- **Other tests.** `tests/grenzen.rs` (the program on invalid and large input: every bad case ends with exit 2, empty
  stdout and one line on stderr; the input limit to the byte; nesting; arguments; sizes 2 by 1 to 1000 by 1000; a scrollback
  of 100,000 lines; an OSC of 136,640 characters; a fixed-seed generator of odd bytes at odd sizes (with refused tries) that
  must neither panic nor hang; wide characters at 2 columns; the refusal of a size: at the start, at a size change, in the
  main and in the alternate screen, nothing changes). `tests/kinder.rs` starts real children in a real PTY with the fixed child programs: the
  output before `Kindende`, the exit codes, the signal codes, the environment, the working folder, keys unchanged, the PTY
  size, the answer to a device query going back to the child, the output of a child that is then ended by a signal, and that
  that a size outside the limits starts no child, that a refused size leaves the PTY and the grid as they were, and that
  dropping a `Kind` ends and collects the child. `tests/grenzen.rs` also checks the cursor in `Ausgabe` (a pure cursor step,
  the start cursor, DECTCEM, the order before another event, the clamp at a size change, never a place in the scrollback),
  the answers of TR10 (only the fixed ones; none to colour, size, title or clipboard requests), and that the alternate
  screen keeps its content and cursor at 2 columns.
  `TERM`, `LC_ALL` and the folder are set in the tests; no answer depends on a process id, a path, the time or how long a
  run takes.
- **Dependencies.** Three crates by name: `alacritty_terminal` 0.26.0 and `portable-pty` 0.9.0 (both pinned with `=`, because
  the contract was measured with them and another version may answer device queries or draw the grid differently) and
  `serde_json` (writes the result of the program; already in the graph). `unicode-width` was a fourth dependency of this crate
  (the crate and version `alacritty_terminal` uses to measure a character; the former one-column special paths needed it to
  find wide characters). Since 20.11.2026 the special paths are gone; the code was built and the gates `fmt`, `clippy`, `test`,
  `licenses` and `notices` were green without them first, then the dependency was taken out of `Cargo.toml` and the gates ran
  again, green. `unicode-width` 0.2.3 stays in `Cargo.lock` and in the graph as a dependency of `alacritty_terminal`; what
  changed is only the entry of `traeger` in `Cargo.lock` and the list of direct dependencies of `traeger` in
  `sbom/traeger.cdx.json` (48 components before and after). `Cargo.lock` adds 61 packages to the workspace graph: `alacritty_terminal` 0.26.0 with `vte` 0.15.0,
  `cursor-icon` 1.2.0, `home` 0.5.12, `log` 0.4.35, `parking_lot` 0.12.5, `parking_lot_core` 0.9.12, `lock_api` 0.4.14,
  `scopeguard` 1.2.0, `polling` 3.11.0, `rustix` 1.1.6, `rustix-openpty` 0.2.0, `linux-raw-sys` 0.12.1, `signal-hook` 0.4.6,
  `signal-hook-registry` 1.4.8, `base64` 0.22.1, `bitflags` 2 and 1.3.2, `libc`
  0.2.191, `errno` 0.3.15, `cfg-if` 1.0.5, `cfg_aliases` 0.1.1, `unicode-width` 0.2.3, `arrayvec` 0.7.8, `atomic-waker` 1.1.2,
  `concurrent-queue` 2.5.0, `crossbeam-utils` 0.8.23, `fastrand` 2.5.0, `futures-io` 0.3.34, `pin-project-lite` 0.2.17,
  `piper` 0.2.5, `hermit-abi` 0.5.3, `redox_syscall` 0.5.18; `portable-pty` 0.9.0 with `anyhow` 1.0.104, `downcast-rs`
  1.2.1, `filedescriptor` 0.8.3, `lazy_static` 1.5.1, `nix` 0.28.0, `serial2` 0.2.39, `shared_library` 0.1.9, `shell-words`
  1.1.1, `thiserror` 1.0.69 with `thiserror-impl` 1.0.69 and `syn` 2.0.119, `winapi` 0.3.9, `winreg` 0.10.1 and `miow` 0.6.1
  (the last three, with the `windows-*` crates `windows-link` 0.2.1, `windows-sys` 0.59.0 and 0.61.2, `windows-targets` 0.52.6
  and its `windows_*` target crates, are Windows only and not compiled on macOS or Linux). All of them are MIT or Apache-2.0
  or both; `unicode-ident` (reached through `syn`) is `(MIT OR Apache-2.0) AND Unicode-3.0` and has the named exception in
  `deny.toml`. Three names occur in two versions, and `deny.toml` skips the older or second version of each with its reason:
  `bitflags` (1.3.2 from `portable-pty`), `syn` (2.0.119 from `thiserror-impl` 1) and `windows-sys` (0.59.0 from
  `alacritty_terminal`). `multiple-versions` stays `deny`. No source of the crate is copied from another project, so the
  crate has no row in "Code taken from other projects".
- **Runtime.** The program needs `stty` in the `PATH` of the child cases only for the child programs `groesse` and
  `eingabe` (a test matter); the carrier itself starts only the child it is told to start.

## `rasterauszug` (screen extract of the session view)

`crates/rasterauszug` is the screen extract of the session view. A terminal line is a text with SGR sequences; the
crate reads it into runs, keeps one model picture per viewer and session number, and turns the messages `Abzug`
and `Rasteraenderung` of `zustandskern` into frames: a full picture or the difference to the model picture, with
detection of a scroll up (model M2 of the measurement). A frame has a fixed byte form (A-B) that the crate also reads
back strictly, and the frames of one connection go through one raw Deflate stream. The contract is the rules RA1 to
RA14; the reference implementation and the contract are kept outside the repository, the cases below were made from
them. The library (`src/lib.rs`) reads no file, starts no process, opens no socket and asks for neither the time nor a
random number. Only `src/main.rs` touches the outside world.

- **Library.** `zeile_lesen`, `zeile_schreiben` and `zeile_pruefen` read, write and check the line form (RA1 to RA4).
  `Verbindung` is the sender of one viewer: `nimm(nr, &message)` returns the bytes on the wire, `nimm_roh` the frame
  before Deflate, `verpacken` the step in between, `vorbild(nr)` the model picture, `vergessen(nr)` forgets a session
  number. `Empfang` is the decoder of the viewer: `nimm(&bytes)` returns the extract and the picture, `nimm_roh` also
  the inflated frame. `rahmen_kodieren`, `rahmen_dekodieren` and `unterschied` are the pieces below them.
  The session number is the one the caller passes; the name of the session is not read. A rejected message or frame
  changes nothing (except the Deflate context of the receiver, RA11). `Fehler::code()` gives the codes of the contract.
- **The program `ra`** (`src/main.rs`): `ra [case.json | -]` reads one case from a file, from `-` or, without an
  argument, from standard input, and writes `{"ergebnisse": [...]}` as one JSON line, one result per step (`Zeile`,
  `Laeufe`, `Verbinden`, `Nachricht`, `Rahmen`, `Vergessen`, `Trennen`). The program is a pure function of its input.
- **Limits.** At most 4,194,304 bytes of input (`EINGABE_MAX`, read to one byte more); JSON nested deeper than 128
  levels (the default of `serde_json`); a picture of 2 to 1000 columns and 1 to 1000 rows (a size with one column is `groesse_ungueltig`, in a message `Abzug` and in a frame with the flag size; the model picture stays as it was; the reason is issue 9112 of `alacritty_terminal`, see `zustandskern` above); a frame of at most 16,777,216 bytes
  after inflating (`RAHMEN_MAX`, rule RA11: the inflater stops there and the receiver of the connection is broken
  after it, as after an inflate error); a window of 32 KiB. A whole number is an `i64`; a number with a fraction or an
  exponent is no whole number.
- **Exit codes.** 0: the results are on standard output. 2: the case is invalid (not JSON, not an object, over the
  input limit, nested too deep, `schritte` missing or not a list, a step without text `art` or of an unknown `art`, a
  missing field or one of the wrong type, `nr` not a whole number from 0 to 255, an unknown or doubled connection,
  `hex` that is no lower-case hex of even length, `laeufe` of the wrong shape, an unreadable file or folder, more than
  one argument); standard output stays empty and one line goes to standard error. 1: the results could not be written.
  A fault in the message of a step is a result with `ok` false, not an exit code.
- **Cases.** `tests/faelle/` holds the 96 visible outside cases of the contract (compacted copies; a case that is not
  JSON is copied as it is) and `tests/erwartet/` the expected result of each (`{"exit": 0, "ausgabe": ...}` or
  `{"exit": 2}`). The test `tests/faelle.rs` runs every case through the library. The Deflate bytes are never
  compared (zlib and `miniz_oxide` differ): the test checks `leitung_hex` and `leitung_laenge` like the outside runner,
  and the ratios of `leitung_erwartung`.
- **Other tests.** `tests/rundreise.rs` holds the two round trips of the contract on 10,000 states each with a fixed
  seed: line form (`lesen(schreiben(z)) == z`, and for random valid text `lesen(schreiben(lesen(s))) == lesen(s)`) and
  encoder to decoder (20,000 messages in 2,500 sessions: the picture of the receiver equals the model picture of the
  sender, the frame equals the re-encoded extract). `tests/robust.rs` feeds random and mutated frames, random wire bytes,
  random JSON messages and random text and checks that nothing panics and that a rejected input changes no state.
  `tests/grenzen.rs` starts the built `ra` (the only test that does): the input limit to the byte, nesting, bad UTF-8,
  truncated and empty input, a file argument, a missing file, too many arguments, a folder. `tests/nebenwirkung.rs`
  reads the sources and fails on any forbidden pattern (`std::fs`, `std::io`, `std::process`, `std::net`, `std::env`,
  `std::thread`, `std::time` and others in the library) and on a source file missing from its list. The unit tests are
  in `src/tests/*.rs`: every SGR number, every error code, every check of the decoder, the scroll rule, the message
  fields, the Deflate limit and the distance check.
- **Deflate.** The sender is `flate2::Compress` (level 6, raw, one context per connection, sync flush, the four end
  bytes `00 00 ff ff` cut). The receiver puts the four bytes back and inflates with `miniz_oxide`'s core inflater into
  a contiguous buffer with the last 32 KiB of the connection in front of the new bytes. This is why `miniz_oxide` is a
  direct dependency (the same package and version `flate2` uses, so the lock file holds no new package for it):
  `flate2` hands out only the streaming inflater, and that reads a match that reaches before the start of the stream
  as zero bytes and accepts the stream, where zlib answers "invalid distance too far back" and the contract says an
  invalid stream is `inflate_fehler`. A differential run against the reference with bit flips in valid streams showed the difference in 11 of 1,500 cases before the change and in none after it.
- **Dependencies.** Three: `serde_json` 1.0.154 (default features; reads the case and the messages and builds the
  results as JSON values; `serde_json` is the dependency of `zustandskern` and `rangkern` as well), `flate2` 1.1.11
  without default features and with the backend `miniz_oxide` only (`rust_backend`), and `miniz_oxide` 0.9.1. They bring `itoa` 1.0.19, `memchr` 2.8.3,
  `serde_core` 1.0.229, `zmij` 1.0.23, `crc32fast` 1.5.5, `cfg-if` 1.0.5, `adler2` 2.0.1 and `simd-adler32` 0.3.10
  (`cargo tree -p rasterauszug --locked -e normal`; versions from `Cargo.lock`, the same as in
  the lock file of the measuring runner of the contract). Neither the lock file nor the SBOM lists `zlib-rs`. Up to 20.11.2026 both did,
  although it was never compiled: the default feature `runtime_detection` of `flate2` names it as a weak dependency
  (`zlib-rs?/std`). Without the defaults `crc32fast` loses its feature `std`; raw Deflate computes no CRC-32. Licences by `cargo metadata`: `flate2`,
  `crc32fast`, `cfg-if` and `serde_json` MIT OR Apache-2.0, `miniz_oxide` MIT OR Zlib OR Apache-2.0, `adler2` 0BSD OR
  MIT OR Apache-2.0, `simd-adler32` MIT; `./gates.sh licenses` passes with no exception in `deny.toml`. No source of
  the crate has a row in "Code taken from other projects" (the code follows the measuring runner of the contract
  `rasterauszug`).

## `ansicht` (browser view, Dioxus-web)

`crates/ansicht` is the browser view of the new interface: the code view of today's workbench (A157 stage 2a) as a thin view (A118) over the
stream of the service. It is built for `wasm32-unknown-unknown`; `wb-bruecke` serves it. Layout, sizes, colours and texts follow appendix A of
the inventory of the code view (values read on `main`, no code of Electron or the Mac app). The view keeps only what it shows: `kern.rs` holds
the rules of the contract `seite` (events in, effects out, no DOM, unit tests on the host), `oberflaeche.rs` the components, `netz.rs` the three
routes. The raster frames are read with `rasterauszug::Empfang`, so there is one decoder in the new build. No `unsafe`; the binary has no
`#[wasm_bindgen]` export (wasm-bindgen starts `main` of a wasm32 binary).

- **Build.** `bash ansicht/bauen.sh` (on the Linux build host, tools from `~/.local/share/wb-rust-werkzeuge`, `wasm-bindgen-cli` in the version of
  `wasm-bindgen` in `Cargo.lock` under `wasm-bindgen-<version>/bin`) builds with the profile `ansicht` (release, `opt-level = "s"`,
  `panic = "abort"`) into its own target folder, runs `wasm-bindgen --target web --out-name ansicht`, and `ansicht/buendeln.mjs` (node, no
  packages) inlines the three JavaScript snippets of `dioxus-interpreter-js` and `dioxus-web` into `ansicht.js`, starts the module at its end
  and writes `index.html` from `ansicht/vorlage.html` and `ansicht/ansicht.css`. The output `ansicht/ausgabe/` is flat and fixed: exactly
  `index.html`, `ansicht.js`, `ansicht_bg.wasm`, only relative references. Paths of the build are remapped, so the output is the same in
  every folder.
- **Check.** `bash ansicht/test/gleichheit.sh` builds afresh into a throw-away folder and compares the three files byte for byte with the
  checked-in output, and checks the form. After any change of the crate, `vorlage.html` or `ansicht.css`, rebuild and commit the output.
- **Contract.** The cases of the contract `seite` run with `node laeufer/se_lauf.mjs --seite ansicht/ausgabe` of the contract package; the
  runner must serve `/ansicht.js` and `/ansicht_bg.wasm` and allow `'wasm-unsafe-eval'` (the contract package carries this change since the addendum of 21.11.2026).
- **Paths without a route.** Every operating path of the inventory that belongs to stage 2a has an element with `data-bw="Bnn"`. A path without
  a route in the new build is a disabled control whose name and hint say "noch nicht angebunden".

## `bruecke` (the bridge of the walking skeleton)

`wb-bruecke <startdatei.json>` is a program of the walking skeleton: the one door the browser of a viewer reaches. It speaks HTTP/1.1 itself with `std`
(no framework, no HTTP crate, no TLS), binds only `127.0.0.1` and hands bytes to the Unix socket of the service `wb-dienst`, which stays without TCP. The
viewer reaches it through `ssh -L`. The contract is `rust/spec/VERTRAEGE.md` (BR1 to BR6); the crate README (`crates/bruecke/README.md`) holds the
start file, the routes and the layout.

- **Security points.** (1) The token file and the port file: the folder is a real folder of the effective user with exactly the mode 0700 (`lstat`, no link),
  the file must not exist, it is created exclusively with the mode 0600, nothing is ever replaced or deleted that was there (`src/rechte.rs`). (2) The token has 32
  bytes from `/dev/urandom` (64 hex characters), is compared in constant time without `==` on byte sequences (`src/token.rs`), is checked before anything goes
  to the service, and is in no output and no protocol line. (3) The order of the checks (form, `Host`, `Origin`, token, method and route) stands in one function
  (`src/pruefung.rs`, `pruefen`); `Host` and `Origin` are compared byte for byte. (4) The address is `Ipv4Addr::LOCALHOST` in `server::binden`; it is no
  parameter and no start key reaches it.
- **Limits.** Head 8192 bytes and 64 head lines, target 2048 bytes, body 16 384 bytes for `/starten` and 1 048 575 bytes for `/eingabe`, 64 connections at once
  (the next gets 503 and no thread), 10 s to read head and body. A broken request ends only its own connection.
- **No new dependency in the graph.** `serde_json` reads the body and writes the start event. `signal-hook` and `rustix` are used because `std` cannot catch
  SIGTERM or read the effective user id without `unsafe` (see the table "Dependencies"). The view is built in with `include_bytes!` from
  `ansicht/ausgabe/index.html`, `ansicht/ausgabe/ansicht.js` and `ansicht/ausgabe/ansicht_bg.wasm` (routes `GET /`, `GET /ansicht.js`,
  `GET /ansicht_bg.wasm`, addendum of 21.11.2026; `script-src` carries `'wasm-unsafe-eval'`). The page of the walking skeleton
  (`seite/ausgabe/`) is no longer served; `/seite.js` is an unknown target. The bridge reads no file at run time except the start file.
- **Tests.** `cargo test -p bruecke`: the unit tests, `tests/programm.rs` (the program as a process, SIGTERM and SIGINT) and `tests/aussen.rs` (the 60 visible
  outside cases of the contract, with the check client and the Python reference of the service as copies in `crates/bruecke/tests/aussen/`).
- **Measurement.** `tests/messung.rs` is ignored in the gates. Run it under the board, for example:
  `cargo test -p bruecke --release --test messung -- --ignored --nocapture`, alone on an otherwise idle Linux host.
  It prints the threads of the process without a connection, with 63 and with 64 streams and after they end, the answer time of 300 page requests while 63
  streams are open, the time of the answer 503 and the resident memory.

<!-- vom Bauvorgang angefuegt -->
> Hinweis: Dieser Text nennt Werkzeuge, die in diesem Klon nicht liegen: `brain`.
> Sie gehoeren zum Setup der Maschine, aus der dieser Baum stammt; hier fehlen
> sie mit Absicht. Wer sie braucht, schreibt sich seine eigenen.
> *Named here, not shipped here.*
