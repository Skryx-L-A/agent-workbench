//! The start of a worker by its orchestrator (contract `dienst`, Nachtrag 28.11.2026 "Worker-Start durch den Orchestrator (Stufe 2)").
//!
//! A valid request of the run of an orchestrator (`meldung.rs`, after `laeuft`) goes to the authority as `worker.anfordern`; after
//! `frei` the service builds the start structure of the worker from the one of the orchestrator and starts it through the whole way
//! of `Starten` (DI12 point 3). Every start with `lauf.stelle` has two steps: after the checks and the folder the service asks the
//! authority `sitzung.starten` with the bytes of the start file, and `lauf` starts only after the answer.
//!
//! SECURITY, fail closed: the orchestrator is the session of the run the line came through, never a value of the line; a failure of
//! the authority is never `frei` and starts nothing; `frei` skips no check of `Starten`; a start never runs without the answer of the
//! authority. Who may request and what a worker may inherit is decided by the authority (its rules); this module adds no rule.

#[cfg(unix)]
use std::os::fd::AsFd;
use std::path::PathBuf;

use serde_json::{Value, json};
use start::Startanfrage;

use crate::dienst::Dienst;
use crate::starten::{Abgelehnt, meta_von, startdatei_inhalt, text_zu};
use crate::stelle::{Anforderung, Auftrag, Fehlschlag, Fuer};

/// Who gets the answer of a start.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Ziel {
    /// A viewer: `Gestartet` or `Ablehnung` (DI12 point 3).
    Betrachter(u64),
    /// The run of the orchestrator with this identifier: `frei <sitzung>` or `abgelehnt <code>` on its report path.
    Meldung(String),
}

/// A start between `sitzung.starten` and the answer of the authority: the folder and the start file exist, the identifier is taken,
/// `lauf` has not started.
#[derive(Debug)]
pub(crate) struct Wartestart {
    pub(crate) ziel: Ziel,
    pub(crate) n: usize,
    pub(crate) kennung: String,
    pub(crate) ordner: PathBuf,
    pub(crate) datei: PathBuf,
    pub(crate) anfrage: Startanfrage,
    pub(crate) start: Value,
    pub(crate) worker: Option<(String, String)>,
    pub(crate) alt: Option<String>,
}

/// The reason of a refusal by the authority.
pub(crate) const STELLE_ABGELEHNT: &str = crate::stelle::ABGELEHNT;
/// The longest code of an answer line to the run (the form of the report, `[a-z0-9_.]` with 1 to 64 characters).
const CODE_MAX: usize = 64;

/// The answer line `abgelehnt <code>` to the run: the code of the refusal (the code of the stage of the crate `start`, of the
/// authority, or the reason of the service). A code out of the form of the report becomes `stelle_antwort_ungueltig`.
pub(crate) fn abgelehnt_zeile(code: &str) -> Vec<u8> {
    let gut = (1..=CODE_MAX).contains(&code.len())
        && code
            .bytes()
            .all(|b| matches!(b, b'a'..=b'z' | b'0'..=b'9' | b'_' | b'.'));
    let code = if gut {
        code
    } else {
        crate::stelle::ANTWORT_UNGUELTIG
    };
    format!("abgelehnt {code}\n").into_bytes()
}

/// The refusal of a start for a failure of the authority: `stelle_abgelehnt` with the code of the authority, or the reason of the
/// service (`stelle_nicht_erreichbar`, `stelle_antwort_ungueltig`).
pub(crate) fn abgelehnt_von(f: Fehlschlag) -> Abgelehnt {
    match f {
        Fehlschlag::Stelle(code) => Abgelehnt {
            grund: STELLE_ABGELEHNT.to_owned(),
            code,
            text: text_zu(STELLE_ABGELEHNT).to_owned(),
        },
        Fehlschlag::Dienst(grund) => Abgelehnt::fest(grund),
    }
}

/// `route` and `kunde` of the start structure of an orchestrator: a text, and a text or `null`.
fn route_und_kunde(start: &Value) -> Option<(String, Option<String>)> {
    let route = start.get("route")?.as_str()?.to_owned();
    let kunde = match start.get("kunde")? {
        Value::Null => None,
        Value::String(k) => Some(k.clone()),
        _ => return None,
    };
    Some((route, kunde))
}

