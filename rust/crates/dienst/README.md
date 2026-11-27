# dienst

**`wb-dienst` is the core service of the trial run (the Kern-Dienst).** The package ships it as `/usr/lib/werkbank/werkbank-kern`; it runs as a service of
the user (`systemd --user`, `RuntimeDirectory=werkbank` with mode 0700, `NoNewPrivileges=yes`, start file `/etc/werkbank/kern.json` written by `postinst`).
In the test build (feature `pruefbau`) the same program also serves as the check program of the contract cases (`--kind` runs fixed child programs of the tests); the delivered build does not know `--kind` and holds no code of them (`tests/naht.rs`). The person stands in the start
file; the socket has only folder and file rights and **no check of the peer**: the boundary is the folder with mode 0700, and who may enter it may speak.
The check of the peer comes from the platform layer (contract `dienst`, section 1).

The crate holds the service of the new session view. One session, or with the key `lauf` in the start file several (DI12): per
session a child in a PTY (through `traeger`) and a state core (`zustandskern`, one `Sitzungskern`), per viewer one extract
(`rasterauszug`, one `Verbindung` for all sessions of the viewer), a room core without a session for `Einstellungen`, `Routen`,
`Anfragen` and `Agenten`, a clock and one Unix socket. The service only
translates: events of the viewers into events of the core, effects of the core into bytes for the PTY and frames on the socket.
Every rule of the core stays in the core, every rule of the extract in the extract, every rule of the screen in the carrier.
The contract is `rust/spec/dienst/VERTRAG.md` (rules DI1 to DI12, addenda of 20.11.2026).

```
wb-dienst <start.json>           runs until SIGTERM or SIGINT; then it closes the viewers, removes the socket file, ends the child
                                 (SIGHUP, up to 1 s, SIGKILL, wait) and exits with 0
wb-dienst --kind <name> [args]   test build only (feature `pruefbau`): a fixed child program of the tests (ausgabe, geteilt, ausgabewarte, ende, signal, zeilen, klammer,
                                 eingabe, bilder, groesse, umgebung, titel, warte, still, stufen, fuellen, osc52viele, lauf)
wb-dienst --exec <flag> <prog>   the cover the service starts its child through (see Security points)
```

**Windows (addendum of 25.11.2026, contract section 21, not accepted yet):** the same program for `x86_64-pc-windows-gnu`. The socket is
`AF_UNIX`, the rights of the folder and the socket are access lists (owner and a protected list of the own user, the counterpart of
0700 and 0600), SIGTERM and SIGINT are the console events `CTRL_BREAK_EVENT` and `CTRL_C_EVENT`, the child runs in a pseudo console
(`ConPTY`) and starts without the cover. `src/sys/windows.rs` is the only file with `unsafe`; the deviations are listed in section 21.

Exit codes at the start, in the order of the checks: 2 start file (DI1), 3 rights (DI2), 4 child (DI8), 1 socket. Each error is one
line `wb-dienst: ...` on stderr, nothing on stdout, and nothing stays behind (no child, no socket file).

## Layout

| File | Content |
| --- | --- |
| `src/startdatei.rs` | DI1: the start file, every rule of section 3 |
| `src/rechte.rs` | DI2, the security point: folder 0700 by `lstat`, owner, path free, socket 0600 |
| `src/rahmen.rs` | DI3: frames `[n: u32][art: u8][payload]`, reassembly, the limit of 1 048 576 bytes at the head |
| `src/betrachter.rs` | one viewer: queue of frames (DI5), non-blocking write, the extract of `rasterauszug` |
| `src/sitzung.rs` | one core (a session with carrier and child, or the room core), state for the rebuild, start and end of the children, `anfang` for the cores |
| `src/starten.rs` | DI12 `Starten`: `start_form` and `start_pruefen` (the one place of the check of the start structure: the crate `start`), the settings file (the crate `einstellungen`) and `Startwahl`, folder and `start.json` (`start::schreiben`) |
| `src/meldung.rs` | contract section 18: the socket `meldung.sock` of a session of `Starten`, the one connection of `lauf` (user and, under Linux and macOS, pid), the line `<code> <grund>` and its judgment at the end of the child |
| `src/dienst.rs` | the loop, the steps of the core and their effects (DI4 to DI9, DI11) |
| `src/huelle.rs` | the cover of the child: the environment is exactly `kind.env` |
| `src/kindprogramm.rs` | the fixed child programs of DI8 |
| `src/json.rs` | the JSON text of the frames (keys sorted, `", "` and `": "`) |

