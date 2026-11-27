//! The handlers of the events (contract, section 4) and the rules R1 to R14 they carry out.

use std::collections::BTreeMap;

use serde_json::{Map, Value, json};

use crate::base64;
use crate::eingabe;
use crate::kern::{
    Abgelehnt, AgentStand, Anfrage, AnfrageZustand, Betrachter, Cursor, Dialog, Ergebnis, Handle,
    Kern, MAX_GROESSE, MIN_SPALTEN, MIN_ZEILEN, Nachweis, SYNC_MS, Synchron, Zeitgeber,
};
use crate::wert::wahr;

/// The event is malformed.
const UNGUELTIG: Abgelehnt = Abgelehnt("ereignis_ungueltig");

/// A field of an event as text.
fn text<'a>(e: &'a Map<String, Value>, schluessel: &str) -> Option<&'a str> {
    e.get(schluessel).and_then(Value::as_str)
}

/// The kinds of input that go to the PTY.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Eingabe {
    Text,
    Taste,
    Einfuegen,
    Antwort,
}

/// The form of `begruendung` (R-L6 point 2): a text, only with a refusal, 1 to 500 scalar values, no control character.
fn begruendung_pruefen(
    e: &Map<String, Value>,
    ablehnen: bool,
) -> Result<Option<String>, Abgelehnt> {
    let Some(wert) = e.get("begruendung") else {
        return Ok(None);
    };
    let grund = wert.as_str().ok_or(UNGUELTIG)?;
    if !ablehnen {
        return Err(Abgelehnt("begruendung_nur_bei_ablehnen"));
    }
    let steuer = |c: char| c < '\u{20}' || ('\u{7f}'..='\u{9f}').contains(&c);
    if grund.is_empty() || grund.chars().count() > BEGRUENDUNG_MAX || grund.chars().any(steuer) {
        return Err(Abgelehnt("begruendung_ungueltig"));
    }
    Ok(Some(grund.to_owned()))
}

/// The longest reason of a refusal in Unicode scalar values (R-L6 point 2).
const BEGRUENDUNG_MAX: usize = 500;

impl Kern {
    /// Runs one event. A rejection leaves the state as it was, except where a rule names a notice.
    pub(crate) fn ereignis(&mut self, e: &Map<String, Value>) -> Ergebnis {
        match text(e, "art") {
            Some("Verbinden") => self.e_verbinden(e),
            Some("Trennen") => self.e_trennen(e),
            Some("RaumWechsel") => self.e_raumwechsel(e),
            Some("Abo") => self.e_abo(e),
            Some("Groesse") => self.e_groesse(e),
            Some("Text") => self.eingabe(e, Eingabe::Text),
            Some("Taste") => self.eingabe(e, Eingabe::Taste),
            Some("Einfuegen") => self.eingabe(e, Eingabe::Einfuegen),
            Some("Antwort") => self.eingabe(e, Eingabe::Antwort),
            Some("Takt") => Ok(()),
            Some("Socket") => self.e_socket(e),
            Some("Ausgabe") => self.e_ausgabe(e),
            Some("SyncBeginn") => self.e_sync_beginn(e),
            Some("SyncEnde") => self.e_sync_ende(e),
            Some("Modus") => self.e_modus(e),
            Some("Titel") => self.e_titel(e),
            Some("Kindende") => self.e_kindende(e),
            Some("GroessenFolge") => self.e_groessenfolge(e),
            Some("Zwischenablage") => self.e_zwischenablage(e),
            Some("Dialog") => self.e_dialog(e),
            Some("DialogEnde") => self.e_dialog_ende(e),
            Some("Anfrage") => self.e_anfrage(e),
            Some("Nachweis") => self.e_nachweis(e),
            Some("Freigabe") => self.e_freigabe(e),
            Some("AnfrageStand") => self.e_anfrage_stand(e),
            Some("RouteWaehlen") => self.e_route_waehlen(e),
            Some("AgentAbfrage") => self.e_agent_abfrage(e),
            Some("Nachricht") => self.e_nachricht(e),
            Some("EinstellungSetzen") => self.e_einstellung_setzen(e),
            _ => Err(Abgelehnt("ereignis_unbekannt")),
        }
    }

    // ---------- lookup ----------

    /// The connected viewer the event names.
    fn betr(&self, e: &Map<String, Value>) -> Result<String, Abgelehnt> {
        text(e, "betrachter")
            .filter(|b| self.betrachter(b).is_some())
            .map(str::to_owned)
            .ok_or(Abgelehnt("betrachter_unbekannt"))
    }

    /// The session the event names.
    fn sitz(&self, e: &Map<String, Value>) -> Result<String, Abgelehnt> {
        text(e, "sitzung")
            .filter(|s| self.sitzungen.contains_key(*s))
            .map(str::to_owned)
            .ok_or(Abgelehnt("sitzung_unbekannt"))
    }

