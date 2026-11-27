//! The sessions as the page manages them (contract, "Nachtrag 26.11.2026: Oberfläche L1, L2, L3, L5, L6, L11"): workers with their
//! orchestrator (DI-L1), `Umbenennen`, `Fortsetzen`, `Wiederbeleben` and `Loeschen` (DI-L3), the new fields of the `Sitzungsliste`
//! and the kind of the end (DI-L5). The events are the service's own, like `Starten` and `Beenden`: the core never sees them.

use serde_json::{Map, Value, json};

use crate::dienst::{Dienst, sendbar};
use crate::sitzung::Sitzung;
use crate::starten::Abgelehnt;

/// The most characters of a name (DI-L3 point 1).
pub(crate) const NAME_MAX: usize = 64;
/// The most characters of an identifier of `orchestrator` and `rolle` (DI-L1 point 1, form of contract 2).
const KENNUNG_MAX: usize = 64;
/// The role of a worker when `Starten` names none (DI-L1 point 1).
pub(crate) const ROLLE_VORGABE: &str = "worker";

/// How a session ended (DI-L5 point 2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EndeArt {
    /// By the event `Beenden`.
    Beenden,
    /// By itself.
    Selbst,
    /// By the end of the service (SIGTERM, SIGINT); only in the Bestand.
    Dienstende,
    /// It still ran when the last run of the service ended without an orderly end (read from the Bestand).
    Neustart,
}

/// What the page knows of a session beyond the core: name, start structure, orchestrator, continuation and the kind of the end.
#[derive(Debug, Default)]
pub(crate) struct Angaben {
    /// The name of `Umbenennen` (DI-L3 point 1), `""` before.
    pub(crate) name: String,
    /// `arbeitsordner` of the start structure, `""` for the session of `kind` (DI-L5 point 1).
    pub(crate) ordner: String,
    /// The start structure of the request, as it came (DI-L3 point 2, DI-L5 point 3); `None` for the session of `kind`.
    pub(crate) start: Option<Value>,
    /// The session of the orchestrator of a worker (DI-L1).
    pub(crate) orchestrator: Option<String>,
    /// The role of a worker (DI-L1).
    pub(crate) rolle: Option<String>,
    /// The session this one continues (DI-L3 point 2).
    pub(crate) fortsetzung_von: Option<String>,
    /// The session that continues this one (DI-L3 point 2).
    pub(crate) fortgesetzt_als: Option<String>,
    /// How it ended; `None` while it runs.
    pub(crate) ende: Option<EndeArt>,
    /// `Beenden` asked the child to end.
    pub(crate) beendet_von_uns: bool,
    /// The end has no code (an entry of the Bestand that still ran, DI-L5 point 4).
    pub(crate) ohne_code: bool,
}

/// `true` for a text in the form of an identifier of contract 2: `^[a-z0-9][a-z0-9._-]{0,63}$`.
pub(crate) fn ist_kennung(t: &str) -> bool {
    let b = t.as_bytes();
    let erstes = |c: u8| c.is_ascii_lowercase() || c.is_ascii_digit();
    let weitere = |c: u8| erstes(c) || matches!(c, b'.' | b'_' | b'-');
    !b.is_empty() && b.len() <= KENNUNG_MAX && erstes(b[0]) && b[1..].iter().all(|&c| weitere(c))
}

/// DI-L3 point 1: 1 to 64 Unicode scalar values, no character below 0x20 or from 0x7f to 0x9f, no white space at the start or the end.
pub(crate) fn name_gueltig(name: &str) -> bool {
    let steuer = |c: char| c < '\u{20}' || ('\u{7f}'..='\u{9f}').contains(&c);
    let zahl = name.chars().count();
    (1..=NAME_MAX).contains(&zahl) && !name.chars().any(steuer) && name.trim() == name
}

/// DI-L5 point 1: the display name is the last part of the working folder; `""` for `""` and `/`.
pub(crate) fn projekt_von(ordner: &str) -> &str {
    let ohne = ordner.trim_end_matches('/');
    ohne.rsplit('/').next().unwrap_or("")
}

