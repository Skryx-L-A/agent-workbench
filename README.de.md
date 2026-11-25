# agent-workbench — development

[English](README.md) | Deutsch

**Dieser Zweig enthält das Produkt im Aufbau: die Agenten-Werkbank als Neubau in Rust.** Die
getestete Werkbank, die heute im Einsatz ist, ein Electron-Fenster mit Kommandozeilenwerkzeugen,
liegt auf dem Zweig [`main`](https://github.com/Skryx-L-A/agent-workbench/tree/main).

## Was es ist

Der Neubau macht aus der Werkbank ein Paket, das eine Firma auf ihren Rechnern installieren kann.
Der Rust-Arbeitsbereich in `rust/` enthält seine Bausteine, darunter:

- **einen Kerndienst** (`wb-dienst`), der jede Sitzung in einem eigenen Terminal betreibt und jedem
  Betrachter eine Ansicht davon liefert, mit einem Zustandskern und einem Rasterauszug, die nur
  Änderungen senden;
- **eine Browser-Ansicht** (`ansicht`, gebaut für WebAssembly) und eine Brücke (`wb-bruecke`), die
  sie nur auf `127.0.0.1` ausliefert;
- **einen Einschluss je Sitzung** unter Linux, gebaut aus einem festen bubblewrap-Profil, mit einem
  Startprogramm, das Startdatei und Einstellungen prüft, bevor etwas läuft;
- **einen lokalen Modellweg**, der genau eine Route und ein Modell durchlässt und den Schlüssel
  außerhalb des Einschlusses hält;
- **einen Suchkern** über Markdown-Notizen (BM25 und Vektoren, kein Modell im Prozess) mit einem
  kleinen MCP-Werkzeug für Agenten;
- **einen Paketbau** (`rust/paket.sh`) für ein Debian-Paket und einen Prüfbefehl für das
  installierte Paket.

`rust/README.md` beschreibt jede Crate und ihren Vertrag (englisch).

## Stand

In Arbeit, **nicht für den Betrieb freigegeben.** Eine installierbare Version gibt es noch nicht.
Der Paketbau zielt auf Ubuntu 26.04 auf x86_64 und erzeugt bisher nur einen Testbau. Einige
Schutzfunktionen, die eine Firmeninstallation braucht, sind noch nicht vollständig;
[SECURITY.md](SECURITY.md) nennt die offenen Punkte. Betreiben Sie diesen Zweig nicht auf einem
Rechner mit fremden Daten.

[CHANGELOG.md](CHANGELOG.md) nennt die Änderungen jedes veröffentlichten Stands.

## Zweige

- `main` ist die getestete Werkbank. Er wird erst aktualisiert, wenn der vollständige Testlauf des
  Quellbaums für genau den veröffentlichten Stand bestanden ist.
- `development` (dieser Zweig) ist das Produkt im Aufbau. Er wird aktualisiert, wenn die
  Rust-Tore unten bestanden sind.
- Jede Aktualisierung eines der beiden Zweige ist ein Schnappschuss. Die private Arbeitshistorie
  wird nicht veröffentlicht.

## Bauen

Sie brauchen [rustup](https://rustup.rs). Die Version der Toolchain steht fest in
`rust/rust-toolchain.toml` (derzeit 1.99.0 mit rustfmt und clippy); rustup installiert sie beim
ersten Aufruf von `cargo`.

```bash
git clone --branch development https://github.com/Skryx-L-A/agent-workbench.git
cd agent-workbench/rust
cargo build --workspace --locked
```

Die Browser-Ansicht wird für `wasm32-unknown-unknown` gebaut; siehe den Abschnitt `ansicht` in
`rust/README.md`.

## Tore

Jeder veröffentlichte Stand hat die Tore von `rust/gates.sh` bestanden. Das Skript führt je Aufruf
ein Tor aus.

**Der Werkzeugordner.** Die Tore nehmen ihre Werkzeuge nur aus einem Ordner je Rechner,
`~/.local/share/wb-rust-werkzeuge`: `rustup/` ist `RUSTUP_HOME`, `cargo/` ist `CARGO_HOME`, und
`tools/` enthält cargo-deny 0.21.0, cargo-cyclonedx 0.5.10 und cargo-mutants 27.1.0. `gates.sh`
setzt `PATH`, `RUSTUP_HOME` und `CARGO_HOME` nur für den eigenen Lauf und nutzt nie ein anderes
Rust des Rechners. Fehlen der Ordner, die Toolchain oder eine Werkzeugversion, endet ein Tor mit
Exit 127; `./gates.sh tools` macht nur diese Prüfung. `rust/tools/werkzeuge-installieren.sh` legt
den Ordner an:

```bash
cd rust
bash tools/werkzeuge-installieren.sh                     # Trockenlauf: zeigt, was es täte
bash tools/werkzeuge-installieren.sh --install --target "$HOME/.local/share/wb-rust-werkzeuge"
./gates.sh tools
```

Es lädt den rustup-Installer von `static.rust-lang.org`, vergleicht seine SHA-256 mit der für die
Plattform festgelegten Prüfsumme (`aarch64-apple-darwin`, `x86_64-unknown-linux-gnu`), startet ihn
mit `--no-modify-path`, baut die drei Werkzeuge mit `cargo install --locked` und prüft, dass sich
außerhalb des Ordners nichts geändert hat. Es braucht kein sudo. Auf anderen Plattformen führen Sie
die cargo-Befehle unten mit der Toolchain aus `rust/rust-toolchain.toml` aus. Einzelheiten:
`rust/README.md`.


| Tor | Befehl |
|---|---|
| `./gates.sh fmt` | `cargo fmt --all --check` |
| `./gates.sh clippy` | `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` |
| `./gates.sh test` | `cargo test --workspace --all-targets --all-features --locked --no-fail-fast` |
| `./gates.sh licenses` | `cargo deny --locked --all-features check licenses bans sources` und eine CycloneDX-SBOM je Crate |
| `./gates.sh notices` | jedes Stück Code aus einem anderen Projekt trägt seinen Hinweis und wird mit ihm ausgeliefert |
| `./gates.sh mutants` | `cargo mutants --workspace`: keine Mutante überlebt |

Die fortlaufende Integration führt die ersten fünf Tore unter Linux und macOS über das Skript aus
und fmt, clippy, test und cargo-deny mit denselben Befehlen unter Windows. Alle sind Pflicht. Das
Mutationstor dauert Stunden und läuft, bevor ein Stand veröffentlicht wird.

## Mitmachen

Pull Requests gehen gegen diesen Zweig. Der Maintainer übernimmt eine Änderung als Patch in den
Quellbaum und nennt Sie im Commit; danach wird der Pull Request mit einem Hinweis geschlossen. Ein
Pull Request braucht die einmalige Zustimmung zur Beitragsvereinbarung (CLA). Einzelheiten:
[CONTRIBUTING.md](CONTRIBUTING.md). Fragen gehören in die Discussions, siehe
[SUPPORT.md](SUPPORT.md).

Kleine, gut beschriebene Aufgaben für den Einstieg sind offene Issues mit dem Label
[`good first issue`](https://github.com/Skryx-L-A/agent-workbench/issues?q=is%3Aissue+is%3Aopen+label%3A%22good+first+issue%22).

## Lizenz

Die eigene Arbeit des Lizenzgebers steht unter der [Apache License 2.0](LICENSE); fremde Bestandteile
und die Namensnennung stehen in [NOTICE.md](NOTICE.md). Es gilt nur der englische Lizenztext. Namen
und Logos sind nicht mitlizenziert; siehe [TRADEMARKS.md](TRADEMARKS.md). Einrichtung, Support und
Schulung gibt es als bezahlte Leistung; siehe [SUPPORT.md](SUPPORT.md).

Frühere Stände behalten die Lizenz, unter der sie veröffentlicht wurden; siehe
[CHANGELOG.md](CHANGELOG.md).