    /// The reported size of an event: columns from 2 to 1000, rows from 1 to 1000 (R3).
    fn groesse_pruefen(e: &Map<String, Value>) -> Result<(i64, i64), Abgelehnt> {
        let gueltig = |k: &str, von: i64| {
            e.get(k)
                .and_then(Value::as_i64)
                .filter(|n| (von..=MAX_GROESSE).contains(n))
        };
        match (
            gueltig("spalten", MIN_SPALTEN),
            gueltig("zeilen", MIN_ZEILEN),
        ) {
            (Some(spalten), Some(zeilen)) => Ok((spalten, zeilen)),
            _ => Err(Abgelehnt("groesse_ungueltig")),
        }
    }

    // ---------- viewers ----------

    fn e_verbinden(&mut self, e: &Map<String, Value>) -> Ergebnis {
        let (Some(bid), Some(raum)) = (text(e, "betrachter"), text(e, "raum")) else {
            return Err(UNGUELTIG);
        };
        if self.betrachter(bid).is_some() {
            return Err(Abgelehnt("bereits_verbunden"));
        }
        let person = text(e, "person")
            .filter(|p| self.personen.contains_key(*p))
            .ok_or(Abgelehnt("person_unbekannt"))?;
        self.betrachter.push((
            bid.to_owned(),
            Betrachter {
                person: person.to_owned(),
                raum: raum.to_owned(),
                abos: Vec::new(),
                stand: BTreeMap::new(),
                gemeldet: BTreeMap::new(),
                ausstehend: 0,
                verfallen: 0,
                rueck: BTreeMap::new(),
            },
        ));
        let t = self.t;
        let einstellungen =
            json!({"t": t, "art": "Einstellungen", "werte": self.einstellungen_fuer(person)});
        let routen = json!({"t": t, "art": "Routen", "routen": self.routenliste()});
        self.an(bid, &einstellungen);
        self.an(bid, &routen);
        self.raumbild(bid);
        Ok(())
    }

    /// Releases all subscriptions of a viewer and every "latest input" that names it (decision 4).
    fn abos_loesen(&mut self, bid: &str) {
        for s in self.sitzungen.values_mut() {
            if s.letzte_eingabe.as_deref() == Some(bid) {
                s.letzte_eingabe = None;
            }
        }
        let timer: Vec<Handle> = self
            .betrachter_mut(bid)
            .map(|b| {
                b.abos.clear();
                b.stand.clear();
                b.gemeldet.clear();
                std::mem::take(&mut b.rueck)
                    .into_values()
                    .filter_map(|r| r.timer)
                    .collect()
            })
            .unwrap_or_default();
        for handle in timer {
            self.timer.remove(&handle);
        }
    }

    fn e_trennen(&mut self, e: &Map<String, Value>) -> Ergebnis {
        let bid = self.betr(e)?;
        self.abos_loesen(&bid);
        self.betrachter.retain(|(id, _)| *id != bid);
        Ok(())
    }

    fn e_raumwechsel(&mut self, e: &Map<String, Value>) -> Ergebnis {
        let bid = self.betr(e)?;
        let raum = text(e, "raum").ok_or(UNGUELTIG)?;
        self.abos_loesen(&bid);
        if let Some(b) = self.betrachter_mut(&bid) {
            raum.clone_into(&mut b.raum);
        }
        let leeren = json!({"t": self.t, "art": "Leeren", "raum": raum});
        self.an(&bid, &leeren);
        self.raumbild(&bid);
        Ok(())
    }

    fn e_abo(&mut self, e: &Map<String, Value>) -> Ergebnis {
        let bid = self.betr(e)?;
        let sid = self.sitz(e)?;
        let raum = self.betrachter(&bid).map(|b| b.raum.clone());
        let s = self.sitzung_mut(&sid)?;
        if s.kundenraum != raum {
            return Err(Abgelehnt("fremder_kundenraum"));
        }
        let offen = s.dialog.is_some();
        let letzte = s.letzte_eingabe.as_deref() == Some(bid.as_str());
        if e.get("an").is_none_or(wahr) {
            let groesse = Self::groesse_pruefen(e)?;
            if let Some(b) = self.betrachter_mut(&bid) {
                if !b.abos.contains(&sid) {
                    b.abos.push(sid.clone());
                }
                b.gemeldet.insert(sid.clone(), groesse);
            }
            self.abzug(&bid, &sid);
            if offen {
                let dialog = self.dialog_bild(&sid);
                self.an(&bid, &dialog);
            }
            return Ok(());
        }
        let b = self
            .betrachter_mut(&bid)
            .ok_or(Abgelehnt("betrachter_unbekannt"))?;
        let Some(pos) = b.abos.iter().position(|a| *a == sid) else {
            return Err(Abgelehnt("kein_abo"));
        };
        b.abos.remove(pos);
        b.stand.remove(&sid);
        b.gemeldet.remove(&sid);
        self.rueckstand_loeschen(&bid, &sid);
        if letzte {
            self.sitzung_mut(&sid)?.letzte_eingabe = None;
        }
        Ok(())
    }

