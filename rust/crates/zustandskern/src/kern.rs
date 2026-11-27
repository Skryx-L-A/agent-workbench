//! The state of the core: sessions, viewers, requests, timers and the effects collected so far.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use serde_json::{Map, Value, json};

use crate::regel::Regel;

/// Smallest distance between two raster changes of a session, in milliseconds.
pub(crate) const BUENDEL_MS: i64 = 16;
/// Time after which a synchronised update is released without its end mark, in milliseconds.
pub(crate) const SYNC_MS: i64 = 150;
/// Largest number of columns or rows.
pub(crate) const MAX_GROESSE: i64 = 1000;
/// Smallest number of columns. The whole session never takes one column: `alacritty_terminal` 0.26.0 panics at a wide character
/// and `Term::resize` never ends in a grid of one column (alacritty/alacritty issue 9112), and the carrier refuses such a size.
pub(crate) const MIN_SPALTEN: i64 = 2;
/// Smallest number of rows.
pub(crate) const MIN_ZEILEN: i64 = 1;

/// The cursor of a session: row, column and visibility (R15).
pub(crate) type Cursor = (i64, i64, bool);

/// A rejected event with its reason.
pub(crate) struct Abgelehnt(pub &'static str);

/// The result of handling one event.
pub(crate) type Ergebnis = Result<(), Abgelehnt>;

/// Identifies a timer: time and sequence number.
pub(crate) type Handle = (i64, u64);

/// What a timer does when it is due.
pub(crate) enum Zeitgeber {
    /// Send the collected raster changes of a session.
    Versand(String),
    /// Release a synchronised update of a session.
    Sync(String),
    /// Let a request expire.
    Frist(String),
    /// Catch a viewer up in a session after the distance since its last change (viewer, session).
    Nachholen(String, String),
}

/// A person known to the case.
pub(crate) struct Person {
    /// Role `admin`.
    pub admin: bool,
    /// Kind `mensch` (not an agent).
    pub mensch: bool,
}

/// A proof of a human person.
pub(crate) struct Nachweis {
    /// The person it belongs to.
    pub person: Option<String>,
    /// Whether it was used.
    pub benutzt: bool,
}

/// A route with its status.
pub(crate) struct Route {
    /// Name as the case wrote it.
    pub name: Value,
    /// Status as the case wrote it; only `frei` can be chosen.
    pub status: Value,
    /// Reason.
    pub grund: Value,
    /// Zone.
    pub zone: Value,
}

/// An open dialog of a session.
pub(crate) struct Dialog {
    /// Identifier.
    pub id: String,
    /// Kind as the recognition wrote it.
    pub art: Value,
    /// The options.
    pub optionen: Vec<Value>,
    /// Whether an answer was written already.
    pub beantwortet: bool,
}

/// A synchronised update that is waiting: lines and mode are held back until release.
pub(crate) struct Synchron {
    /// Lines that arrived meanwhile.
    pub zeilen: BTreeMap<usize, String>,
    /// The stand of the last output that arrived meanwhile.
    pub stand: Option<i64>,
    /// The inverse mode that arrived meanwhile.
    pub invers: Option<bool>,
    /// The latest cursor that arrived meanwhile (R15).
    pub cursor: Option<Cursor>,
    /// The timer that releases it.
    pub timer: Handle,
}

/// A terminal session.
pub(crate) struct Sitzung {
    /// Identifier.
    pub id: String,
    /// Title.
    pub titel: Value,
    /// Route as the case wrote it.
    pub route: Value,
    /// Customer room.
    pub kundenraum: Option<String>,
    /// Owner.
    pub person: Option<String>,
    /// Columns and rows.
    pub groesse: (i64, i64),
    /// The child process has ended.
    pub beendet: bool,
    /// Exit code of the child.
    pub code: Option<i64>,
    /// Bracketed paste mode.
    pub bracketed_paste: bool,
    /// The lines of the raster.
    pub zeilen: Vec<String>,
    /// Stand of the raster.
    pub stand: i64,
    /// Lines changed since the last dispatch.
    pub schmutzig: BTreeSet<usize>,
    /// Time of the last dispatch.
    pub letzter_versand: Option<i64>,
    /// A dispatch is scheduled.
    pub faellig: Option<i64>,
    /// A synchronised update is running.
    pub sync: Option<Synchron>,
    /// Inverse screen (DECSCNM).
    pub invers: bool,
    /// The cursor, once the core knows one (R15).
    pub cursor: Option<Cursor>,
    /// The viewer with the latest input.
    pub letzte_eingabe: Option<String>,
    /// The open dialog.
    pub dialog: Option<Dialog>,
}

/// What one viewer misses in one session while it lies behind the stand (R14).
#[derive(Default)]
pub(crate) struct Rueck {
    /// The lines of the images that expired for the viewer.
    pub verpasst: BTreeSet<usize>,
    /// Time of the last raster change sent to the viewer in this session.
    pub letzter: Option<i64>,
    /// The last raster change was a catch-up and not a dispatch of the session.
    pub nachgeholt: bool,
    /// The timer that ends the distance since the last raster change.
    pub timer: Option<Handle>,
}

/// A connected viewer.
pub(crate) struct Betrachter {
    /// The person.
    pub person: String,
    /// The customer room it looks at.
    pub raum: String,
    /// Subscribed sessions in the order of subscription.
    pub abos: Vec<String>,
    /// The stand of each session it received last.
    pub stand: BTreeMap<String, i64>,
    /// The size it reported for each subscribed session.
    pub gemeldet: BTreeMap<String, (i64, i64)>,
    /// The last report of the carrier: bytes the socket has not taken yet (R14).
    pub ausstehend: i64,
    /// Number of images that expired for it since it connected (R14).
    pub verfallen: i64,
    /// What it misses per session (R14).
    pub rueck: BTreeMap<String, Rueck>,
}

/// State of a request.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum AnfrageZustand {
    /// Waiting for a decision.
    Offen,
    /// Granted.
    Erteilt,
    /// Refused (R-L6).
    Abgelehnt,
    /// Run out.
    Abgelaufen,
}