/// The keys `orchestrator` and `rolle` of `Starten` (DI-L1 points 1 and 2): `None` without them; the role defaults to `worker`.
///
/// # Errors
/// `start_form`: `orchestrator` is no identifier, or `rolle` stands without `orchestrator` or is no identifier.
pub(crate) fn worker_form(
    ereignis: &Map<String, Value>,
) -> Result<Option<(String, String)>, Abgelehnt> {
    if !ereignis.contains_key("orchestrator") && !ereignis.contains_key("rolle") {
        return Ok(None);
    }
    let kennung = |k: &str| {
        ereignis
            .get(k)
            .and_then(Value::as_str)
            .filter(|t| ist_kennung(t))
    };
    let Some(orchestrator) = kennung("orchestrator") else {
        return Err(Abgelehnt::fest("start_form"));
    };
    let rolle = match ereignis.get("rolle") {
        None => ROLLE_VORGABE,
        Some(_) => kennung("rolle").ok_or_else(|| Abgelehnt::fest("start_form"))?,
    };
    Ok(Some((orchestrator.to_owned(), rolle.to_owned())))
}

impl Sitzung {
    /// `beendet` for the list: the child has ended, or the service ended it at its own end (DI-L5 point 2).
    pub(crate) fn ist_beendet(&self) -> bool {
        self.kind_ende.is_some() || self.angaben.ende.is_some()
    }

    /// The code of the end for the list, the workers and the Bestand; none for an entry of the Bestand that still ran.
    pub(crate) fn ende_code(&self) -> Option<u32> {
        self.kind_ende.filter(|_| !self.angaben.ohne_code)
    }

    /// DI-L5 point 2: it ended by itself with a code other than 0, or it still ran at the end of the last run of the service.
    pub(crate) fn abgestuerzt(&self) -> bool {
        match self.angaben.ende {
            Some(EndeArt::Neustart) => true,
            Some(EndeArt::Selbst) => self.ende_code().is_some_and(|c| c != 0),
            _ => false,
        }
    }

    /// The fields of DI-L5 point 1 in an entry of the list.
    pub(crate) fn listenfelder(&self, x: &mut Value) {
        let a = &self.angaben;
        x["name"] = json!(a.name);
        x["ordner"] = json!(a.ordner);
        x["projekt"] = json!(projekt_von(&a.ordner));
        if self.ist_beendet() {
            x["abgestuerzt"] = json!(self.abgestuerzt());
        }
        if let Some(v) = &a.fortsetzung_von {
            x["fortsetzung_von"] = json!(v);
        }
        if let Some(v) = &a.fortgesetzt_als {
            x["fortgesetzt_als"] = json!(v);
        }
    }
}

impl Dienst {
    /// DI-L1 point 3: the workers in the order of the start.
    pub(crate) fn agenten_liste(&self) -> Vec<Value> {
        self.sitzungen
            .iter()
            .filter(|s| !s.sid.as_ref().is_some_and(|sid| self.entfernt.contains(sid)))
            .filter_map(|s| {
                let sid = s.sid.as_ref()?;
                let orchestrator = s.angaben.orchestrator.as_ref()?;
                let mut x = json!({"worker": sid, "sitzung": orchestrator, "rolle": s.angaben.rolle, "modell": s.meta[1],
                                   "zustand": if s.ist_beendet() { "beendet" } else { "laeuft" }, "nr": s.nr});
                if let Some(code) = s.ende_code() {
                    x["code"] = json!(code);
                }
                Some(x)
            })
            .collect()
    }

    /// The message `Agenten` of the service (DI-L1 point 3).
    pub(crate) fn agenten_nachricht(&mut self) -> Value {
        let agenten = self.agenten_liste();
        json!({"t": self.jetzt(), "art": "Agenten", "agenten": agenten})
    }

    /// After a change of the list: the list to everyone, then `Agenten` when the list of the workers changed (DI-L1 point 4), then the
    /// Bestand (DI-L5 point 3).
    pub(crate) fn liste_und_agenten(&mut self) {
        self.liste_senden();
        let neu = self.agenten_liste();
        if neu != self.agenten_letzt {
            self.agenten_letzt = neu;
            let nummern: Vec<u64> = self.betrachter.keys().copied().collect();
            for nr in nummern {
                if sendbar(self.betrachter.get(&nr)) {
                    let nachricht = self.agenten_nachricht();
                    self.nachricht_an(nr, &nachricht, None);
                }
            }
        }
        self.bestand_schreiben();
    }

    /// `Umbenennen` (DI-L3 point 1): running and ended sessions, the session of `kind` too.
    pub(crate) fn umbenennen(&mut self, nr: u64, ereignis: &Map<String, Value>) {
        let form = ereignis.len() == 3 && ereignis.contains_key("name");
        let Some(sid) = ereignis
            .get("sitzung")
            .and_then(Value::as_str)
            .filter(|_| form)
        else {
            return self.ablehnen(nr, &json!({"grund": "umbenennen_form"}));
        };
        let Some(k) = self.finde(sid) else {
            return self.ablehnen(nr, &json!({"grund": "sitzung_unbekannt"}));
        };
        let Some(name) = ereignis
            .get("name")
            .and_then(Value::as_str)
            .filter(|n| name_gueltig(n))
        else {
            return self.ablehnen(nr, &json!({"grund": "name_ungueltig"}));
        };
        name.clone_into(&mut self.sitzungen[k].angaben.name);
        self.liste_und_agenten();
    }