    fn e_groesse(&mut self, e: &Map<String, Value>) -> Ergebnis {
        let bid = self.betr(e)?;
        let sid = self.sitz(e)?;
        let abo = self.betrachter(&bid).is_some_and(|b| b.abos.contains(&sid));
        if !abo {
            return Err(Abgelehnt("kein_abo"));
        }
        let (spalten, zeilen) = Self::groesse_pruefen(e)?;
        if let Some(b) = self.betrachter_mut(&bid) {
            b.gemeldet.insert(sid.clone(), (spalten, zeilen));
        }
        if self.sitzung_mut(&sid)?.letzte_eingabe.as_deref() == Some(bid.as_str()) {
            self.groesse_setzen(&sid, spalten, zeilen);
        }
        Ok(())
    }

    /// The carrier reports the bytes the socket of a viewer has not taken yet; under the limit the viewer catches
    /// up (R14).
    fn e_socket(&mut self, e: &Map<String, Value>) -> Ergebnis {
        let bid = self.betr(e)?;
        let ausstehend = e
            .get("ausstehend")
            .and_then(Value::as_i64)
            .filter(|a| *a >= 0)
            .ok_or(UNGUELTIG)?;
        if let Some(b) = self.betrachter_mut(&bid) {
            b.ausstehend = ausstehend;
        }
        self.nachholen(&bid);
        Ok(())
    }

    // ---------- input to the PTY ----------

    /// The modifier bits of a key event, or `None` for an unknown or repeated modifier.
    fn umschalter(e: &Map<String, Value>) -> Option<u8> {
        let namen: &[Value] = match e.get("umschalter") {
            None => &[],
            Some(Value::Array(a)) => a,
            Some(_) => return None,
        };
        namen.iter().try_fold(0_u8, |bits, name| {
            let bit = eingabe::umschalter_bit(name.as_str()?)?;
            (bits & bit == 0).then_some(bits + bit)
        })
    }

    fn eingabe(&mut self, e: &Map<String, Value>, art: Eingabe) -> Ergebnis {
        let bid = self.betr(e)?;
        let sid = self.sitz(e)?;
        let abo = self.betrachter(&bid).is_some_and(|b| b.abos.contains(&sid));
        if !abo {
            return Err(Abgelehnt("kein_abo"));
        }
        let s = self.sitzung_mut(&sid)?;
        if s.beendet {
            return Err(Abgelehnt("sitzung_beendet"));
        }
        let daten = if art == Eingabe::Antwort {
            let Some(dialog) = s
                .dialog
                .as_mut()
                .filter(|d| Some(d.id.as_str()) == text(e, "dialog"))
            else {
                return Err(Abgelehnt("dialog_unbekannt"));
            };
            if dialog.beantwortet {
                return Err(Abgelehnt("bereits_beantwortet"));
            }
            let hoechste = i64::try_from(dialog.optionen.len().min(9)).unwrap_or(9);
            let wahl = e
                .get("wahl")
                .and_then(Value::as_i64)
                .filter(|w| (1..=hoechste).contains(w))
                .and_then(|w| u8::try_from(w).ok())
                .ok_or(Abgelehnt("wahl_ungueltig"))?;
            dialog.beantwortet = true;
            vec![b'0' + wahl]
        } else {
            if s.dialog.is_some() {
                return Err(Abgelehnt("pane_busy"));
            }
            let klammern = s.bracketed_paste;
            Self::bytes_der_eingabe(e, art, klammern)?
        };
        if daten.is_empty() {
            return Ok(());
        }
        self.pty(&sid, &daten);
        self.sitzung_mut(&sid)?.letzte_eingabe = Some(bid.clone());
        let gemeldet = self
            .betrachter(&bid)
            .and_then(|b| b.gemeldet.get(&sid).copied());
        if let Some((spalten, zeilen)) = gemeldet {
            self.groesse_setzen(&sid, spalten, zeilen);
        }
        Ok(())
    }

    /// The bytes for a text, key or paste event (R1).
    fn bytes_der_eingabe(
        e: &Map<String, Value>,
        art: Eingabe,
        klammern: bool,
    ) -> Result<Vec<u8>, Abgelehnt> {
        match art {
            Eingabe::Taste => {
                let taste = text(e, "taste").ok_or(Abgelehnt("taste_unbekannt"))?;
                let bits = Self::umschalter(e).ok_or(Abgelehnt("taste_unbekannt"))?;
                eingabe::taste_bytes(taste, bits).ok_or(Abgelehnt("taste_unbekannt"))
            }
            Eingabe::Text => {
                let eingabe = text(e, "text").ok_or(UNGUELTIG)?;
                eingabe::text_bytes(eingabe).ok_or(Abgelehnt("steuerzeichen_im_text"))
            }
            Eingabe::Einfuegen | Eingabe::Antwort => {
                let eingabe = text(e, "text").ok_or(UNGUELTIG)?;
                Ok(eingabe::einfuegen_bytes(eingabe, klammern))
            }
        }
    }