## Several sessions (DI12)

The cores are held in the order of their start (`Dienst::sitzungen`): the room core first (only with `lauf`), then the session of
`kind` (number 0), then the sessions of `Starten` (`s-<n>`, number `n`, at most 255 numbers per run, never used again). `Verbinden`,
`Trennen`, `Takt` and `Socket` go to every core, an event of a session to the core of that session (`sitzung` in the event), the
messages `Einstellungen`, `Routen`, `Anfragen`, `Agenten` and `Leeren` come from the room core only, and the service sends its own
`Sitzungsliste` (after a start, after the end of a child, when a viewer connects). `Starten` checks in the order of the contract:
the structure (`start::lesen_anfrage`, stages 1 to 13 of contract 1), settings file (`einstellungen::lesen`, 65 536 bytes at most), `Startwahl` if it changed,
`start::pruefen` (stages 14 to 17), at most 8 running sessions, number, folder, child. A refusal of the check carries `grund` `start_pruefung`, `code`
`start.<code>` and the fixed text of the stage, as the crate gives them (the service has no list of codes); the other refusals carry `grund`, `code` and
`text` of the service. `Startwahl` carries per route `schluessel`, `art`, `klasse` and `modell` (the text of the route or `null`) and the harness table of
contract 1 (`claude`, `copilot`, `pi`; a test holds it equal to the crate). `Beenden` sends SIGHUP and the loop sends SIGKILL after 1 s; at the end of the service every child gets SIGHUP, one second
together, then SIGKILL, and every folder the service made is removed. The error of one core builds only that core again (DI6).
The frame `Kernfehler` of the test mode is not allowed with `lauf`.

## Threads

One thread (the loop) owns every state and does all the work, so that the steps of the core come in one order. It waits in one
`poll` (`rustix`, feature `event`) on the wake socket, the listener and every connection, and reads the connections that are ready
in the order of the viewer numbers, one read each per turn. So the order in which the inputs of two viewers are handled is fixed
by the readiness, not by which reading thread wakes first (there are no reading threads for connections). All sockets are
non-blocking; the loop writes to them itself and never blocks there. The other threads are the reading thread inside
`traeger::Kind` and one that waits for SIGTERM and SIGINT; the signal thread and `Anhalter::anhalten` set a flag and write one byte
to the wake socket. The loop wakes for each readable connection and at least every 2 ms (1 ms while a viewer has a backlog).

## Dependencies

`traeger`, `zustandskern`, `rasterauszug` (path), `serde_json`, `rustix` 1.1.6 (feature `process`: `geteuid` for DI2 point 2 and
`kill_process` for SIGHUP and SIGKILL), `signal-hook` 0.4.6 (SIGTERM and SIGINT; safe API only), under macOS only `nix` 0.28 (feature `socket`: `LOCAL_PEERCRED` and
`LOCAL_PEERPID` for the peer of the report of `lauf`). The reason for each stands in
`Cargo.toml`. No `tokio`, no `axum`, no `tungstenite`, no `unsafe`, no network socket of any kind.

## Security points

- DI2 (`src/rechte.rs`): the whole check of folder and socket, with its comment at the top of the file.
- DI8 (`src/sitzung.rs` `kind_starten`, `src/huelle.rs`): the environment of the child is exactly `kind.env`. `traeger::Kind` clears
  the environment, but `portable-pty` always adds `SHELL`; the cover clears it once more and leaves `SHELL` out unless the start
  file names it.