/// The start structure of the worker (stage 2, point 2): `route`, `kunde` and `arbeitsordner` of the orchestrator, `harness`,
/// `modell` and `effort` of the line, fassung 2 without a name. Nothing else of the orchestrator goes over.
pub(crate) fn worker_start(orchestrator: &Value, fuer: &Fuer, a: &Anforderung) -> Option<Value> {
    let ordner = orchestrator.get("arbeitsordner")?.as_str()?;
    Some(
        json!({"fassung": 2, "harness": a.harness, "modell": a.modell, "route": fuer.route, "kunde": fuer.kunde,
                "arbeitsordner": ordner, "effort": a.effort, "name": null}),
    )
}

impl Dienst {
    /// The refusal of a start to whom it belongs.
    pub(crate) fn ziel_ablehnen(&mut self, ziel: &Ziel, a: &Abgelehnt) {
        match ziel {
            Ziel::Betrachter(nr) => self.start_ablehnen(*nr, a),
            Ziel::Meldung(orchestrator) => {
                self.meldung_antworten(orchestrator, &abgelehnt_zeile(&a.code));
            }
        }
    }

    /// The answer to the open request of the run of `sitzung`; dropped when the session has no report path any more (its end).
    pub(crate) fn meldung_antworten(&mut self, sitzung: &str, zeile: &[u8]) {
        if let Some(m) = self
            .finde(sitzung)
            .and_then(|k| self.sitzungen[k].meldung.as_mut())
        {
            m.antworten(zeile);
        }
    }

    /// The running workers of the orchestrator `sitzung`, with the starts of its workers that wait for the authority.
    fn laufende_worker(&self, sitzung: &str) -> u64 {
        let laufend = self
            .sitzungen
            .iter()
            .filter(|s| s.angaben.orchestrator.as_deref() == Some(sitzung) && !s.ist_beendet())
            .count();
        let wartend = self
            .wartend
            .values()
            .filter(|w| w.worker.as_ref().is_some_and(|(o, _)| o == sitzung))
            .count();
        u64::try_from(laufend + wartend).unwrap_or(u64::MAX)
    }

    /// What `worker.anfordern` names for the orchestrator `sitzung`: the session itself, its running workers, `route` and `kunde`.
    fn fuer(&self, sitzung: &str) -> Option<Fuer> {
        let k = self.finde(sitzung)?;
        let (route, kunde) = route_und_kunde(self.sitzungen[k].angaben.start.as_ref()?)?;
        Some(Fuer {
            sitzung: sitzung.to_owned(),
            laufend: self.laufende_worker(sitzung),
            route,
            kunde,
        })
    }

    /// The valid requests that came on the report paths in this turn: each goes to the thread of the authority (the loop never
    /// waits). Without the thread the answer is `stelle_nicht_erreichbar`, as in stage 1.
    pub(crate) fn anforderungen_abholen(&mut self) {
        let neu: Vec<(String, Anforderung)> = self
            .sitzungen
            .iter_mut()
            .filter_map(|s| {
                let sid = s.sid.clone()?;
                Some((sid, s.meldung.as_mut()?.anforderung_nehmen()?))
            })
            .collect();
        for (sitzung, anforderung) in neu {
            let Some(fuer) = self.fuer(&sitzung) else {
                self.meldung_antworten(&sitzung, &abgelehnt_zeile("start_form"));
                continue;
            };
            let gesendet = self
                .stelle
                .as_ref()
                .is_some_and(|faden| faden.auftrag(Auftrag::Anfordern { fuer, anforderung }));
            if !gesendet {
                self.meldung_antworten(&sitzung, crate::meldung::ANTWORT_STELLE);
            }
        }
    }