    // ---------- carrier ----------

    /// A cursor as the carrier reports it: `[row, column, visible]` with whole numbers inside `groesse` = (columns,
    /// rows) and a truth value (R15, point 1). Anything else is `ereignis_ungueltig`.
    pub(crate) fn cursor_pruefen(wert: &Value, groesse: (i64, i64)) -> Result<Cursor, Abgelehnt> {
        match wert.as_array().map(Vec::as_slice) {
            Some([zeile, spalte, Value::Bool(sichtbar)]) => {
                let zeile = zeile.as_i64().filter(|z| (0..groesse.1).contains(z));
                let spalte = spalte.as_i64().filter(|s| (0..groesse.0).contains(s));
                zeile
                    .zip(spalte)
                    .map(|(z, s)| (z, s, *sichtbar))
                    .ok_or(UNGUELTIG)
            }
            _ => Err(UNGUELTIG),
        }
    }

    /// Rejects a stand that is not above the highest known one (R4).
    fn stand_pruefen(&mut self, sid: &str, stand: i64) -> Ergebnis {
        let s = self.sitzung_mut(sid)?;
        let hoechster = s.sync.as_ref().and_then(|y| y.stand).unwrap_or(s.stand);
        if stand <= hoechster {
            return Err(Abgelehnt("stand_alt"));
        }
        Ok(())
    }

    /// Takes lines, the inverse mode and/or the cursor of one stand; inside a synchronised update all wait.
    fn uebernehmen(
        &mut self,
        sid: &str,
        stand: i64,
        neu: BTreeMap<usize, String>,
        invers: Option<bool>,
        cursor: Option<Cursor>,
    ) {
        let Some(s) = self.sitzungen.get_mut(sid) else {
            return;
        };
        if let Some(sync) = s.sync.as_mut() {
            sync.zeilen.extend(neu);
            sync.stand = Some(stand);
            if invers.is_some() {
                sync.invers = invers;
            }
            if cursor.is_some() {
                sync.cursor = cursor;
            }
            return;
        }
        self.anwenden(sid, stand, &neu, invers, cursor);
    }

    fn e_ausgabe(&mut self, e: &Map<String, Value>) -> Ergebnis {
        let sid = self.sitz(e)?;
        let (Some(stand), Some(Value::Object(zeilen))) =
            (e.get("stand").and_then(Value::as_i64), e.get("zeilen"))
        else {
            return Err(UNGUELTIG);
        };
        // The cursor is checked before the stand: an invalid cursor with an old stand is `ereignis_ungueltig` (R15).
        let cursor = match e.get("cursor") {
            None => None,
            Some(c) => Some(Self::cursor_pruefen(c, self.sitzung_mut(&sid)?.groesse)?),
        };
        self.stand_pruefen(&sid, stand)?;
        let hoechste = self.sitzung_mut(&sid)?.groesse.1;
        let mut neu = BTreeMap::new();
        for (k, v) in zeilen {
            let dezimal = !k.is_empty()
                && k.bytes().all(|b| b.is_ascii_digit())
                && (k.len() == 1 || !k.starts_with('0'));
            let index = k
                .parse::<usize>()
                .ok()
                .filter(|i| i64::try_from(*i).is_ok_and(|i| i < hoechste));
            match (dezimal, index, v.as_str()) {
                (true, Some(i), Some(zeile)) => {
                    neu.insert(i, zeile.to_owned());
                }
                _ => return Err(Abgelehnt("zeile_ausserhalb")),
            }
        }
        self.uebernehmen(&sid, stand, neu, None, cursor);
        Ok(())
    }

    fn e_sync_beginn(&mut self, e: &Map<String, Value>) -> Ergebnis {
        let sid = self.sitz(e)?;
        if self.sitzung_mut(&sid)?.sync.is_some() {
            return Ok(());
        }
        let timer = self.timer_setzen(self.t.saturating_add(SYNC_MS), Zeitgeber::Sync(sid.clone()));
        self.sitzung_mut(&sid)?.sync = Some(Synchron {
            zeilen: BTreeMap::new(),
            stand: None,
            invers: None,
            cursor: None,
            timer,
        });
        Ok(())
    }

    fn e_sync_ende(&mut self, e: &Map<String, Value>) -> Ergebnis {
        let sid = self.sitz(e)?;
        self.sync_freigeben(&sid);
        Ok(())
    }