    /// `Fortsetzen` and `Wiederbeleben` (DI-L3 points 2 and 3): the way of `Starten` with the stored start structure and with the
    /// orchestrator and the role of the old session; a new identifier, a new folder, a new child.
    pub(crate) fn fortsetzen(&mut self, nr: u64, ereignis: &Map<String, Value>, wieder: bool) {
        let form = if wieder {
            "wiederbeleben_form"
        } else {
            "fortsetzen_form"
        };
        let Some(sid) = ereignis
            .get("sitzung")
            .and_then(Value::as_str)
            .filter(|_| ereignis.len() == 2)
        else {
            return self.start_ablehnen(nr, &Abgelehnt::fest(form));
        };
        let Some(k) = self.finde(sid) else {
            return self.start_ablehnen(nr, &Abgelehnt::fest("sitzung_unbekannt"));
        };
        let s = &self.sitzungen[k];
        let grund = if !s.ist_beendet() {
            Some("sitzung_laeuft")
        } else if wieder && !s.abgestuerzt() {
            Some("nicht_abgestuerzt")
        } else if s.angaben.fortgesetzt_als.is_some() {
            Some("schon_fortgesetzt")
        } else {
            None
        };
        // The start file of a continued session needs `fortsetzung_von`, which only the second version of the crate `start` writes. Until
        // then every session is `nicht_fortsetzbar` after the checks above; no start file is written by hand (fail closed).
        self.start_ablehnen(nr, &Abgelehnt::fest(grund.unwrap_or("nicht_fortsetzbar")));
    }

    /// `Loeschen` (DI-L3 points 4 and 5): only an ended session of `Starten`, with its identifier as the confirmation, never one that
    /// leads a running worker. It takes the entry out of the list and the Bestand with the ended workers of the session, the core and
    /// the carrier, and every subscription; the raster number is free. No file goes: the folder of the session went at its end (DI12
    /// point 12), and nothing outside the folder of the service is touched. The identifier is never handed out again.
    pub(crate) fn loeschen(&mut self, nr: u64, ereignis: &Map<String, Value>) {
        let form = ereignis
            .keys()
            .all(|k| matches!(k.as_str(), "art" | "sitzung" | "bestaetigung"));
        let Some(sid) = ereignis
            .get("sitzung")
            .and_then(Value::as_str)
            .filter(|_| form)
        else {
            return self.ablehnen(nr, &json!({"grund": "loeschen_form"}));
        };
        let Some(k) = self.finde(sid) else {
            return self.ablehnen(nr, &json!({"grund": "sitzung_unbekannt"}));
        };
        let s = &self.sitzungen[k];
        let grund = if s.eintrag.is_none() {
            Some("nicht_loeschbar")
        } else if !s.ist_beendet() {
            Some("sitzung_laeuft")
        } else if ereignis.get("bestaetigung").and_then(Value::as_str) != Some(sid) {
            Some("bestaetigung_fehlt")
        } else if self
            .sitzungen
            .iter()
            .any(|w| w.angaben.orchestrator.as_deref() == Some(sid) && !w.ist_beendet())
        {
            Some("worker_laeuft")
        } else {
            None
        };
        if let Some(g) = grund {
            return self.ablehnen(nr, &json!({"grund": g}));
        }
        let weg: Vec<String> = self
            .sitzungen
            .iter()
            .filter(|w| {
                w.sid.as_deref() == Some(sid) || w.angaben.orchestrator.as_deref() == Some(sid)
            })
            .filter_map(|w| w.sid.clone())
            .collect();
        // DI-E4 points 3 to 5 (Nachtrag 27.11.2026): first the conversations, then the entries; a session with `fortgesetzt_als` has no
        // conversation of its own any more. An error leaves list, Bestand, cores and subscriptions as they are.
        if let Some(wurzel) = self.cfg.lauf.as_ref().and_then(|l| l.gespraeche.clone()) {
            let namen: Vec<String> = self
                .sitzungen
                .iter()
                .filter(|w| w.angaben.fortgesetzt_als.is_none())
                .filter_map(|w| w.sid.clone())
                .filter(|w| weg.contains(w))
                .collect();
            if let Err(grund) = crate::gespraeche::entfernen(&wurzel, &namen) {
                use std::io::Write;
                let _ = writeln!(
                    std::io::stderr(),
                    "wb-dienst: gespraech nicht geloescht: {grund}"
                );
                return self.ablehnen(nr, &json!({"grund": "gespraech_nicht_geloescht"}));
            }
        }
        for w in &weg {
            self.sitzung_wegnehmen(w);
        }
        self.liste_und_agenten();
    }