impl AnfrageZustand {
    /// The word of the contract.
    pub(crate) fn text(self) -> &'static str {
        match self {
            Self::Offen => "offen",
            Self::Erteilt => "erteilt",
            Self::Abgelehnt => "abgelehnt",
            Self::Abgelaufen => "abgelaufen",
        }
    }
}

/// A request to the decision point.
pub(crate) struct Anfrage {
    /// Identifier.
    pub id: String,
    /// The session it belongs to.
    pub sitzung: String,
    /// Tool.
    pub werkzeug: Value,
    /// Preview.
    pub vorschau: Value,
    /// Absolute time at which it runs out.
    pub frist: i64,
    /// State.
    pub zustand: AnfrageZustand,
    /// The reason of a refusal (R-L6), if one was given.
    pub begruendung: Option<String>,
    /// The timer that lets it run out.
    pub timer: Option<Handle>,
}

/// The last known state of a worker.
pub(crate) struct AgentStand {
    /// `laeuft`, `wartet` or `fertig`.
    pub zustand: &'static str,
    /// Time of the query.
    pub gesehen_um: i64,
    /// Source as the query wrote it.
    pub quelle: Value,
}

/// A worker that belongs to a session.
pub(crate) struct Worker {
    /// Its session.
    pub sitzung: Option<String>,
    /// The last known state.
    pub stand: Option<AgentStand>,
}

/// The effects collected so far.
pub(crate) struct Aus {
    /// Messages per viewer in the order of sending.
    pub nachrichten: BTreeMap<String, Vec<Value>>,
    /// Bytes to the PTY.
    pub pty: Vec<Value>,
    /// Size commands to the carrier.
    pub groessen: Vec<Value>,
    /// Rejected events.
    pub ablehnungen: Vec<Value>,
    /// Log of setting changes.
    pub protokoll: Vec<Value>,
    /// Grants to the decision point.
    pub an_stelle: Vec<Value>,
    /// Immediate new queries of a worker.
    pub abfragen: Vec<Value>,
    /// Bytes of all messages so far, as serialised.
    pub bytes: usize,
    /// The limit of `bytes`.
    pub grenze: usize,
    /// The limit was passed; no message is kept any more.
    pub ueberlauf: bool,
}