    fn e_modus(&mut self, e: &Map<String, Value>) -> Ergebnis {
        let sid = self.sitz(e)?;
        let klammern = match e.get("bracketed_paste") {
            None => None,
            Some(Value::Bool(b)) => Some(*b),
            Some(_) => return Err(UNGUELTIG),
        };
        let invers = match (e.get("invers"), e.get("stand").and_then(Value::as_i64)) {
            (None, _) => None,
            (Some(Value::Bool(b)), Some(stand)) => Some((*b, stand)),
            _ => return Err(UNGUELTIG),
        };
        if klammern.is_none() && invers.is_none() {
            return Err(UNGUELTIG);
        }
        if let Some((_, stand)) = invers {
            self.stand_pruefen(&sid, stand)?;
        }
        if let Some(k) = klammern {
            self.sitzung_mut(&sid)?.bracketed_paste = k;
        }
        if let Some((wert, stand)) = invers {
            self.uebernehmen(&sid, stand, BTreeMap::new(), Some(wert), None);
        }
        Ok(())
    }

    fn e_titel(&mut self, e: &Map<String, Value>) -> Ergebnis {
        let sid = self.sitz(e)?;
        let titel = text(e, "text").ok_or(UNGUELTIG)?;
        let s = self.sitzung_mut(&sid)?;
        s.titel = Value::from(titel);
        let raum = s.kundenraum.clone();
        self.sitzungsliste_senden(raum.as_deref());
        Ok(())
    }

    fn e_kindende(&mut self, e: &Map<String, Value>) -> Ergebnis {
        let sid = self.sitz(e)?;
        let code = e.get("code").and_then(Value::as_i64).ok_or(UNGUELTIG)?;
        let s = self.sitzung_mut(&sid)?;
        s.beendet = true;
        s.code = Some(code);
        let raum = s.kundenraum.clone();
        self.sitzungsliste_senden(raum.as_deref());
        Ok(())
    }

    fn e_groessenfolge(&mut self, e: &Map<String, Value>) -> Ergebnis {
        self.sitz(e)?;
        Err(Abgelehnt("groesse_nur_kern"))
    }

    fn e_zwischenablage(&mut self, e: &Map<String, Value>) -> Ergebnis {
        let sid = self.sitz(e)?;
        if e.get("lesen").is_some_and(wahr) {
            return Err(Abgelehnt("osc52_lesen"));
        }
        let daten = text(e, "daten_b64").ok_or(UNGUELTIG)?;
        let roh = base64::dekodieren(daten).ok_or(Abgelehnt("osc52_ungueltig"))?;
        let s = self.sitzung_mut(&sid)?;
        let (person, ziel) = (s.person.clone(), s.letzte_eingabe.clone());
        let aus = self
            .regeln
            .get("osc52_schreiben")
            .is_some_and(|r| r.stufe(r.wirksam(person.as_deref()).0) == Some("aus"));
        if aus {
            return Ok(());
        }
        let grenze = self.wirksam("osc52_grenze", person.as_deref()).unwrap_or(0);
        let laenge = i64::try_from(roh.len()).unwrap_or(i64::MAX);
        if laenge > grenze {
            return Err(Abgelehnt("osc52_zu_gross"));
        }
        let abos = self.abonnenten(&sid);
        let Some(ziel) = ziel.filter(|z| abos.contains(z)) else {
            return Err(Abgelehnt("osc52_kein_empfaenger"));
        };
        let t = self.t;
        let inhalt = json!({"t": t, "art": "Zwischenablage", "sitzung": sid, "daten_b64": base64::kodieren(&roh)});
        self.an(&ziel, &inhalt);
        let hinweis = json!({"t": t, "art": "Hinweis", "hinweis": "osc52_geschrieben", "sitzung": sid,
            "laenge": laenge});
        self.an_alle(&abos, &hinweis);
        Ok(())
    }

    // ---------- dialog recognition ----------

    /// The dialog message of a session.
    pub(crate) fn dialog_bild(&self, sid: &str) -> Value {
        let d = self.sitzungen.get(sid).and_then(|s| s.dialog.as_ref());
        json!({"t": self.t, "art": "Dialog", "sitzung": sid, "id": d.map(|d| &d.id),
            "dialog_art": d.map(|d| &d.art), "optionen": d.map(|d| &d.optionen)})
    }

    fn e_dialog(&mut self, e: &Map<String, Value>) -> Ergebnis {
        let sid = self.sitz(e)?;
        let (Some(id), Some(Value::Array(optionen))) = (text(e, "id"), e.get("optionen")) else {
            return Err(UNGUELTIG);
        };
        if optionen.is_empty() {
            return Err(UNGUELTIG);
        }
        self.sitzung_mut(&sid)?.dialog = Some(Dialog {
            id: id.to_owned(),
            art: e.get("dialog_art").cloned().unwrap_or(Value::Null),
            optionen: optionen.clone(),
            beantwortet: false,
        });
        let bild = self.dialog_bild(&sid);
        let ziele = self.abonnenten(&sid);
        self.an_alle(&ziele, &bild);
        Ok(())
    }

