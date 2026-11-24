# Contributing

Contributions are welcome. Please open an issue or a discussion first for larger changes, so the
approach can be agreed on before you invest time. Questions belong in Discussions, see
[SUPPORT.md](SUPPORT.md).

## Branches

- `main` is the tested workbench: the desktop window, its core and the command-line tools. It is
  updated only after the full test run of the source tree has passed for exactly that state.
- `development` is the product being built: the Rust rebuild under `rust/` and what it needs to
  build, test, run and package. It is updated after the Rust gates have passed.

Both branches are snapshots of a private source tree. Their history shows one commit per
published state, not the working history.

## Pull requests

Open pull requests against `development`, also for a fix you found on `main`. The maintainer
decides whether a change belongs on `main` as well.

A pull request is not merged on GitHub. The maintainer takes the change over into the source tree
as a patch. You are named in the resulting commit, as co-author (`Co-authored-by:` with the name
and address of your commit) or in the commit message, and the change then reaches the public
branch with the next published state. The pull request is closed with a note that says where the
change went. If the maintainer has to change your patch, the note says so.

Keep a pull request to one topic. Write commit messages in English. Describe what changes and
why, and how you tested it.

## Building and testing

### `development` (Rust)

The Rust workspace is in `rust/`. The toolchain version is fixed in `rust/rust-toolchain.toml`;
rustup installs it on the first `cargo` call.

```bash
cd rust
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo test --workspace --all-targets --all-features --locked --no-fail-fast
cargo deny --locked --all-features check licenses bans sources
```

These are the commands of the gates in `rust/gates.sh`, which the maintainer runs before every
published state. The script uses its tools only from one folder per machine,
`~/.local/share/wb-rust-werkzeuge`, in pinned versions (toolchain, cargo-deny, cargo-cyclonedx,
cargo-mutants), and never another Rust on the machine. `rust/tools/werkzeuge-installieren.sh` makes
the folder on arm64 macOS and x86_64 Linux; without arguments it only shows what it would do:

```bash
cd rust
bash tools/werkzeuge-installieren.sh --install --target "$HOME/.local/share/wb-rust-werkzeuge"
./gates.sh tools
./gates.sh fmt    # likewise clippy, test, licenses, notices, mutants
```

Details: `rust/README.md`, sections Dependencies and Gates. A new dependency must
pass the licence list in `rust/deny.toml`.

### `main` (workbench)

There is no build step for the tools in `shell/`. The checks that the continuous integration runs
are:

```bash
git ls-files -z -- '*.sh' | xargs -0 -n1 bash -n
git ls-files -z -- '*.py' | xargs -0 python3 -m py_compile
```

On `main`, the window in `app/` is built as described in the installation guide of that branch.

## Contributor License Agreement

This project is licensed under the Business Source License 1.1 with an additional use grant (see
[LICENSE](LICENSE) and, in plain words, [COMMERCIAL-LICENSE.md](COMMERCIAL-LICENSE.md)). It is
also offered under partner agreements. To keep both possible, every contributor accepts the
[Contributor License Agreement](CLA.md) once. It gives the maintainer the exclusive right to use
your contribution, while you keep the right to use your own contribution for any purpose.

To accept it, comment on your pull request with exactly this sentence:

> I have read the Contributor License Agreement and I agree to it.

A check named `cla` reports whether your acceptance was found. The maintainer takes over a change
only after the agreement is accepted for it, whatever the check shows. After your first accepted
pull request you are listed in [CLA-SIGNATURES.md](CLA-SIGNATURES.md) and do not need to comment
again in this repository.

If you contribute as part of your job or for a client, your employer or client must have signed
the Corporate Contributor License Agreement, or must have given you the rights needed or waived
its rights in writing. Write to **licensing@prontea.de**.

# Mitwirken

Beiträge sind willkommen. Bitte eröffnen Sie bei größeren Änderungen zuerst ein Issue oder eine
Diskussion, damit der Ansatz abgestimmt ist, bevor Sie Zeit investieren. Fragen gehören in die
Discussions, siehe [SUPPORT.md](SUPPORT.md).

## Zweige

- `main` ist die getestete Werkbank: das Desktop-Fenster, sein Kern und die
  Kommandozeilenwerkzeuge. Er wird erst aktualisiert, wenn der vollständige Testlauf des
  Quellbaums für genau diesen Stand bestanden ist.
- `development` ist das Produkt im Aufbau: der Rust-Neubau unter `rust/` und was er zum Bauen,
  Testen, Betreiben und Paketieren braucht. Er wird aktualisiert, wenn die Rust-Tore bestanden
  sind.

Beide Zweige sind Schnappschüsse eines privaten Quellbaums. Ihre Historie zeigt einen Commit je
veröffentlichtem Stand, nicht die Arbeitshistorie.