    /// Takes one ended session out of the service (DI-L3 point 5): every subscription on it ends, every connection forgets its raster
    /// number, the number is free (after the numbers never handed out, in the order of the ends), the core and the carrier go.
    /// The loop handles one event at a time: no step waits now, so the indexes of the other sessions may change.
    fn sitzung_wegnehmen(&mut self, sid: &str) {
        let Some(k) = self.finde(sid) else {
            return;
        };
        let s = &self.sitzungen[k];
        if let (Some(nummer), Some(ende)) = (s.nr, s.beendet_als) {
            self.frei_geloescht.push((ende, nummer));
        }
        let nummer = s.nr;
        for b in self.betrachter.values_mut() {
            b.abos.remove(sid);
            if let Some(n) = nummer {
                b.raster.vergessen(n);
            }
        }
        self.entfernt.remove(sid);
        // `Kind::drop`: the child has ended and was collected; the carrier was given back at `Kindende`
        self.sitzungen.remove(k);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eine_kennung_hat_die_form_von_vertrag_2() {
        for gut in ["s-1", "a", "0", "a.b_c-d", &"a".repeat(64)] {
            assert!(ist_kennung(gut), "{gut}");
        }
        for schlecht in [
            "",
            "A",
            "-a",
            ".a",
            "_a",
            "a b",
            "aä",
            "a/b",
            &"a".repeat(65),
            "S-1",
        ] {
            assert!(!ist_kennung(schlecht), "{schlecht}");
        }
    }

    #[test]
    fn ein_name_hat_1_bis_64_zeichen_ohne_steuerzeichen_und_rand() {
        for gut in [
            "x",
            "Plan ä",
            "a b",
            &"ä".repeat(64),
            "\u{a0}x".trim_start(),
        ] {
            assert!(name_gueltig(gut), "{gut:?}");
        }
        for schlecht in [
            "",
            " x",
            "x ",
            "\tx",
            "x\n",
            "a\u{1f}b",
            "a\u{7f}b",
            "a\u{80}b",
            "a\u{9f}b",
            &"ä".repeat(65),
        ] {
            assert!(!name_gueltig(schlecht), "{schlecht:?}");
        }
        assert!(name_gueltig("a\u{a0}b"), "0xa0 is no control character");
        assert!(name_gueltig("a\u{1f}b".replace('\u{1f}', " ").as_str()));
    }

    #[test]
    fn das_projekt_ist_der_letzte_teil_des_ordners() {
        assert_eq!(projekt_von("/home/a/kunde-x"), "kunde-x");
        assert_eq!(projekt_von("/home/a/kunde-x/"), "kunde-x");
        assert_eq!(projekt_von("/"), "");
        assert_eq!(projekt_von(""), "");
        assert_eq!(projekt_von("/x"), "x");
    }

    fn karte(wert: &Value) -> Map<String, Value> {
        wert.as_object().cloned().unwrap_or_default()
    }

    #[test]
    fn orchestrator_und_rolle_eines_workers() {
        let w = |e: Value| worker_form(&karte(&e)).map_err(|a| a.grund);
        assert_eq!(w(json!({"art": "Starten", "start": {}})), Ok(None));
        assert_eq!(
            w(json!({"art": "Starten", "orchestrator": "s-1"})),
            Ok(Some(("s-1".to_owned(), ROLLE_VORGABE.to_owned())))
        );
        assert_eq!(
            w(json!({"art": "Starten", "orchestrator": "s-1", "rolle": "pruefer"})),
            Ok(Some(("s-1".to_owned(), "pruefer".to_owned())))
        );
        for falsch in [
            json!({"rolle": "worker"}),
            json!({"orchestrator": "S-1"}),
            json!({"orchestrator": 1}),
            json!({"orchestrator": null}),
            json!({"orchestrator": "s-1", "rolle": "X"}),
            json!({"orchestrator": "s-1", "rolle": null}),
        ] {
            assert_eq!(w(falsch.clone()), Err("start_form".to_owned()), "{falsch}");
        }
    }
}