    fn e_dialog_ende(&mut self, e: &Map<String, Value>) -> Ergebnis {
        let sid = self.sitz(e)?;
        let dialog = self
            .sitzung_mut(&sid)?
            .dialog
            .take()
            .ok_or(Abgelehnt("dialog_unbekannt"))?;
        let nachricht = json!({"t": self.t, "art": "DialogEnde", "sitzung": sid, "id": dialog.id});
        let ziele = self.abonnenten(&sid);
        self.an_alle(&ziele, &nachricht);
        Ok(())
    }

    // ---------- decision point and gateway ----------

    fn e_anfrage(&mut self, e: &Map<String, Value>) -> Ergebnis {
        let (Some(aid), Some(frist)) = (text(e, "id"), e.get("frist").and_then(Value::as_i64))
        else {
            return Err(UNGUELTIG);
        };
        let sid = self.sitz(e)?;
        if let Some(a) = self.anfragen.get_mut(aid) {
            if a.zustand != AnfrageZustand::Offen {
                return Err(Abgelehnt("nicht_offen"));
            }
            a.vorschau = e.get("vorschau").cloned().unwrap_or(Value::Null);
        } else {
            let abgelaufen = frist <= self.t;
            let timer =
                (!abgelaufen).then(|| self.timer_setzen(frist, Zeitgeber::Frist(aid.to_owned())));
            self.anfragen.insert(
                aid.to_owned(),
                Anfrage {
                    id: aid.to_owned(),
                    sitzung: sid,
                    werkzeug: e.get("werkzeug").cloned().unwrap_or(Value::Null),
                    vorschau: e.get("vorschau").cloned().unwrap_or(Value::Null),
                    frist,
                    zustand: if abgelaufen {
                        AnfrageZustand::Abgelaufen
                    } else {
                        AnfrageZustand::Offen
                    },
                    begruendung: None,
                    timer,
                },
            );
            self.anfrage_folge.push(aid.to_owned());
        }
        self.anfrage_melden(aid);
        Ok(())
    }

    fn e_nachweis(&mut self, e: &Map<String, Value>) -> Ergebnis {
        let person = text(e, "person").filter(|p| self.personen.contains_key(*p));
        let (Some(nid), Some(person)) = (text(e, "nachweis"), person) else {
            return Err(UNGUELTIG);
        };
        if self.nachweise.contains_key(nid) {
            return Err(Abgelehnt("nachweis_doppelt"));
        }
        self.nachweise.insert(
            nid.to_owned(),
            Nachweis {
                person: Some(person.to_owned()),
                benutzt: false,
            },
        );
        Ok(())
    }

    fn e_freigabe(&mut self, e: &Map<String, Value>) -> Ergebnis {
        let bid = self.betr(e)?;
        let Some(Value::Array(ids)) = e.get("anfragen") else {
            return Err(UNGUELTIG);
        };
        let ids: Vec<&str> = ids
            .iter()
            .map(Value::as_str)
            .collect::<Option<_>>()
            .ok_or(UNGUELTIG)?;
        // R-L6 point 1: a missing field never leads to a release, so it is rejected before any other form check.
        let ablehnen = match e.get("entscheidung") {
            None | Some(Value::Null) => return Err(Abgelehnt("entscheidung_fehlt")),
            Some(Value::String(w)) if w == "erteilen" => false,
            Some(Value::String(w)) if w == "ablehnen" => true,
            Some(_) => return Err(UNGUELTIG),
        };
        let begruendung = begruendung_pruefen(e, ablehnen)?;
        let [aid] = ids.as_slice() else {
            return Err(Abgelehnt("sammelfreigabe"));
        };
        let (pid, raum) = self
            .betrachter(&bid)
            .map(|b| (b.person.clone(), b.raum.clone()))
            .ok_or(Abgelehnt("betrachter_unbekannt"))?;
        if !self.personen.get(&pid).is_some_and(|p| p.mensch) {
            return Err(Abgelehnt("agent_kennung"));
        }
        let nid = match e.get("nachweis") {
            None | Some(Value::Null) => return Err(Abgelehnt("nachweis_fehlt")),
            Some(v) => v.as_str().ok_or(Abgelehnt("nachweis_ungueltig"))?,
        };
        let nachweis = self
            .nachweise
            .get(nid)
            .filter(|n| n.person.as_deref() == Some(pid.as_str()))
            .ok_or(Abgelehnt("nachweis_ungueltig"))?;
        if nachweis.benutzt {
            return Err(Abgelehnt("nachweis_verbraucht"));
        }
        let a = self
            .anfragen
            .get(*aid)
            .ok_or(Abgelehnt("anfrage_unbekannt"))?;
        let anfrage_raum = self
            .sitzungen
            .get(&a.sitzung)
            .and_then(|s| s.kundenraum.as_deref());
        if anfrage_raum != Some(raum.as_str()) {
            return Err(Abgelehnt("fremder_kundenraum"));
        }
        match a.zustand {
            AnfrageZustand::Abgelaufen => return Err(Abgelehnt("abgelaufen")),
            AnfrageZustand::Erteilt | AnfrageZustand::Abgelehnt => {
                return Err(Abgelehnt("nicht_offen"));
            }
            AnfrageZustand::Offen => {}
        }
        if let Some(n) = self.nachweise.get_mut(nid) {
            n.benutzt = true;
        }
        let mut eintrag = json!({"t": self.t, "anfrage": aid, "person": pid, "nachweis": nid});
        let zustand = if ablehnen {
            if let Some(m) = eintrag.as_object_mut() {
                m.insert("entscheidung".to_owned(), json!("ablehnen"));
                if let Some(grund) = &begruendung {
                    m.insert("begruendung".to_owned(), json!(grund));
                }
            }
            AnfrageZustand::Abgelehnt
        } else {
            AnfrageZustand::Erteilt
        };
        self.anfrage_schliessen(aid, zustand, begruendung);
        self.aus.an_stelle.push(eintrag);
        self.anfrage_melden(aid);
        Ok(())
    }

