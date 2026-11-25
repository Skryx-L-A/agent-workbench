# bruecke

`wb-bruecke` is a program of the walking skeleton. It binds only `127.0.0.1` (the address is fixed in the code, there is no switch for another one and no listener on `::1`) and it carries no TLS. The browser of a viewer reaches it through an `ssh -L` tunnel; the bridge hands bytes on to the Unix socket of the service `wb-dienst`, which stays without TCP.

The contract is `rust/spec/bruecke/VERTRAG.md` (state 20.11.2026). The bridge knows no event of the view. It checks a request (form, `Host`, `Origin`, token), translates two routes into frames of the service and passes everything else on unchanged.

**Windows (addendum of 25.11.2026, contract section 17, not accepted yet):** the same program for `x86_64-pc-windows-gnu`, with the
`AF_UNIX` socket of the service, access lists instead of the modes 0700 and 0600, `BCryptGenRandom` for the token and the console
events for SIGTERM and SIGINT. `src/sys/windows.rs` is the only file with `unsafe`.

## Call

```
wb-bruecke <startdatei.json>
```

It runs until SIGTERM or SIGINT, then closes the listener and all connections (also those to the service), removes the token file and the port file and exits with 0.

The start file is one JSON object. Required: `dienst` (an object with exactly `socket`, an absolute path of at most 100 bytes), `port` (0 to 65535, 0 lets the system choose), `token_datei` (an absolute path), `deflate` (a boolean: the value of `deflate` of the service, sent as `X-Deflate`). Optional: `port_datei`, `pruefmodus` (default false), and, only with `pruefmodus` true, `lese_frist_ms` (100 to 10000) and `verbindungen_max` (1 to 64). Any other key is invalid, `adresse` and `host` included.

Exit codes at the start: 2 call or start file invalid, 3 rights of the token file or the port file, 1 the bind fails or `/dev/urandom` cannot be read. Each failure writes exactly one line `wb-bruecke: <text>` to stderr and nothing to stdout. After the files are written, stdout gets exactly one line `PORT <n>`.

## Routes

| Method and target | Token | Effect |
| --- | --- | --- |
| `GET /` | no | the page of the view (`rust/ansicht/ausgabe/index.html`, built in with `include_bytes!`) |
| `GET /ansicht.js` | no | the script of the view (`rust/ansicht/ausgabe/ansicht.js`, built in, `text/javascript`) |
| `GET /ansicht_bg.wasm` | no | the module of the view (`rust/ansicht/ausgabe/ansicht_bg.wasm`, built in, `application/wasm`) |
| `GET /strom` | yes | the event stream: every byte of the service, unchanged; the heads `X-Verbindung` and `X-Deflate` |
| `POST /starten` | yes | one short connection to the service; 200 `{"sitzung": ...}`, 422 with `code`, `grund` and `text` of the service |
| `POST /eingabe` | yes | the body goes unchanged as an event frame to the connection named by `X-Verbindung`; 204 |

Every other method or target needs the token first and then answers 405 or 404. No answer carries a head that begins `Access-Control-`.

## Layout

| Module | Task |
| --- | --- |
| `startdatei` | reads and checks the start file (BR1) |
| `rechte` | the folder of the token file and the port file, exclusive creation with the mode 0600 (BR1) |
| `token` | randomness, hex form, comparison in constant time with a counter of the positions read (BR2) |
| `form` | the form of a request, sizes 414 and 431 (BR3, BR5) |
| `pruefung` | the order of the checks in one place: form, `Host`, `Origin`, token, method and route; for a POST also type, length, size (BR3) |
| `verbindung` | one connection: read head and body within the time limit, route, stream, input (BR4, BR5) |
| `dienst` | the frames of the service and the start of a session |
| `server` | bind, files, accept loop, end |
| `programm` | the program: arguments, port line, signals |

One thread per connection (at most 64 at once, the next gets 503 and no thread). A stream uses a second thread that watches the browser side. A broken request ends only its own connection.

## Tests

`cargo test -p bruecke` runs the unit tests (including the nine pairs of the comparison, a fixed random stream against the form check, the order of the checks, the body limits at 16 384, 16 385, 1 048 575 and 1 048 576 bytes, the time limit with a short time and the fixed address), the program tests (`tests/programm.rs`) and the visible outside cases of the contract (`tests/aussen.rs`). The outside cases need `python3` and `lsof`; `tests/aussen/` holds copies of the check client, the runner, the Python reference of the service (the other side) and the visible cases (see `tests/aussen/README.md`). The measurement with 64 connections is not a test; it runs alone on request (see `rust/README.md`, section `bruecke`).