/// The core.
pub(crate) struct Kern {
    /// The current time: the `t` of the event, or the time of a timer that runs before it.
    pub t: i64,
    /// Index of the event in work.
    pub idx: usize,
    /// Timers by time and sequence number.
    pub timer: BTreeMap<Handle, Zeitgeber>,
    /// Sequence number of the last timer.
    pub folge: u64,
    /// The effects.
    pub aus: Aus,
    /// Persons by name.
    pub personen: BTreeMap<String, Person>,
    /// Proofs by name.
    pub nachweise: BTreeMap<String, Nachweis>,
    /// Routes in the order of the case.
    pub routen: Vec<Route>,
    /// Settings by key.
    pub regeln: BTreeMap<String, Regel>,
    /// Sessions by identifier.
    pub sitzungen: BTreeMap<String, Sitzung>,
    /// Identifiers in the order of the case (an identifier given twice stands twice).
    pub reihenfolge: Vec<String>,
    /// Workers by identifier.
    pub worker: BTreeMap<String, Worker>,
    /// Requests by identifier.
    pub anfragen: BTreeMap<String, Anfrage>,
    /// Identifiers of the requests in the order of arrival.
    pub anfrage_folge: Vec<String>,
    /// Connected viewers in the order of connecting.
    pub betrachter: Vec<(String, Betrachter)>,
}

impl Aus {
    /// Starts without effects; `grenze` is the limit for the bytes of all messages.
    pub(crate) fn neu(grenze: usize) -> Self {
        Self {
            nachrichten: BTreeMap::new(),
            pty: Vec::new(),
            groessen: Vec::new(),
            ablehnungen: Vec::new(),
            protokoll: Vec::new(),
            an_stelle: Vec::new(),
            abfragen: Vec::new(),
            bytes: 0,
            grenze,
            ueberlauf: false,
        }
    }
}

/// Adds `cursor` to a message or to a session of the end state when the core knows one; otherwise the field stays
/// away (R15).
fn cursor_feld(cursor: Option<Cursor>, nachricht: &mut Value) {
    if let (Some((zeile, spalte, sichtbar)), Some(objekt)) = (cursor, nachricht.as_object_mut()) {
        objekt.insert("cursor".to_owned(), json!([zeile, spalte, sichtbar]));
    }
}

/// The cursor inside a size of `groesse` = (columns, rows): row and column each at most the last index (R15).
fn geklemmt(cursor: Cursor, groesse: (i64, i64)) -> Cursor {
    (
        cursor.0.min(groesse.1 - 1),
        cursor.1.min(groesse.0 - 1),
        cursor.2,
    )
}

/// Serialised length of a value in bytes.
fn laenge(wert: &Value) -> usize {
    serde_json::to_vec(wert).map_or(0, |b| b.len())
}

impl Kern {
    // ---------- output ----------

    /// Sends a message to each target. Returns `false` when the limit for all messages is passed.
    pub(crate) fn an_alle(&mut self, ziele: &[String], nachricht: &Value) -> bool {
        if self.aus.ueberlauf {
            return false;
        }
        let gesamt = laenge(nachricht).saturating_mul(ziele.len());
        self.aus.bytes = self.aus.bytes.saturating_add(gesamt);
        if self.aus.bytes > self.aus.grenze {
            self.aus.ueberlauf = true;
            return false;
        }
        for bid in ziele {
            self.aus
                .nachrichten
                .entry(bid.clone())
                .or_default()
                .push(nachricht.clone());
        }
        true
    }

    /// Sends a message to one viewer.
    pub(crate) fn an(&mut self, bid: &str, nachricht: &Value) -> bool {
        self.an_alle(&[bid.to_owned()], nachricht)
    }

    /// Rejects the event in work.
    pub(crate) fn ablehnen(&mut self, grund: &str) {
        self.aus
            .ablehnungen
            .push(json!({"t": self.t, "ereignis": self.idx, "grund": grund}));
    }

    /// Writes bytes to the PTY of a session.
    pub(crate) fn pty(&mut self, sid: &str, bytes: &[u8]) {
        let hex = bytes.iter().fold(String::new(), |mut hex, b| {
            let _ = write!(hex, "{b:02x}");
            hex
        });
        self.aus
            .pty
            .push(json!({"t": self.t, "sitzung": sid, "hex": hex}));
    }

    // ---------- lookup ----------

    /// The viewer with this identifier.
    pub(crate) fn betrachter(&self, bid: &str) -> Option<&Betrachter> {
        self.betrachter
            .iter()
            .find(|(id, _)| id == bid)
            .map(|(_, b)| b)
    }

