# Outside cases of the contract `bruecke`

Copies from the contract of the bridge (state 21.11.2026), used by `tests/aussen.rs`. The copies of `pruefclient.py` and the three references differ from the contract folder only in their comments: they name the contract text instead of its file name, and decisions by their date. The sha256 column holds the sums of the files in this folder.

| File | What | sha256 |
| --- | --- | --- |
| `pruefclient.py` | the check client, with the addendum of 21.11.2026 (`ohne_adresse`: a namespace of XML is no address; `csp_wasm`) and the addendum of 27.11.2026 (the message `Ereignisliste` that the service sends first on every connection is dropped, as in the check client of `dienst`); equal to the contract folder except for the comments named above and this one hunk | `8aea67df83ef778dae3745732d0854c698e80586054ccb9a770e5ffb1dddb13b` |
| `test_aussen.py` | the outside runner | `354a0999063e4ad735b401bee571352791f7371456a63aee2e02ed91153a63bd` |
| `referenz/dienst_ref.py` | Python reference of the service `dienst` (the other side of every case), with the addendum DI4 of 21.11.2026 (carrier events of a new size after the messages of the step) P25 of 23.11.2026 (identifier and raster number separate, list field `nr`) and DI12 point 15a (old own target list of a number removed when it is handed out, start refused before the identifier loop when every raster number is held), and the addenda of 26.11.2026 and 27.11.2026: DI-L1, DI-L3, DI-L5, DI-L6 and DI-L11, the list of events of a connection (DI-E5), the refusal `entscheidung_fehlt` of a `Freigabe` without `entscheidung` (R-L6 point 1 of `zustandskern`) `opencode` in the harness table of `Startwahl` (decision of 27.11.2026) and the channel to the Stelle (addendum of 27.11.2026: `lauf.stelle` has the form with `erwartet`, and `nachweis` in a `Freigabe` is the error `feld_verboten`); equal to the contract folder except for the comments named above | `76df19776cdd879bd7c5ca3bc6e69793fcba983a57c0c5ccee9365edcc1e34cc` |
| `referenz/ra_ref.py` | Python reference of `rasterauszug` (decoder of the check client) | `292a03726df713d62fcf1f40102ebffa62b7082084c60aaf66410b9a27543a16` |
| `referenz/zk_ref.py` | Python reference of `zustandskern` (used by the service reference) | `8fe2b33794241724ad9fbf0b7726795714467782f1710fe643eea67d3e704a58` |
| `einstellungen/vorfuehrung.json` | settings file read by the service reference | `3612ca0afaab8e5c482ba0f5d94965dccfef7b6f162008da8a05a8f73f40db54` |
| `aussentests/faelle/`, `aussentests/erwartet/` | the 62 visible cases (set A) and their expectations, rebuilt with the addendum of 21.11.2026 (routes `/ansicht.js` and `/ansicht_bg.wasm`, no `/seite.js`) | |

These are test tools, not part of the program. The hidden set is not here. The expectations are the contract's; do not change them. When the contract of `dienst`, `rasterauszug` or `zustandskern` changes, renew the copies from the contract folder.