    /// The decision of the decision point (R-L6 point 6): no proof is used, `an_stelle` stays empty.
    fn e_anfrage_stand(&mut self, e: &Map<String, Value>) -> Ergebnis {
        let zustand = match text(e, "zustand") {
            Some("erteilt") => AnfrageZustand::Erteilt,
            Some("abgelehnt") => AnfrageZustand::Abgelehnt,
            Some("abgelaufen") => AnfrageZustand::Abgelaufen,
            _ => return Err(UNGUELTIG),
        };
        let aid = text(e, "id").ok_or(UNGUELTIG)?;
        let begruendung = begruendung_pruefen(e, zustand == AnfrageZustand::Abgelehnt)?;
        let a = self
            .anfragen
            .get(aid)
            .ok_or(Abgelehnt("anfrage_unbekannt"))?;
        if a.zustand != AnfrageZustand::Offen {
            return Err(Abgelehnt("nicht_offen"));
        }
        self.anfrage_schliessen(aid, zustand, begruendung);
        self.anfrage_melden(aid);
        Ok(())
    }

    /// Ends an open request: the new state, the reason, no timer any more.
    fn anfrage_schliessen(
        &mut self,
        aid: &str,
        zustand: AnfrageZustand,
        begruendung: Option<String>,
    ) {
        let mut timer = None;
        if let Some(a) = self.anfragen.get_mut(aid) {
            a.zustand = zustand;
            a.begruendung = begruendung;
            timer = a.timer.take();
        }
        if let Some(handle) = timer {
            self.timer.remove(&handle);
        }
    }

    // ---------- routes ----------

    fn e_route_waehlen(&mut self, e: &Map<String, Value>) -> Ergebnis {
        let bid = self.betr(e)?;
        let sid = self.sitz(e)?;
        let raum = self.betrachter(&bid).map(|b| b.raum.clone());
        let s = self.sitzung_mut(&sid)?;
        if s.kundenraum != raum {
            return Err(Abgelehnt("fremder_kundenraum"));
        }
        let name = e.get("route").cloned().unwrap_or(Value::Null);
        let t = self.t;
        let Some(route) = self.routen.iter().find(|r| r.name == name) else {
            let hinweis =
                json!({"t": t, "art": "Hinweis", "hinweis": "route_ohne_klasse", "route": name});
            self.an(&bid, &hinweis);
            return Err(Abgelehnt("route_ohne_klasse"));
        };
        if route.status != "frei" {
            let hinweis = json!({"t": t, "art": "Hinweis", "hinweis": "route_gesperrt", "route": name,
                "grund": route.grund});
            self.an(&bid, &hinweis);
            return Err(Abgelehnt("route_gesperrt"));
        }
        let s = self.sitzung_mut(&sid)?;
        s.route = name;
        let raum = s.kundenraum.clone();
        self.sitzungsliste_senden(raum.as_deref());
        Ok(())
    }

    // ---------- agent state ----------

    fn e_agent_abfrage(&mut self, e: &Map<String, Value>) -> Ergebnis {
        let wid = text(e, "worker")
            .filter(|w| self.worker.contains_key(*w))
            .ok_or(Abgelehnt("worker_unbekannt"))?;
        let zustand = match text(e, "zustand") {
            Some("laeuft") => "laeuft",
            Some("wartet") => "wartet",
            Some("fertig") => "fertig",
            Some("unbekannt") => {
                self.aus.abfragen.push(json!({"t": self.t, "worker": wid}));
                return Ok(());
            }
            _ => return Err(UNGUELTIG),
        };
        let stand = AgentStand {
            zustand,
            gesehen_um: self.t,
            quelle: e.get("quelle").cloned().unwrap_or(Value::Null),
        };
        let sitzung = self.worker.get(wid).map(|w| w.sitzung.clone());
        let nachricht = json!({"t": self.t, "art": "Agentenzustand", "worker": wid, "sitzung": sitzung,
            "zustand": stand.zustand, "gesehen_um": stand.gesehen_um, "quelle": stand.quelle});
        let Some(w) = self.worker.get_mut(wid) else {
            return Ok(());
        };
        w.stand = Some(stand);
        let raum = self
            .worker
            .get(wid)
            .and_then(|w| self.raum_des_workers(w))
            .map(str::to_owned);
        if let Some(raum) = raum {
            let ziele = self.im_raum(&raum);
            self.an_alle(&ziele, &nachricht);
        }
        Ok(())
    }