- DI12 (`src/starten.rs`): `start_form` is the one place that checks the structure of a start; the folder `s-<n>` is 0700 and
  `start.json` 0600 and an existing folder is never replaced or removed; the environment of a started child is exactly `lauf.env`
  (the same cover); `lauf.programm`, `lauf.einstellungen` and `lauf.sitzungen` come from the start file only.
  `lauf.sitzungen` (optional, an absolute path): `Starten` does not hand out a number whose entry `<lauf.sitzungen>/s-<n>` still exists (`lstat`, any kind of entry,
  a dead link too); a missing entry is free, any other error is `start_ordner`; `nummer_max` holds for every number looked at. The service creates and removes nothing there.
  `lauf.ziellisten` and `lauf.laufzeit` (optional absolute paths, DI12 point 15): a number is free only when `lstat` finds NO entry of it: `<sitzungen>/s-<n>`, `<laufzeit>/s-<n>`,
  `<ziellisten>/s-<n>.zielliste.json`, `<marker_ordner>/s-<n>.suche.json`, `<marker_ordner>/s-<n>.modellweg.json` (an entry counts only with its key; any entry of any kind takes the number,
  only `NotFound` frees it, any other error is `start_ordner`). The entries stand in one place, `eintraege_der_nummer` in `src/starten.rs`; the service creates and reads nothing there.
  DI12 point 15a: it removes one thing there. When a number above every number of this run is taken by its own regular target list alone, `alte_zielliste_raeumen`
  (`src/starten.rs`) removes that list through the descriptor of `lauf.ziellisten` (`openat` with `O_DIRECTORY | O_NOFOLLOW`, own owner, `statat` without following,
  `unlinkat`) and hands out the number. A link, a folder, a foreign file or a list next to another entry of the number stays and keeps the number taken.
  `lauf.marker_datei` and `lauf.marker_ordner` (both optional absolute paths, DI12 point 14): the service reads the marker file exactly once at its start
  (`src/markerdatei.rs`; the folder `O_NOFOLLOW`, own owner, mode 0700; the file relative to it with `O_NOFOLLOW`, own owner, mode 0600, at most 4096 bytes,
  JSON with exactly `{"liste": [...]}`; a missing file or folder is no error and gives no markers; any other failure is exit code 2 with
  `lauf.marker_datei ungueltig: <reason>`). The folder of the count comes from `lauf.marker_ordner` of the start file, never from the file; the markers are built with
  `start::Marker::neu(liste, ordner)` and go into every `start.json` (`mit_sitzung`) and nowhere else. A request of the view with `marker` stays refused (`start.marker_nicht_erlaubt`).
- DI5 (`src/dienst.rs` `abtrennen`): the log line of the hard bound holds the name of the viewer, the reason and two numbers, no
  content of the session.

## Tests

`cargo test -p dienst --all-features` (the feature `pruefbau` carries the child programs the cases need; the files that run a child program are empty without it; `cargo test -p dienst --test naht` proves that the delivered build knows no `--kind`): unit tests in `src/`, `tests/dienst.rs` (start errors, end, steady load, limit of the core, threads),
`tests/mehrere.rs` (DI12: numbers and identifiers, `Beenden` with SIGKILL for a child that ignores SIGHUP, folders and their
removal also after the crash of a session, the rebuild of one core, no listener), `tests/meldung.rs` (section 18, Linux and macOS: the option `--meldung`, the
line of `lauf` and its borders, a foreign connection, a path too long), `tests/faelle.rs` (the 280 visible outside cases
through the Python runner in `tests/werkzeug/`; needs `python3` and `lsof`). The runner repeats nothing: a red case ends its
runner, and the test is red.

<!-- vom Bauvorgang angefuegt -->
> Hinweis: Dieser Text nennt Werkzeuge, die in diesem Klon nicht liegen: `pi`.
> Sie gehoeren zum Setup der Maschine, aus der dieser Baum stammt; hier fehlen
> sie mit Absicht. Wer sie braucht, schreibt sich seine eigenen.
> *Named here, not shipped here.*
