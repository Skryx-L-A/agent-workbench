//! The read class of the person (partial contract `rust/spec/datentor/person/VERTRAG.md`).
//!
//! The read class of a person is the highest class the person may read. It comes from the entries of the admins, per user
//! or per group (P4). [`darf_person`] adds the check of the person to [`darf`](crate::darf): a unit goes out only if route
//! and person both admit it. [`lesen_person`] is the check without a route for a person who reads content herself (the
//! folder sheet of the service, Nachtrag 27.11.2026). A person without an entry has no read class and gets nothing (P2,
//! fail closed). The gate knows no class name: classes are compared only by their position. Like `darf`, both functions are
//! pure and never panic.

use crate::{Einheit, Grund, Vorgabe, darf, ist_schluessel, wirksamer_rang};

/// The most entries of `leseklassen` (Nachtrag 27.11.2026, the limit of the settings field).
pub const LESEKLASSEN_MAX: usize = 4096;

/// The level of an entry: a user or a group. A type of its own, so that `Ebene` of the zones stays untouched.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PersonEbene {
    /// An entry for one user.
    Nutzer,
    /// An entry for a group.
    Gruppe,
}

/// One entry of `leseklassen`: the read class of a user or of a group.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Leseeintrag {
    /// The level of the entry.
    pub ebene: PersonEbene,
    /// The key of the user or the group.
    pub wer: String,
    /// The read class, a key of `klassen`.
    pub klasse: String,
}

/// The first violation found by [`Personen::neu`] (P0).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PersonenFehler {
    /// More than [`LESEKLASSEN_MAX`] entries.
    ZuViele(usize),
    /// `wer` does not have the form `^[a-z0-9][a-z0-9._-]{0,63}$`.
    SchluesselForm(String),
    /// The class of an entry is not in `klassen` (a load error).
    KlasseUnbekannt {
        /// The key of the user or the group.
        wer: String,
        /// The class that is not in `klassen`.
        klasse: String,
    },
    /// The same pair of level and key stands twice, also with the same class.
    Doppelt(String),
}

impl std::fmt::Display for PersonenFehler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ZuViele(n) => write!(f, "{n} entries, more than {LESEKLASSEN_MAX}"),
            Self::SchluesselForm(s) => write!(f, "key {s:?} does not have the key form"),
            Self::KlasseUnbekannt { wer, klasse } => {
                write!(f, "class {klasse:?} of {wer:?} is not in klassen")
            }
            Self::Doppelt(s) => write!(f, "{s:?} stands twice on the same level"),
        }
    }
}

impl std::error::Error for PersonenFehler {}

/// An entry with the position of its class, checked against the settings.
#[derive(Clone, Debug)]
struct Eintrag {
    ebene: PersonEbene,
    wer: String,
    klasse: String,
    rang: usize,
}

/// The checked read classes; they can only be made by [`Personen::neu`].
#[derive(Clone, Debug)]
pub struct Personen {
    eintraege: Vec<Eintrag>,
}

/// The person of the session: the user and the groups from the start of the session, never from the agent's request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Person<'a> {
    /// The key of the user; `None` or the empty text means no person (P1).
    pub nutzer: Option<&'a str>,
    /// The keys of the groups of the user.
    pub gruppen: &'a [&'a str],
}

impl Person<'_> {
    /// The user, if there is one: `None` and the empty text are no user (P1).
    fn nutzer(&self) -> Option<&str> {
        self.nutzer.filter(|n| !n.is_empty())
    }
}

