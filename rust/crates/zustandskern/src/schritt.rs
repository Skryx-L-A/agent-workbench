//! The step API: a service leads the core through a session one event at a time.
//!
//! [`Sitzungskern`] is the core behind [`crate::laufen`]. `laufen` builds one core from the start of a case and
//! feeds it every event with [`Sitzungskern::schritt`]; a service does the same over the life of a session and
//! asks [`Sitzungskern::naechste_frist`] when it must send the next event of its own (a `Takt`) so that a due
//! timer runs. The clock stays outside: the only time is the field `t` of the events.

use std::collections::BTreeMap;

use serde_json::{Map, Value};

use crate::kern::{Abgelehnt, Kern};
use crate::{NACHRICHTEN_MAX, Ungueltig, ereignis_pruefen};

/// The effects of one step, in the form of the output of the program `zk` (without the end state).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Wirkungen {
    /// Messages per viewer in the order of sending.
    pub nachrichten: BTreeMap<String, Vec<Value>>,
    /// Bytes to the PTY (`t`, `sitzung`, `hex`).
    pub pty: Vec<Value>,
    /// Size commands to the carrier.
    pub groessen: Vec<Value>,
    /// Rejected events (`t`, `ereignis`, `grund`); `ereignis` counts the events of the whole session.
    pub ablehnungen: Vec<Value>,
    /// Log of setting changes.
    pub protokoll: Vec<Value>,
    /// Grants to the decision point.
    pub an_stelle: Vec<Value>,
    /// Immediate new queries of a worker.
    pub abfragen: Vec<Value>,
}

impl Wirkungen {
    /// Appends the effects of a later step: every list and every viewer's messages continue in order.
    pub fn anhaengen(&mut self, spaeter: Self) {
        for (bid, nachrichten) in spaeter.nachrichten {
            self.nachrichten.entry(bid).or_default().extend(nachrichten);
        }
        self.pty.extend(spaeter.pty);
        self.groessen.extend(spaeter.groessen);
        self.ablehnungen.extend(spaeter.ablehnungen);
        self.protokoll.extend(spaeter.protokoll);
        self.an_stelle.extend(spaeter.an_stelle);
        self.abfragen.extend(spaeter.abfragen);
    }

    /// Whether the step had no effect at all.
    #[must_use]
    pub fn ist_leer(&self) -> bool {
        self.nachrichten.is_empty()
            && self.pty.is_empty()
            && self.groessen.is_empty()
            && self.ablehnungen.is_empty()
            && self.protokoll.is_empty()
            && self.an_stelle.is_empty()
            && self.abfragen.is_empty()
    }

    /// The effects as one JSON object with the keys of the output of `zk` except `zustand`.
    #[must_use]
    pub fn als_wert(&self) -> Value {
        serde_json::json!({
            "nachrichten": self.nachrichten, "pty": self.pty, "groessen": self.groessen,
            "ablehnungen": self.ablehnungen, "protokoll": self.protokoll, "an_stelle": self.an_stelle,
            "abfragen": self.abfragen,
        })
    }
}

/// Why a step stopped, as the entry points tell it apart.
pub(crate) enum Abbruch {
    /// The case or the event is invalid.
    Ungueltig(Ungueltig),
    /// The messages of the step passed their limit.
    Ueberlauf,
}

impl From<Ungueltig> for Abbruch {
    fn from(grund: Ungueltig) -> Self {
        Self::Ungueltig(grund)
    }
}

/// The core of one session of the view, led event by event.
pub struct Sitzungskern {
    /// The state.
    kern: Kern,
    /// The most bytes the messages of one step may hold.
    grenze: usize,
    /// Number of events taken so far; the next event has this index.
    zaehler: usize,
    /// The `t` of the last event; the next must not fall below it.
    zuletzt: i64,
    /// A step passed the limit for its messages; the state is no longer sound.
    verbraucht: bool,
}

impl Sitzungskern {
    /// Builds a core from the start (`anfang`) of a case, with [`NACHRICHTEN_MAX`] as the limit of one step.
    ///
    /// # Errors
    ///
    /// [`Ungueltig`] when `anfang` is no object or is malformed.
    pub fn neu(anfang: &Value) -> Result<Self, Ungueltig> {
        Self::neu_mit(anfang, NACHRICHTEN_MAX)
    }