## Pull Requests

Eröffnen Sie Pull Requests gegen `development`, auch für eine Korrektur, die Sie auf `main`
gefunden haben. Der Maintainer entscheidet, ob eine Änderung auch auf `main` gehört.

Ein Pull Request wird nicht auf GitHub gemergt. Der Maintainer übernimmt die Änderung als Patch in
den Quellbaum. Sie werden im entstehenden Commit genannt, als Co-Autor (`Co-authored-by:` mit Name
und Adresse Ihres Commits) oder in der Commit-Nachricht, und die Änderung erreicht den
öffentlichen Zweig mit dem nächsten veröffentlichten Stand. Der Pull Request wird mit einem
Hinweis geschlossen, wohin die Änderung gegangen ist. Musste der Maintainer Ihren Patch ändern,
sagt der Hinweis das.

Beschränken Sie einen Pull Request auf ein Thema. Schreiben Sie Commit-Nachrichten auf Englisch.
Beschreiben Sie, was sich ändert, warum, und wie Sie es getestet haben.

## Bauen und testen

### `development` (Rust)

Der Rust-Arbeitsbereich liegt in `rust/`. Die Version der Toolchain steht fest in
`rust/rust-toolchain.toml`; rustup installiert sie beim ersten Aufruf von `cargo`.

```bash
cd rust
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo test --workspace --all-targets --all-features --locked --no-fail-fast
cargo deny --locked --all-features check licenses bans sources
```

Das sind die Befehle der Tore in `rust/gates.sh`, die der Maintainer vor jedem veröffentlichten
Stand ausführt. Das Skript nimmt seine Werkzeuge nur aus einem Ordner je Rechner,
`~/.local/share/wb-rust-werkzeuge`, in festgelegten Versionen (Toolchain, cargo-deny,
cargo-cyclonedx, cargo-mutants), und nie ein anderes Rust des Rechners.
`rust/tools/werkzeuge-installieren.sh` legt den Ordner unter macOS auf arm64 und Linux auf x86_64
an; ohne Argumente zeigt es nur, was es täte:

```bash
cd rust
bash tools/werkzeuge-installieren.sh --install --target "$HOME/.local/share/wb-rust-werkzeuge"
./gates.sh tools
./gates.sh fmt    # ebenso clippy, test, licenses, notices, mutants
```

Einzelheiten: `rust/README.md`, Abschnitte Dependencies und Gates. Eine neue
Abhängigkeit muss die Lizenzliste in `rust/deny.toml` bestehen.

### `main` (Werkbank)

Die Werkzeuge in `shell/` haben keinen Bauschritt. Die Prüfungen der fortlaufenden Integration
sind:

```bash
git ls-files -z -- '*.sh' | xargs -0 -n1 bash -n
git ls-files -z -- '*.py' | xargs -0 python3 -m py_compile
```

Auf `main` wird das Fenster in `app/` gebaut, wie es die Installationsanleitung dieses Zweigs beschreibt.

## Beitragsvereinbarung

Dieses Projekt steht unter der Business Source License 1.1 mit einer zusätzlichen
Nutzungserlaubnis (siehe [LICENSE](LICENSE) und, in einfachen Worten,
[COMMERCIAL-LICENSE.md](COMMERCIAL-LICENSE.md)). Es wird zusätzlich unter Partnerverträgen
angeboten. Damit beides möglich bleibt, stimmt jede beitragende Person einmal der
[Beitragsvereinbarung](CLA.md) zu (deutsche Lesefassung: [CLA-de.md](CLA-de.md)). Sie gibt dem
Maintainer das ausschließliche Nutzungsrecht an Ihrem Beitrag, und Sie behalten das Recht, Ihren
eigenen Beitrag für jeden Zweck zu nutzen.

Zur Zustimmung schreiben Sie in Ihren Pull Request als Kommentar genau diesen Satz:

> I have read the Contributor License Agreement and I agree to it.

Eine Prüfung namens `cla` meldet, ob Ihre Zustimmung gefunden wurde. Der Maintainer übernimmt eine
Änderung erst, wenn die Vereinbarung dafür angenommen ist, unabhängig davon, was die Prüfung
anzeigt. Nach Ihrem ersten angenommenen Pull Request stehen Sie in
[CLA-SIGNATURES.md](CLA-SIGNATURES.md) und müssen in diesem Repository nicht erneut zustimmen.

Tragen Sie im Rahmen Ihrer Arbeit oder für einen Auftraggeber bei, muss Ihr Arbeitgeber oder
Auftraggeber die Firmen-Beitragsvereinbarung unterschrieben, Ihnen die nötigen Rechte eingeräumt
oder schriftlich auf seine Rechte verzichtet haben. Wenden Sie sich an **licensing@prontea.de**.
