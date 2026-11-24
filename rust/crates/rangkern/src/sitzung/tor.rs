//! The gate in front of the output of the search socket (contract 4, section "Suche mit Tor").
//!
//! The gate acts on the whole list `L` that `rangkern serve` would give for the same request with no cut. For every hit it
//! asks `datentor::darf` with the route of the session, which comes from the start and never from a request. The first `k`
//! allowed hits are the answer; a blocked hit takes no place. Nothing of a blocked note stays in the answer: not its path,
//! its title, its excerpt, nor a reference to it from an allowed hit (`abgeloest_durch`, `contradiction`).

use std::collections::HashMap;

use datentor::{Einheit, Vorgabe, darf};

use crate::hit::SearchHit;
use crate::notes::NoteRec;

/// A pair of notes of an open contradiction, as the finding names them.
pub type Paar = (Option<String>, Option<String>);

/// The gate's judgement of one note.
#[derive(Debug, PartialEq, Eq)]
pub struct Urteil {
    /// The reason; `frei` means allowed.
    pub grund: &'static str,
    /// The effective class from `darf`, or `None` for a note without a valid class.
    pub klasse: Option<String>,
    /// The customer of the note for the log line: a text of the key form, else none.
    pub kunde: Option<String>,
}

impl Urteil {
    /// Whether the note may go to the route.
    #[must_use]
    pub fn darf(&self) -> bool {
        self.grund == "frei"
    }
}

/// One group of blocked hits for the log: the same class, customer and reason.
#[derive(Debug, PartialEq, Eq)]
pub struct Gruppe {
    /// The effective class, `None` for a note without a valid class.
    pub klasse: Option<String>,
    /// The customer, `None` for a note without one (or one that is no key).
    pub kunde: Option<String>,
    /// The code of the reason.
    pub grund: &'static str,
    /// How many blocked hits are in the group, from 1.
    pub anzahl: u32,
}

/// What the gate lets through.
#[derive(Debug)]
pub struct Durchgelassen {
    /// The first `k` allowed hits, with the references to blocked notes taken out.
    pub hits: Vec<SearchHit>,
    /// The groups of blocked hits among the first `k` of `L`, in the order of their first hit in `L`.
    pub gruppen: Vec<Gruppe>,
}

impl Durchgelassen {
    /// The number of blocked hits among the first `k` of `L`.
    #[must_use]
    pub fn gesperrt(&self) -> usize {
        self.gruppen.iter().map(|g| g.anzahl as usize).sum()
    }
}

/// The gate of one session: the settings and the route.
#[derive(Debug)]
pub struct Tor {
    vorgabe: Vorgabe,
    route: String,
}

impl Tor {
    /// The gate for `route` with the settings `vorgabe`.
    #[must_use]
    pub fn new(vorgabe: Vorgabe, route: String) -> Self {
        Self { vorgabe, route }
    }

    /// The judgement of a note. A note that is not in the index (it vanished since the list was made) is blocked: its class
    /// and its customer are the empty text, which no class and no customer of the settings has.
    #[must_use]
    pub fn urteil(&self, note: Option<&NoteRec>) -> Urteil {
        let (klasse, kunde, log) = match note {
            Some(note) => (
                note.datenklasse.for_gate(),
                note.kunde.for_gate(),
                note.kunde.log_key().map(str::to_owned),
            ),
            None => (Some(""), Some(""), None),
        };
        let antwort = darf(&self.vorgabe, &Einheit { klasse }, Some(&self.route), kunde);
        Urteil {
            grund: antwort.grund.code(),
            klasse: antwort.klasse,
            kunde: log,
        }
    }

    /// The gate on the list `liste` for a request of `k` hits. `notes` are the notes of the index by path, `paare` the open
    /// contradictions.
    #[must_use]
    pub fn durchlassen(
        &self,
        liste: Vec<SearchHit>,
        k: usize,
        notes: &HashMap<&str, &NoteRec>,
        paare: &[Paar],
    ) -> Durchgelassen {
        let urteil_von = |rel: &str| self.urteil(notes.get(rel).copied());
        let erlaubt = |rel: &str| urteil_von(rel).darf();
        let urteile: Vec<Urteil> = liste.iter().map(|hit| urteil_von(&hit.rel)).collect();
        let mut gruppen: Vec<Gruppe> = Vec::new();
        for urteil in urteile.iter().take(k).filter(|u| !u.darf()) {
            match gruppen.iter_mut().find(|g| {
                g.grund == urteil.grund && g.klasse == urteil.klasse && g.kunde == urteil.kunde
            }) {
                Some(gruppe) => gruppe.anzahl += 1,
                None => gruppen.push(Gruppe {
                    klasse: urteil.klasse.clone(),
                    kunde: urteil.kunde.clone(),
                    grund: urteil.grund,
                    anzahl: 1,
                }),
            }
        }
        let hits = liste
            .into_iter()
            .zip(&urteile)
            .filter(|(_, urteil)| urteil.darf())
            .map(|(hit, _)| hit)
            .take(k)
            .map(|mut hit| {
                let gezeigt = if hit.superseded_by_rel.is_empty() {
                    &hit.superseded_by
                } else {
                    &hit.superseded_by_rel
                };
                let rest: Vec<String> =
                    gezeigt.iter().filter(|rel| erlaubt(rel)).cloned().collect();
                hit.superseded_by_rel.clone_from(&rest);
                hit.superseded_by = rest;
                if hit.contradiction {
                    hit.contradiction = partner_erlaubt(&hit.rel, paare, &erlaubt);
                }
                hit
            })
            .collect();
        Durchgelassen { hits, gruppen }
    }
}

/// Whether an open contradiction has an allowed partner for the note at `rel`.
fn partner_erlaubt(rel: &str, paare: &[Paar], erlaubt: &impl Fn(&str) -> bool) -> bool {
    paare.iter().any(|(a, b)| {
        let partner = if a.as_deref() == Some(rel) {
            b
        } else if b.as_deref() == Some(rel) {
            a
        } else {
            return false;
        };
        partner.as_deref().is_some_and(erlaubt)
    })
}