    // ---------- messages between sessions ----------

    fn e_nachricht(&mut self, e: &Map<String, Value>) -> Ergebnis {
        let sid = self.sitz(e)?;
        let nachricht = text(e, "text").ok_or(UNGUELTIG)?;
        let herkunft = e.get("herkunft").and_then(Value::as_object);
        let absender = match herkunft.and_then(|h| text(h, "art").map(|a| (a, h))) {
            Some(("sitzung", h)) => {
                let von = text(h, "sitzung")
                    .filter(|s| self.sitzungen.contains_key(*s))
                    .ok_or(Abgelehnt("absender_unbekannt"))?;
                json!({"art": "sitzung", "sitzung": von})
            }
            Some(("mensch", h)) => {
                let person = text(h, "betrachter")
                    .and_then(|b| self.betrachter(b))
                    .map(|b| &b.person)
                    .filter(|p| self.personen.get(*p).is_some_and(|p| p.mensch))
                    .ok_or(Abgelehnt("absender_unbekannt"))?;
                json!({"art": "mensch", "person": person})
            }
            _ => return Err(Abgelehnt("absender_fehlt")),
        };
        let raum = self.sitzung_mut(&sid)?.kundenraum.clone();
        if let Some(raum) = raum {
            let t = self.t;
            let bericht = json!({"t": t, "art": "Nachricht", "sitzung": sid, "absender": absender,
                "text": nachricht, "zeit": t});
            let ziele = self.im_raum(&raum);
            self.an_alle(&ziele, &bericht);
        }
        Ok(())
    }

    // ---------- settings ----------

    fn e_einstellung_setzen(&mut self, e: &Map<String, Value>) -> Ergebnis {
        let bid = self.betr(e)?;
        let schluessel = text(e, "schluessel")
            .filter(|k| self.regeln.contains_key(*k))
            .ok_or(Abgelehnt("schluessel_unbekannt"))?;
        let Some(ebene @ ("firma" | "gruppe" | "nutzer")) = text(e, "ebene") else {
            return Err(UNGUELTIG);
        };
        let regel = self
            .regeln
            .get(schluessel)
            .ok_or(Abgelehnt("schluessel_unbekannt"))?;
        let wert = e
            .get("wert")
            .and_then(|w| regel.wert_aus(w))
            .filter(|w| !regel.zahl || *w >= regel.min.max(0))
            .ok_or(Abgelehnt("wert_ungueltig"))?;
        let pid = self
            .betrachter(&bid)
            .map(|b| b.person.clone())
            .ok_or(Abgelehnt("betrachter_unbekannt"))?;
        let admin = self.personen.get(&pid).is_some_and(|p| p.admin);
        let (alt, betroffen) = if ebene == "nutzer" {
            if !regel.nutzer_darf {
                return Err(Abgelehnt("regel_nicht_beim_nutzer"));
            }
            if !regel.strenger(wert, regel.basis().0) {
                return Err(Abgelehnt("nur_verschaerfen"));
            }
            (regel.nutzer.get(&pid).copied(), vec![pid.clone()])
        } else {
            if !admin {
                return Err(Abgelehnt("nur_admin"));
            }
            let alt = if ebene == "firma" {
                regel.firma
            } else {
                regel.gruppe
            };
            (
                alt,
                self.betrachter
                    .iter()
                    .map(|(_, b)| b.person.clone())
                    .collect(),
            )
        };
        let regel = self
            .regeln
            .get_mut(schluessel)
            .ok_or(Abgelehnt("schluessel_unbekannt"))?;
        match ebene {
            "firma" => regel.firma = Some(wert),
            "gruppe" => regel.gruppe = Some(wert),
            _ => {
                regel.nutzer.insert(pid.clone(), wert);
            }
        }
        let (alt_json, neu_json) = (alt.map(|a| regel.wert_json(a)), regel.wert_json(wert));
        self.aus.protokoll.push(
            json!({"t": self.t, "person": pid, "schluessel": schluessel, "ebene": ebene,
            "alt": alt_json, "neu": neu_json}),
        );
        let ziele: Vec<(String, String)> = self
            .betrachter
            .iter()
            .filter(|(_, b)| betroffen.contains(&b.person))
            .map(|(id, b)| (id.clone(), b.person.clone()))
            .collect();
        for (id, person) in ziele {
            let nachricht = json!({"t": self.t, "art": "Einstellungen", "werte": self.einstellungen_fuer(&person)});
            self.an(&id, &nachricht);
        }
        Ok(())
    }
}