    /// The answer of the authority to `worker.anfordern`. The orchestrator has ended or lost its report path: nothing starts (stage
    /// 2, point 4). A failure goes to the run as `abgelehnt <code>`; `frei` starts the worker through the whole way of `Starten`.
    pub(crate) fn stelle_angefordert(
        &mut self,
        fuer: &Fuer,
        anforderung: &Anforderung,
        ergebnis: Result<String, Fehlschlag>,
    ) {
        // an orchestrator that has ended has no report path any more, so no answer reaches it; `Starten` refuses its worker with
        // `orchestrator_beendet` (DI-L1 point 2) before anything is made
        let Some(k) = self.finde(&fuer.sitzung) else {
            return;
        };
        let o = &self.sitzungen[k];
        let zusage = match ergebnis {
            Ok(z) => z,
            Err(f) => {
                let zeile = crate::stelle::meldeantwort(&Err(f), "");
                return self.meldung_antworten(&fuer.sitzung, &zeile);
            }
        };
        let Some(start) = o
            .angaben
            .start
            .as_ref()
            .and_then(|s| worker_start(s, fuer, anforderung))
        else {
            return self.meldung_antworten(&fuer.sitzung, &abgelehnt_zeile("start_form"));
        };
        let worker = Some((fuer.sitzung.clone(), anforderung.rolle.clone()));
        self.starten_fuer(
            Ziel::Meldung(fuer.sitzung.clone()),
            &start,
            Ok(worker),
            None,
            Some(&zusage),
        );
    }

    /// The first step of a start with `lauf.stelle`: `sitzung.starten` with the bytes of the start file, with a worker also
    /// `orchestrator` and `zusage`. The identifier stays taken whatever comes; the start waits and counts as a running session.
    pub(crate) fn stelle_fragen(&mut self, w: Wartestart, zusage: Option<&str>) {
        self.naechste = self.naechste.max(w.n);
        let orchestrator = w.worker.as_ref().map(|(o, _)| o.as_str());
        let eingabe = startdatei_inhalt(
            &w.anfrage,
            &w.kennung,
            self.marker.as_ref(),
            w.alt.as_deref(),
        )
        .and_then(|b| crate::stelle::starten_eingabe(&b, orchestrator, zusage));
        let gesendet = eingabe.is_some_and(|eingabe| {
            self.stelle.as_ref().is_some_and(|faden| {
                faden.auftrag(Auftrag::Starten {
                    sitzung: w.kennung.clone(),
                    eingabe,
                })
            })
        });
        if !gesendet {
            crate::loeschen::sitzung_entfernen(self.wurzel.as_fd(), &w.kennung);
            return self.ziel_ablehnen(&w.ziel, &Abgelehnt::fest(crate::stelle::NICHT_ERREICHBAR));
        }
        self.wartend.insert(w.kennung.clone(), w);
    }

    /// The answer of the authority to `sitzung.starten`. A refusal or a failure removes the folder of the session (DI12 point 12,
    /// through the descriptors) and refuses the start; the identifier stays taken. Then the checks that may have changed while the
    /// start waited (the orchestrator, a continued session, the raster number), then `lauf`.
    pub(crate) fn stelle_gestartet(&mut self, kennung: &str, ergebnis: Result<(), Fehlschlag>) {
        let Some(w) = self.wartend.remove(kennung) else {
            return;
        };
        let rahmen_max = self.cfg.lauf.as_ref().map_or(0, |l| l.rahmen_max);
        let fehler = match ergebnis {
            Err(f) => Some(abgelehnt_von(f)),
            Ok(()) => self.noch_startbar(&w).err(),
        };
        let nummer = match fehler {
            Some(a) => Err(a),
            None => self
                .rahmen_waehlen(w.n, rahmen_max)
                .ok_or_else(|| Abgelehnt::fest("nummern_erschoepft")),
        };
        match nummer {
            Ok(nummer) => self.start_abschliessen(w, nummer),
            Err(a) => {
                crate::loeschen::sitzung_entfernen(self.wurzel.as_fd(), &w.kennung);
                self.ziel_ablehnen(&w.ziel, &a);
            }
        }
    }