    /// The viewer with this identifier, to change.
    pub(crate) fn betrachter_mut(&mut self, bid: &str) -> Option<&mut Betrachter> {
        self.betrachter
            .iter_mut()
            .find(|(id, _)| id == bid)
            .map(|(_, b)| b)
    }

    /// The viewers that look at a room, in the order of connecting.
    pub(crate) fn im_raum(&self, raum: &str) -> Vec<String> {
        self.betrachter
            .iter()
            .filter(|(_, b)| b.raum == raum)
            .map(|(id, _)| id.clone())
            .collect()
    }

    /// The viewers that subscribed to a session, in the order of connecting.
    pub(crate) fn abonnenten(&self, sid: &str) -> Vec<String> {
        self.betrachter
            .iter()
            .filter(|(_, b)| b.abos.iter().any(|a| a == sid))
            .map(|(id, _)| id.clone())
            .collect()
    }

    /// The session, to change.
    pub(crate) fn sitzung_mut(&mut self, sid: &str) -> Result<&mut Sitzung, Abgelehnt> {
        self.sitzungen
            .get_mut(sid)
            .ok_or(Abgelehnt("sitzung_unbekannt"))
    }

    /// The effective value of a rule for a person.
    pub(crate) fn wirksam(&self, schluessel: &str, person: Option<&str>) -> Option<i64> {
        self.regeln.get(schluessel).map(|r| r.wirksam(person).0)
    }

    // ---------- settings and lists ----------

    /// The settings message body for a person: value, default and level per key.
    pub(crate) fn einstellungen_fuer(&self, person: &str) -> Value {
        let werte: Map<String, Value> = self
            .regeln
            .iter()
            .map(|(k, r)| {
                let (wert, herkunft) = r.wirksam(Some(person));
                (
                    k.clone(),
                    json!({"wert": r.wert_json(wert), "vorgabe": r.wert_json(r.vorgabe), "herkunft": herkunft}),
                )
            })
            .collect();
        Value::Object(werte)
    }

    /// The sessions of a room.
    pub(crate) fn sitzungsliste(&self, raum: &str) -> Value {
        let liste: Vec<Value> = self
            .reihenfolge
            .iter()
            .filter_map(|sid| self.sitzungen.get(sid))
            .filter(|s| s.kundenraum.as_deref() == Some(raum))
            .map(|s| {
                json!({"id": s.id, "titel": s.titel, "route": s.route, "groesse": [s.groesse.0, s.groesse.1],
                       "zustand": if s.beendet { "kindende" } else { "laeuft" }, "code": s.code})
            })
            .collect();
        Value::Array(liste)
    }

    /// Sends the session list to every viewer of a room.
    pub(crate) fn sitzungsliste_senden(&mut self, raum: Option<&str>) {
        let Some(raum) = raum else { return };
        let nachricht =
            json!({"t": self.t, "art": "Sitzungsliste", "sitzungen": self.sitzungsliste(raum)});
        let ziele = self.im_raum(raum);
        self.an_alle(&ziele, &nachricht);
    }

    /// The routes with the flag whether one can choose them.
    pub(crate) fn routenliste(&self) -> Value {
        let liste: Vec<Value> = self
            .routen
            .iter()
            .map(|r| {
                json!({"name": r.name, "status": r.status, "grund": r.grund, "zone": r.zone,
                       "waehlbar": r.status == "frei"})
            })
            .collect();
        Value::Array(liste)
    }

    /// The image of a request.
    pub(crate) fn anfrage_bild(a: &Anfrage) -> Value {
        json!({"id": a.id, "sitzung": a.sitzung, "werkzeug": a.werkzeug, "vorschau": a.vorschau,
               "frist": a.frist, "zustand": a.zustand.text()})
    }

    /// The open requests of a room.
    pub(crate) fn offene_anfragen(&self, raum: &str) -> Value {
        let liste: Vec<Value> = self
            .anfrage_folge
            .iter()
            .filter_map(|aid| self.anfragen.get(aid))
            .filter(|a| a.zustand == AnfrageZustand::Offen)
            .filter(|a| {
                self.sitzungen
                    .get(&a.sitzung)
                    .is_some_and(|s| s.kundenraum.as_deref() == Some(raum))
            })
            .map(Self::anfrage_bild)
            .collect();
        Value::Array(liste)
    }

