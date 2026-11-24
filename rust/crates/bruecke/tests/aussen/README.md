# Outside cases of the contract `bruecke`

Copies from the contract of the bridge (state 21.11.2026), used by `tests/aussen.rs`. The copies of `pruefclient.py` and the three references differ from the contract folder only in their comments: they name the contract text instead of its file name, and decisions by their date. The sha256 column holds the sums of the files in this folder.

| File | What | sha256 |
| --- | --- | --- |
| `pruefclient.py` | the check client, with the addendum of 21.11.2026 (`ohne_adresse`: a namespace of XML is no address; `csp_wasm`); equal to the contract folder except for the comments named above | `624e6564a060bef5d41fedf00e5bd3eaf31a70b8aa9f404a0631e6bca2bba16b` |
| `test_aussen.py` | the outside runner | `354a0999063e4ad735b401bee571352791f7371456a63aee2e02ed91153a63bd` |
| `referenz/dienst_ref.py` | Python reference of the service `dienst` (the other side of every case), with the addendum DI4 of 21.11.2026 (carrier events of a new size after the messages of the step) and P25 of 23.11.2026 (identifier and raster number separate, list field `nr`); equal to the contract folder except for the comments named above | `fda8d2a3747e58b20bce4f62f6c5b1b7bd7d8e35cdc4dc77d75ea66a16660b53` |
| `referenz/ra_ref.py` | Python reference of `rasterauszug` (decoder of the check client) | `292a03726df713d62fcf1f40102ebffa62b7082084c60aaf66410b9a27543a16` |
| `referenz/zk_ref.py` | Python reference of `zustandskern` (used by the service reference) | `8fe2b33794241724ad9fbf0b7726795714467782f1710fe643eea67d3e704a58` |
| `einstellungen/vorfuehrung.json` | settings file read by the service reference | `3612ca0afaab8e5c482ba0f5d94965dccfef7b6f162008da8a05a8f73f40db54` |
| `aussentests/faelle/`, `aussentests/erwartet/` | the 62 visible cases (set A) and their expectations, rebuilt with the addendum of 21.11.2026 (routes `/ansicht.js` and `/ansicht_bg.wasm`, no `/seite.js`) | |

These are test tools, not part of the program. The hidden set is not here. The expectations are the contract's; do not change them. When the contract of `dienst`, `rasterauszug` or `zustandskern` changes, renew the copies from the contract folder.