    /// The orchestrator of a worker still runs (DI-L1 point 2, stage 2 point 4), and the session a start continues is not continued
    /// by another start that came first (DI-L3 point 2).
    fn noch_startbar(&self, w: &Wartestart) -> Result<(), Abgelehnt> {
        if let Some((orchestrator, _)) = &w.worker {
            self.orchestrator_pruefen(orchestrator, &meta_von(&w.anfrage)[3])?;
        }
        let fortgesetzt = w
            .alt
            .as_deref()
            .and_then(|a| self.finde(a))
            .is_some_and(|v| self.sitzungen[v].angaben.fortgesetzt_als.is_some());
        if fortgesetzt {
            return Err(Abgelehnt::fest("schon_fortgesetzt"));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn a() -> Anforderung {
        Anforderung {
            rolle: "pruefer".to_owned(),
            harness: "pi".to_owned(),
            modell: "qwen3:8b".to_owned(),
            effort: "low".to_owned(),
        }
    }

    #[test]
    fn der_worker_erbt_nur_route_kunde_und_ordner() {
        let orchestrator = json!({"fassung": 2, "harness": "claude", "modell": "m", "route": "cloud-beispiel", "kunde": "k-beispiel",
                                  "arbeitsordner": "/tmp/p", "effort": "high", "name": "Haupt"});
        let (route, kunde) = route_und_kunde(&orchestrator).expect("route");
        let fuer = Fuer {
            sitzung: "s-1".to_owned(),
            laufend: 0,
            route,
            kunde,
        };
        assert_eq!(
            worker_start(&orchestrator, &fuer, &a()),
            Some(
                json!({"fassung": 2, "harness": "pi", "modell": "qwen3:8b", "route": "cloud-beispiel", "kunde": "k-beispiel",
                        "arbeitsordner": "/tmp/p", "effort": "low", "name": null})
            )
        );
        let ohne_kunde = json!({"route": "r", "kunde": null, "arbeitsordner": "/x"});
        assert_eq!(route_und_kunde(&ohne_kunde), Some(("r".to_owned(), None)));
        for falsch in [
            json!({"kunde": null, "arbeitsordner": "/x"}),
            json!({"route": "r", "arbeitsordner": "/x"}),
            json!({"route": "r", "kunde": 1}),
            json!({"route": 1, "kunde": null}),
        ] {
            assert_eq!(route_und_kunde(&falsch), None, "{falsch}");
        }
        assert_eq!(worker_start(&json!({"route": "r"}), &fuer, &a()), None);
    }

    #[test]
    fn die_antwortzeile_traegt_den_code_in_der_form_der_meldung() {
        assert_eq!(
            abgelehnt_zeile("start.modell_route"),
            b"abgelehnt start.modell_route\n"
        );
        assert_eq!(
            abgelehnt_zeile("stelle.zusage_ungueltig"),
            b"abgelehnt stelle.zusage_ungueltig\n"
        );
        let lang = format!("stelle.{}", "a".repeat(58));
        assert_eq!(lang.len(), 65);
        assert_eq!(
            abgelehnt_zeile(&lang),
            b"abgelehnt stelle_antwort_ungueltig\n"
        );
        assert_eq!(
            abgelehnt_zeile(&lang[..64]),
            format!("abgelehnt {}\n", &lang[..64]).into_bytes()
        );
        for falsch in ["", "Stelle.x", "a b", "a-b"] {
            assert_eq!(
                abgelehnt_zeile(falsch),
                b"abgelehnt stelle_antwort_ungueltig\n",
                "{falsch}"
            );
        }
    }

    #[test]
    fn eine_ablehnung_der_stelle_traegt_ihren_code() {
        let a = abgelehnt_von(Fehlschlag::Stelle("stelle.zusage_ungueltig".to_owned()));
        assert_eq!(
            (a.grund.as_str(), a.code.as_str(), a.text.as_str()),
            (
                "stelle_abgelehnt",
                "stelle.zusage_ungueltig",
                "Stelle lehnt ab"
            )
        );
        let b = abgelehnt_von(Fehlschlag::Dienst(crate::stelle::NICHT_ERREICHBAR));
        assert_eq!(
            (b.grund.as_str(), b.code.as_str(), b.text.as_str()),
            (
                crate::stelle::NICHT_ERREICHBAR,
                crate::stelle::NICHT_ERREICHBAR,
                "Stelle nicht erreichbar"
            )
        );
        let c = abgelehnt_von(Fehlschlag::Dienst(crate::stelle::ANTWORT_UNGUELTIG));
        assert_eq!(c.text, "Antwort der Stelle ungueltig");
    }
}