    /// The workers of a room that were seen.
    pub(crate) fn agenten(&self, raum: &str) -> Value {
        let liste: Vec<Value> = self
            .worker
            .iter()
            .filter_map(|(wid, w)| w.stand.as_ref().map(|st| (wid, w, st)))
            .filter(|(_, w, _)| self.raum_des_workers(w) == Some(raum))
            .map(|(wid, w, st)| {
                json!({"worker": wid, "sitzung": w.sitzung, "zustand": st.zustand, "gesehen_um": st.gesehen_um,
                    "quelle": st.quelle})
            })
            .collect();
        Value::Array(liste)
    }

    /// The customer room of the session of a worker.
    pub(crate) fn raum_des_workers(&self, w: &Worker) -> Option<&str> {
        w.sitzung
            .as_ref()
            .and_then(|sid| self.sitzungen.get(sid))
            .and_then(|s| s.kundenraum.as_deref())
    }

    /// The picture of the room of a viewer: sessions, open requests, workers.
    pub(crate) fn raumbild(&mut self, bid: &str) {
        let Some(raum) = self.betrachter(bid).map(|b| b.raum.clone()) else {
            return;
        };
        let t = self.t;
        let liste = json!({"t": t, "art": "Sitzungsliste", "sitzungen": self.sitzungsliste(&raum)});
        let anfragen = json!({"t": t, "art": "Anfragen", "anfragen": self.offene_anfragen(&raum)});
        let agenten = json!({"t": t, "art": "Agenten", "agenten": self.agenten(&raum)});
        self.an(bid, &liste);
        self.an(bid, &anfragen);
        self.an(bid, &agenten);
    }

    // ---------- grid ----------

    /// Sends the full raster of a session to a viewer and notes the stand it now has.
    pub(crate) fn abzug(&mut self, bid: &str, sid: &str) {
        let Some(s) = self.sitzungen.get(sid) else {
            return;
        };
        let stand = s.stand;
        let mut nachricht = json!({"t": self.t, "art": "Abzug", "sitzung": sid, "stand": s.stand,
            "groesse": [s.groesse.0, s.groesse.1], "invers": s.invers, "zeilen": s.zeilen});
        cursor_feld(s.cursor, &mut nachricht);
        self.rueckstand_loeschen(bid, sid);
        self.an(bid, &nachricht);
        if let Some(b) = self.betrachter_mut(bid) {
            b.stand.insert(sid.to_owned(), stand);
        }
    }

    /// Sets a timer and returns its handle.
    pub(crate) fn timer_setzen(&mut self, zeit: i64, zeitgeber: Zeitgeber) -> Handle {
        self.folge += 1;
        let handle = (zeit, self.folge);
        self.timer.insert(handle, zeitgeber);
        handle
    }

    /// The smallest distance between two raster changes of a session in whole milliseconds: the bundling
    /// distance or, when longer, the rounded-up reciprocal of the effective frame rate limit of its owner.
    pub(crate) fn abstand(&self, person: Option<&str>) -> i64 {
        let rate =
            u64::try_from(self.wirksam("bildrate_max", person).unwrap_or(1).max(1)).unwrap_or(1);
        BUENDEL_MS.max(i64::try_from(1000_u64.div_ceil(rate)).unwrap_or(1000))
    }

    /// Marks lines as changed and sends them now or at the end of the distance.
    pub(crate) fn schmutzig_machen(&mut self, sid: &str, zeilen: &[usize]) {
        let t = self.t;
        let Some(s) = self.sitzungen.get_mut(sid) else {
            return;
        };
        s.schmutzig.extend(zeilen.iter().copied());
        if s.faellig.is_some() {
            return;
        }
        let letzter = s.letzter_versand;
        let person = s.person.clone();
        let ab = self.abstand(person.as_deref());
        match letzter {
            Some(l) if t - l < ab => {
                let faellig = l.saturating_add(ab);
                self.timer_setzen(faellig, Zeitgeber::Versand(sid.to_owned()));
                if let Some(s) = self.sitzungen.get_mut(sid) {
                    s.faellig = Some(faellig);
                }
            }
            _ => self.versenden(sid),
        }
    }