impl Personen {
    /// Builds the read classes for the settings `v` and checks section 3 of the partial contract; the error names the first
    /// violation. Without a valid list there is no gate (fail closed).
    ///
    /// # Errors
    ///
    /// Returns the first violation: more than [`LESEKLASSEN_MAX`] entries, a key of the wrong form, a class that is not in
    /// `klassen` (compared byte for byte) or a pair of level and key that stands twice.
    pub fn neu(v: &Vorgabe, leseklassen: Vec<Leseeintrag>) -> Result<Self, PersonenFehler> {
        if leseklassen.len() > LESEKLASSEN_MAX {
            return Err(PersonenFehler::ZuViele(leseklassen.len()));
        }
        let mut eintraege: Vec<Eintrag> = Vec::with_capacity(leseklassen.len());
        for e in leseklassen {
            if !ist_schluessel(&e.wer) {
                return Err(PersonenFehler::SchluesselForm(e.wer));
            }
            let Some(rang) = v.rang(&e.klasse) else {
                return Err(PersonenFehler::KlasseUnbekannt {
                    wer: e.wer,
                    klasse: e.klasse,
                });
            };
            if eintraege
                .iter()
                .any(|x| x.ebene == e.ebene && x.wer == e.wer)
            {
                return Err(PersonenFehler::Doppelt(e.wer));
            }
            eintraege.push(Eintrag {
                ebene: e.ebene,
                wer: e.wer,
                klasse: e.klasse,
                rang,
            });
        }
        Ok(Self { eintraege })
    }

    /// P4, the read class of the person as a key of `klassen`: the user's own entry if there is one, also below a group;
    /// else the highest class of the entries of the user's groups; else none. A person without a user has none.
    #[must_use]
    pub fn leseklasse(&self, p: &Person<'_>) -> Option<&str> {
        let nutzer = p.nutzer()?;
        if let Some(e) = self
            .eintraege
            .iter()
            .find(|e| e.ebene == PersonEbene::Nutzer && e.wer == nutzer)
        {
            return Some(&e.klasse);
        }
        self.eintraege
            .iter()
            .filter(|e| e.ebene == PersonEbene::Gruppe && p.gruppen.contains(&e.wer.as_str()))
            .max_by_key(|e| e.rang)
            .map(|e| e.klasse.as_str())
    }
}

/// The reason of a [`PersonAntwort`]: a reason of contract 3 or one of the person.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PersonGrund {
    /// A reason of contract 3, `Grund::Frei` included.
    Tor(Grund),
    /// P1: no person (no context, or the user is missing, `null` or empty).
    PersonFehlt,
    /// P2: the person has no read class by P4.
    PersonOhneLeseklasse,
    /// P3: the effective class of the unit lies above the read class of the person.
    KlasseUeberPerson,
}

impl PersonGrund {
    /// The code of the reason, in key form.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::Tor(g) => g.code(),
            Self::PersonFehlt => "person-fehlt",
            Self::PersonOhneLeseklasse => "person-ohne-leseklasse",
            Self::KlasseUeberPerson => "klasse-ueber-person",
        }
    }
}

/// The answer of the person check.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PersonAntwort {
    /// The reason; `PersonGrund::Tor(Grund::Frei)` means allowed.
    pub grund: PersonGrund,
    /// The effective class of the unit (D5), also for a reason of the person; `None` for a class that is given but unknown.
    pub klasse: Option<String>,
    /// The read class of the person by P4, or `None` if the person is missing or has none.
    pub leseklasse: Option<String>,
}

impl PersonAntwort {
    /// True exactly if the reason is `frei`.
    #[must_use]
    pub fn darf(&self) -> bool {
        self.grund == PersonGrund::Tor(Grund::Frei)
    }

    /// The fixed message of the reason; `{route}` is the route as it was passed. It never names the user, a group, content,
    /// title, path or customer of the unit.
    #[must_use]
    pub fn meldung(&self, route: Option<&str>) -> String {
        match self.grund {
            PersonGrund::Tor(g) => crate::Antwort {
                grund: g,
                klasse: None,
            }
            .meldung(route),
            PersonGrund::PersonFehlt => "Anfrage ohne Person".to_owned(),
            PersonGrund::PersonOhneLeseklasse => "Person ohne Leseklasse".to_owned(),
            PersonGrund::KlasseUeberPerson => {
                "Klasse der Einheit liegt über der Leseklasse der Person".to_owned()
            }
        }
    }
}

