# Contracts

Each building block of the product is written against a contract: a fixed set of rules with visible cases. A build counts as
conforming when it passes every visible case of its contract. The cases are data and live in the folders named below; the
crate tests read them from there. A contract with the cases folder "crate tests" keeps its cases next to the crate under
`rust/crates/<crate>/tests/`.

This file is the public index of the contracts. Where a comment in the code names a contract, a section or a rule number,
it refers to the contract listed here.

| Id | Contract | Purpose | Cases | Crate |
|---|---|---|---|---|
| 1 | `start` | Start structure of a run and the call "start run": what a start file must hold and in which order it is checked. | `rust/spec/start/` | `start` |
| 2 | `einstellungen` | Settings file, versions 1 and 2: form, defaults and the blocks per harness. | `rust/spec/einstellungen/` | `einstellungen` |
| 3 | `datentor` | The decision `darf(unit, route, customer)`, with the sub-contracts `klassen` (class depth), `person` (reading class of the person) and `zonen` (model zones). | `rust/spec/datentor/` | `datentor` |
| 4 | `such-socket` | One search socket per session (`rangkern sitzung`), with additions for macOS and Windows. | `rust/spec/such-socket/` | `rangkern`, `such-mcp` |
| 5 | `protokollzeile` | The log line "output blocked" and the check command of the chain. | `rust/spec/protokollzeile/` | `formate` |
| 6 | `stelle` | The decision point: it decides only and executes nothing; versions 1 and 2. | `rust/spec/stelle/` | `stelle` |
| 7 | `gateway` | Gateway with model path (draft). | `rust/spec/gateway/` | none yet |
| 8 | `modellweg` | Model path: one route and one model through, the key outside the enclosure. | `rust/spec/modellweg/` | `modellweg` |
| 9 | `anmelden` | Sign-in of a command-line harness (`werkbank-anmelden`). | `rust/spec/anmelden/` | `anmelden` |
| – | `personenbezug` | Personal data: information and deletion per person (draft). | `rust/spec/personenbezug/` | none yet |
| – | `bruecke` | The bridge that serves the browser view on `127.0.0.1` only (rules BR). | crate tests | `bruecke` |
| – | `dienst` | The core service: one terminal per session and a view for every viewer (rules DI). | crate tests | `dienst` |
| – | `traeger` | The carrier between terminal and state core. | crate tests | `traeger` |
| – | `zustandskern` | The state core of a session. | crate tests | `zustandskern` |
| – | `rasterauszug` | The screen extract that sends only what changed. | crate tests | `rasterauszug` |
| – | `seite` | The browser page and its surface. | `rust/spec/seite/` | `ansicht` |
| – | `halter` | Keys held outside the session on Windows, Linux and macOS. | `rust/spec/halter/` | none yet |
| – | `klassenerbe` | A result inherits the class of its sources (draft). | `rust/spec/klassenerbe/` | none yet |
| – | `macos-einschluss` | Enclosure of an agent run on macOS. | `rust/spec/macos-einschluss/` | none yet |
| – | `windows-einschluss` | Enclosure of an agent run on Windows. | `rust/spec/windows-einschluss/` | none yet |
| – | `peerkanal` | Local channel with a check of the other side; versions 1 and 2. | `rust/spec/peerkanal/` | none yet |
| – | `regelebenen` | Rule levels: company, group, user. | `rust/spec/regelebenen/` | none yet |
| – | `schreibtor` | Every write into the knowledge store goes through one gate. | `rust/spec/schreibtor/` | none yet |
| – | `paket` | Package build, delivery list and the check command of the installed package. | `rust/spec/paket/` | `werkbank-pruefen` |
| – | `vorfuehrung` | The demonstration as outside tests: steps, runner and its configuration. | `rust/spec/vorfuehrung/` | none (runner in Python) |

"none yet" means that the cases exist and no crate of this workspace implements the contract so far.