    /// Sends the collected raster changes to every subscriber that does not have the stand yet; an image the
    /// viewer cannot take expires for it (R14).
    pub(crate) fn versenden(&mut self, sid: &str) {
        let t = self.t;
        let Some(s) = self.sitzungen.get_mut(sid) else {
            return;
        };
        s.faellig = None;
        s.letzter_versand = Some(t);
        let neu = std::mem::take(&mut s.schmutzig);
        for bid in self.abonnenten(sid) {
            if let Some(b) = self.betrachter_mut(&bid) {
                b.rueck
                    .entry(sid.to_owned())
                    .or_default()
                    .verpasst
                    .extend(neu.iter().copied());
            }
            self.beliefern(&bid, sid, true);
        }
    }

    /// Whether more bytes lie at the socket of a viewer than its limit allows; exactly the limit still passes (R14).
    fn gesperrt(&self, bid: &str) -> bool {
        self.betrachter(bid).is_some_and(|b| {
            b.ausstehend
                > self
                    .wirksam("rueckstau_max", Some(b.person.as_str()))
                    .unwrap_or(i64::MAX)
        })
    }

    /// Brings a viewer to the stand of a session. A dispatch of the session (`durch_versand`) expires for a viewer
    /// over its limit; a catch-up (a report of the socket or the timer) sends the whole backlog as one change (R14).
    pub(crate) fn beliefern(&mut self, bid: &str, sid: &str, durch_versand: bool) {
        let t = self.t;
        let Some(s) = self.sitzungen.get(sid) else {
            return;
        };
        let (stand, invers, person, cursor) = (s.stand, s.invers, s.person.clone(), s.cursor);
        let Some(b) = self.betrachter(bid) else {
            return;
        };
        let von = b.stand.get(sid).copied();
        let (letzter, nachgeholt) = b
            .rueck
            .get(sid)
            .map_or((None, false), |r| (r.letzter, r.nachgeholt));
        if von.is_some_and(|v| v >= stand) {
            if let Some(r) = self.betrachter_mut(bid).and_then(|b| b.rueck.get_mut(sid)) {
                r.verpasst.clear();
            }
            return;
        }
        if self.gesperrt(bid) {
            if durch_versand && let Some(b) = self.betrachter_mut(bid) {
                b.verfallen = b.verfallen.saturating_add(1);
            }
            return;
        }
        let ab = self.abstand(person.as_deref());
        if let Some(l) = letzter
            && t - l < ab
            && (!durch_versand || nachgeholt)
        {
            // The end of the distance lies after `t` here; the bound keeps it so, because a timer set for the
            // present would call this function again at once, without end.
            let neu = self.timer_setzen(
                l.saturating_add(ab).max(t.saturating_add(1)),
                Zeitgeber::Nachholen(bid.to_owned(), sid.to_owned()),
            );
            let alt = self
                .betrachter_mut(bid)
                .and_then(|b| b.rueck.get_mut(sid))
                .and_then(|r| r.timer.replace(neu));
            if let Some(alt) = alt {
                self.timer.remove(&alt);
            }
            return;
        }
        let Some(s) = self.sitzungen.get(sid) else {
            return;
        };
        let mut index: BTreeSet<usize> = self
            .betrachter(bid)
            .and_then(|b| b.rueck.get(sid))
            .map(|r| r.verpasst.clone())
            .unwrap_or_default();
        if !durch_versand {
            index.extend(s.schmutzig.iter().copied());
        }
        let zeilen: Map<String, Value> = index
            .iter()
            .filter_map(|&i| s.zeilen.get(i).map(|z| (i.to_string(), json!(z))))
            .collect();
        let mut nachricht = json!({"t": t, "art": "Rasteraenderung", "sitzung": sid, "von": von, "stand": stand,
            "invers": invers, "zeilen": zeilen});
        cursor_feld(cursor, &mut nachricht);
        self.an(bid, &nachricht);
        if let Some(b) = self.betrachter_mut(bid) {
            b.stand.insert(sid.to_owned(), stand);
            let r = b.rueck.entry(sid.to_owned()).or_default();
            r.verpasst.clear();
            r.letzter = Some(t);
            r.nachgeholt = !durch_versand;
        }
    }

    /// Catches a viewer up in every session in which it lies behind the stand (R14).
    pub(crate) fn nachholen(&mut self, bid: &str) {
        let abos = self
            .betrachter(bid)
            .map(|b| b.abos.clone())
            .unwrap_or_default();
        for sid in abos {
            self.beliefern(bid, &sid, false);
        }
    }