    /// Like [`Sitzungskern::neu`] with another limit for the bytes of the messages of one step.
    ///
    /// # Errors
    ///
    /// See [`Sitzungskern::neu`].
    pub fn neu_mit(anfang: &Value, nachrichten_max: usize) -> Result<Self, Ungueltig> {
        let Value::Object(anfang) = anfang else {
            return Err("anfang ist kein Objekt".into());
        };
        Self::aus_objekt(anfang, nachrichten_max)
    }

    /// Builds a core from the start as an object.
    pub(crate) fn aus_objekt(
        anfang: &Map<String, Value>,
        nachrichten_max: usize,
    ) -> Result<Self, Ungueltig> {
        Ok(Self {
            kern: Kern::neu(anfang, nachrichten_max)?,
            grenze: nachrichten_max,
            zaehler: 0,
            zuletzt: 0,
            verbraucht: false,
        })
    }

    /// Applies one event and returns its effects: first every timer that is due at or before its `t` runs, then
    /// the event itself. An event the core rejects is no error; it shows in `ablehnungen`. The index in
    /// `ablehnungen` counts the events given to this core, starting with 0.
    ///
    /// # Errors
    ///
    /// [`Ungueltig`] when the event is no object, has no whole number `t`, or has a `t` below the one before
    /// (the first below 0); the core is then unchanged and the event is not counted. [`Ungueltig`] also when the
    /// messages of this step pass the limit; the core is then spent and every later step fails.
    pub fn schritt(&mut self, ereignis: &Value) -> Result<Wirkungen, Ungueltig> {
        if self.verbraucht {
            return Err("Kern verbraucht".into());
        }
        let (e, t) = ereignis_pruefen(self.zaehler, ereignis, self.zuletzt)?;
        match self.schritt_geprueft(e, t, self.grenze) {
            Ok((wirkungen, _)) => Ok(wirkungen),
            Err(Abbruch::Ungueltig(grund)) => Err(grund),
            Err(Abbruch::Ueberlauf) => {
                Err(format!("Nachrichten uebersteigen {} Byte", self.grenze).into())
            }
        }
    }

    /// Applies an event that was checked already, with `grenze` as the limit of this step. Returns the effects
    /// and the bytes of their messages.
    pub(crate) fn schritt_geprueft(
        &mut self,
        ereignis: &Map<String, Value>,
        t: i64,
        grenze: usize,
    ) -> Result<(Wirkungen, usize), Abbruch> {
        self.kern.aus.bytes = 0;
        self.kern.aus.grenze = grenze;
        self.kern.aus.ueberlauf = false;
        self.zuletzt = t;
        self.kern.zeit_bis(t);
        self.kern.idx = self.zaehler;
        self.zaehler += 1;
        if let Err(Abgelehnt(grund)) = self.kern.ereignis(ereignis) {
            self.kern.ablehnen(grund);
        }
        if self.kern.aus.ueberlauf {
            self.verbraucht = true;
            return Err(Abbruch::Ueberlauf);
        }
        let bytes = self.kern.aus.bytes;
        Ok((self.kern.wirkungen_nehmen(), bytes))
    }

    /// The smallest `t` at which the core does something without a new event: a dispatch of raster changes
    /// after the distance of the frame rate limit (R5), a catch-up of a viewer that lies behind (R14), the end of
    /// a synchronised update or the end of a request. `None` when no timer is set. The value is always above the
    /// `t` of the last event; an event with exactly this `t` (a `Takt` will do) makes the effect happen.
    #[must_use]
    pub fn naechste_frist(&self) -> Option<i64> {
        self.kern.timer.keys().next().map(|&(zeit, _)| zeit)
    }

    /// The state as the field `zustand` of the output of `zk` shows it.
    #[must_use]
    pub fn zustand(&self) -> Value {
        self.kern.zustand()
    }
}

impl Kern {
    /// Takes the effects collected so far and leaves the lists empty.
    fn wirkungen_nehmen(&mut self) -> Wirkungen {
        let a = &mut self.aus;
        Wirkungen {
            nachrichten: std::mem::take(&mut a.nachrichten),
            pty: std::mem::take(&mut a.pty),
            groessen: std::mem::take(&mut a.groessen),
            ablehnungen: std::mem::take(&mut a.ablehnungen),
            protokoll: std::mem::take(&mut a.protokoll),
            an_stelle: std::mem::take(&mut a.an_stelle),
            abfragen: std::mem::take(&mut a.abfragen),
        }
    }
}