/// The read class of the person with its position in `v`; `None` if the person has none. A read class that `v` does not
/// know counts as none (fail closed; it can only happen with read classes built for other settings).
fn lese_rang<'p>(v: &Vorgabe, p: &'p Personen, person: &Person<'_>) -> Option<(&'p str, usize)> {
    let k = p.leseklasse(person)?;
    v.rang(k).map(|r| (k, r))
}

/// Decides whether `einheit` may go to `route` for `person` (section 4): `darf` of contract 3 and the person rules P1 to
/// P3 in the order D2, P1, P2, D3, D4, D6, P3, D7 to D11. Keys are compared byte for byte. Pure, never panics.
#[must_use]
pub fn darf_person(
    v: &Vorgabe,
    p: &Personen,
    einheit: &Einheit<'_>,
    route: Option<&str>,
    kunde: Option<&str>,
    person: &Person<'_>,
) -> PersonAntwort {
    let tor = darf(v, einheit, route, kunde);
    let lese = lese_rang(v, p, person);
    let leseklasse = lese.map(|(k, _)| k.to_owned());
    let grund = match tor.grund {
        Grund::RouteFehlt => PersonGrund::Tor(Grund::RouteFehlt),
        g => personenregeln(person, lese).unwrap_or_else(|| match g {
            Grund::RouteOhneKlasse | Grund::EinheitOhneKlasse | Grund::KlasseUeberRoute => {
                PersonGrund::Tor(g)
            }
            _ => ueber_person(v, einheit, kunde, lese).unwrap_or(PersonGrund::Tor(g)),
        }),
    };
    PersonAntwort {
        grund,
        klasse: tor.klasse,
        leseklasse,
    }
}

/// P1 and P2; `None` if the person is there and has a read class.
fn personenregeln(person: &Person<'_>, lese: Option<(&str, usize)>) -> Option<PersonGrund> {
    if person.nutzer().is_none() {
        return Some(PersonGrund::PersonFehlt);
    }
    if lese.is_none() {
        return Some(PersonGrund::PersonOhneLeseklasse);
    }
    None
}

/// P3; `None` if the effective class does not lie above the read class (or is unknown, which D4 decides).
fn ueber_person(
    v: &Vorgabe,
    einheit: &Einheit<'_>,
    kunde: Option<&str>,
    lese: Option<(&str, usize)>,
) -> Option<PersonGrund> {
    let wirksam = wirksamer_rang(v, einheit, kunde)?;
    let (_, grenze) = lese?;
    (wirksam > grenze).then_some(PersonGrund::KlasseUeberPerson)
}

/// Decides whether `person` may read `einheit` herself, without a route (Nachtrag 27.11.2026, rule P7): the folder sheet of
/// the service asks "may this person read content of this class". The rules run in the order P1, P2, D4, P3, D1; the rules
/// of a route (D2, D3, D6) and of the customer (D7 to D11) do not apply, because nothing goes to a model. `kunde` is the
/// customer the content belongs to: content of a customer has the highest class (D5). A unit without a class has the
/// `vorgabestufe` (D5). Pure, never panics.
#[must_use]
pub fn lesen_person(
    v: &Vorgabe,
    p: &Personen,
    einheit: &Einheit<'_>,
    kunde: Option<&str>,
    person: &Person<'_>,
) -> PersonAntwort {
    let wirksam = wirksamer_rang(v, einheit, kunde);
    let lese = lese_rang(v, p, person);
    let grund = personenregeln(person, lese).unwrap_or_else(|| {
        if wirksam.is_none() {
            PersonGrund::Tor(Grund::EinheitOhneKlasse)
        } else {
            ueber_person(v, einheit, kunde, lese).unwrap_or(PersonGrund::Tor(Grund::Frei))
        }
    });
    PersonAntwort {
        grund,
        klasse: wirksam.and_then(|w| v.klassen.get(w)).cloned(),
        leseklasse: lese.map(|(k, _)| k.to_owned()),
    }
}