    /// Forgets what a viewer misses in a session and stops its timer (R14).
    pub(crate) fn rueckstand_loeschen(&mut self, bid: &str, sid: &str) {
        let rueck = self.betrachter_mut(bid).and_then(|b| b.rueck.remove(sid));
        if let Some(handle) = rueck.and_then(|r| r.timer) {
            self.timer.remove(&handle);
        }
    }

    /// Takes lines, stand and (when given) the inverse mode and the cursor into the raster and sends a change when
    /// something changed: the mode, the cursor, a line, or an output without lines. An output without lines is a
    /// pure cursor step or a plain stand and goes through the same distance as a line (R5, R15).
    pub(crate) fn anwenden(
        &mut self,
        sid: &str,
        stand: i64,
        zeilen: &BTreeMap<usize, String>,
        invers: Option<bool>,
        cursor: Option<Cursor>,
    ) {
        let Some(s) = self.sitzungen.get_mut(sid) else {
            return;
        };
        let mut geaendert = Vec::new();
        for (&i, text) in zeilen {
            if let Some(zeile) = s.zeilen.get_mut(i) {
                zeile.clone_from(text);
                geaendert.push(i);
            }
        }
        s.stand = stand;
        let wechsel = invers.is_some_and(|neu| neu != s.invers)
            || cursor.is_some_and(|neu| Some(neu) != s.cursor);
        if let Some(neu) = invers {
            s.invers = neu;
        }
        if cursor.is_some() {
            s.cursor = cursor;
        }
        if wechsel || !geaendert.is_empty() || invers.is_none() {
            self.schmutzig_machen(sid, &geaendert);
        }
    }

    /// Ends a synchronised update: the held lines and mode take effect like a new output.
    pub(crate) fn sync_freigeben(&mut self, sid: &str) {
        let Some(s) = self.sitzungen.get_mut(sid) else {
            return;
        };
        let Some(sync) = s.sync.take() else {
            return;
        };
        self.timer.remove(&sync.timer);
        if let Some(stand) = sync.stand {
            // The held cursor is clamped against the size that holds now (R15, point 8).
            let cursor = self
                .sitzungen
                .get(sid)
                .zip(sync.cursor)
                .map(|(s, c)| geklemmt(c, s.groesse));
            self.anwenden(sid, stand, &sync.zeilen, sync.invers, cursor);
        }
    }

    /// Sets the size of a session when it differs: cuts or fills the lines, tells the carrier and the subscribers.
    pub(crate) fn groesse_setzen(&mut self, sid: &str, spalten: i64, zeilen: i64) {
        let Some(s) = self.sitzungen.get_mut(sid) else {
            return;
        };
        if s.groesse == (spalten, zeilen) {
            return;
        }
        s.groesse = (spalten, zeilen);
        let n = usize::try_from(zeilen).unwrap_or(0);
        s.zeilen.resize(n, String::new());
        s.schmutzig.retain(|&i| i < n);
        // The cursor never leaves the size (R15, point 8); the subscribers get it with the full image below.
        s.cursor = s.cursor.map(|c| geklemmt(c, s.groesse));
        let raum = s.kundenraum.clone();
        self.aus
            .groessen
            .push(json!({"t": self.t, "sitzung": sid, "spalten": spalten, "zeilen": zeilen}));
        for bid in self.abonnenten(sid) {
            self.abzug(&bid, sid);
        }
        self.sitzungsliste_senden(raum.as_deref());
    }

    // ---------- timers ----------

    /// Runs every timer that is due at `t` or earlier, in the order of time and creation, then sets the time.
    pub(crate) fn zeit_bis(&mut self, t: i64) {
        while let Some(eintrag) = self.timer.first_entry() {
            let (zeit, _) = *eintrag.key();
            if zeit > t {
                break;
            }
            let zeitgeber = eintrag.remove();
            self.t = zeit;
            match zeitgeber {
                Zeitgeber::Versand(sid) => self.versenden(&sid),
                Zeitgeber::Sync(sid) => self.sync_freigeben(&sid),
                Zeitgeber::Nachholen(bid, sid) => {
                    if self.betrachter(&bid).is_some_and(|b| b.abos.contains(&sid)) {
                        self.beliefern(&bid, &sid, false);
                    }
                }
                Zeitgeber::Frist(aid) => {
                    if let Some(a) = self.anfragen.get_mut(&aid)
                        && a.zustand == AnfrageZustand::Offen
                    {
                        a.zustand = AnfrageZustand::Abgelaufen;
                        self.anfrage_melden(&aid);
                    }
                }
            }
        }
        self.t = t;
    }

    /// Tells the viewers of the room of the session about a request.
    pub(crate) fn anfrage_melden(&mut self, aid: &str) {
        let Some(a) = self.anfragen.get(aid) else {
            return;
        };
        let Some(raum) = self
            .sitzungen
            .get(&a.sitzung)
            .and_then(|s| s.kundenraum.clone())
        else {
            return;
        };
        let bild = Self::anfrage_bild(a);
        let mut nachricht = json!({"t": self.t, "art": "Anfrage", "id": bild["id"], "sitzung": bild["sitzung"],
            "werkzeug": bild["werkzeug"], "vorschau": bild["vorschau"], "frist": bild["frist"],
            "zustand": bild["zustand"]});
        if let (Some(grund), Some(m)) = (a.begruendung.as_ref(), nachricht.as_object_mut()) {
            m.insert("begruendung".to_owned(), json!(grund));
        }
        let ziele = self.im_raum(&raum);
        self.an_alle(&ziele, &nachricht);
    }

    // ---------- end ----------

    /// The end state as the contract lists it.
    pub(crate) fn zustand(&self) -> Value {
        let sitzungen: Vec<Value> = self
            .reihenfolge
            .iter()
            .filter_map(|sid| self.sitzungen.get(sid))
            .map(|s| {
                let mut bild = json!({"id": s.id, "titel": s.titel, "route": s.route, "kundenraum": s.kundenraum,
                    "groesse": [s.groesse.0, s.groesse.1],
                    "zustand": if s.beendet { "kindende" } else { "laeuft" }, "code": s.code,
                    "stand": s.stand, "letzte_eingabe": s.letzte_eingabe,
                    "dialog": s.dialog.as_ref().map(|d| &d.id),
                    "bracketed_paste": s.bracketed_paste, "invers": s.invers});
                cursor_feld(s.cursor, &mut bild);
                bild
            })
            .collect();
        let betrachter: Map<String, Value> = self
            .betrachter
            .iter()
            .map(|(bid, b)| {
                let rueckstand: Map<String, Value> = b
                    .abos
                    .iter()
                    .filter(|sid| {
                        let stand = self.sitzungen.get(*sid).map(|s| s.stand);
                        b.stand.get(*sid).copied().zip(stand).is_some_and(|(v, s)| v < s)
                    })
                    .map(|sid| {
                        let zeilen: Vec<usize> = b
                            .rueck
                            .get(sid)
                            .map(|r| r.verpasst.iter().copied().collect())
                            .unwrap_or_default();
                        (sid.clone(), json!(zeilen))
                    })
                    .collect();
                (
                    bid.clone(),
                    json!({"person": b.person, "raum": b.raum, "abos": b.abos,
                        "ausstehend": b.ausstehend, "verfallen": b.verfallen, "rueckstand": rueckstand}),
                )
            })
            .collect();
        let anfragen: Map<String, Value> = self
            .anfrage_folge
            .iter()
            .filter_map(|aid| {
                self.anfragen
                    .get(aid)
                    .map(|a| (aid.clone(), json!(a.zustand.text())))
            })
            .collect();
        let agenten: Map<String, Value> = self
            .worker
            .iter()
            .map(|(wid, w)| {
                let stand = w.stand.as_ref().map_or(Value::Null, |st| {
                    json!({"zustand": st.zustand, "gesehen_um": st.gesehen_um, "quelle": st.quelle})
                });
                (wid.clone(), stand)
            })
            .collect();
        let regeln: Map<String, Value> = self
            .regeln
            .iter()
            .map(|(k, r)| {
                let nutzer: Map<String, Value> = r
                    .nutzer
                    .iter()
                    .map(|(p, &w)| (p.clone(), r.wert_json(w)))
                    .collect();
                (
                    k.clone(),
                    json!({"firma": r.firma.map(|w| r.wert_json(w)), "gruppe": r.gruppe.map(|w| r.wert_json(w)),
                        "nutzer": nutzer}),
                )
            })
            .collect();
        json!({"sitzungen": sitzungen, "betrachter": betrachter, "anfragen": anfragen,
            "agenten": agenten, "regeln": regeln})
    }
}
