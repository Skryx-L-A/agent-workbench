//! Model zones in the gate (partial contract `rust/spec/datentor/zonen/VERTRAG.md`).
//!
//! A zone is an entry of the admins with a key, a class set, a condition and a list of routes; every route belongs to at
//! most one zone. [`darf_zonen`] adds a second check per route to [`darf`](crate::darf): with the option off it is exactly
//! `darf`, with the option on it runs the rules of section 5 in table order. The gate knows no zone name, no prefix and no
//! class name: keys are compared byte for byte, classes only by their position. Like `darf`, the function is pure and
//! never panics.

use crate::{
    Art, Einheit, Grund, Vorgabe, a50_freigabe, darf, ist_schluessel, kunde_pruefen, wirksamer_rang,
};

/// The classes a zone or a rule admits, by position in `klassen` (A29).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Klassensatz {
    /// Up to the highest class.
    Alle,
    /// Only the first class of `klassen`.
    Niedrigste,
    /// No class at all: every unit is blocked.
    Keine,
}

impl Klassensatz {
    /// The highest position the set admits; `None` lies below every class.
    fn grenze(self, v: &Vorgabe) -> Option<usize> {
        match self {
            Self::Alle => Some(v.hoechste()),
            Self::Niedrigste => Some(0),
            Self::Keine => None,
        }
    }
}

/// The condition of a zone (A128).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Bedingung {
    /// Always usable.
    Immer,
    /// Only with a reason why a zone of higher precedence is not enough.
    Nachrangig,
    /// Customer data only with a release per route, customer and project.
    Freigabe,
    /// Blocked; an evaluation exception lifts the block.
    Gesperrt,
}

/// A zone of the admins.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Zone {
    /// The key of the zone.
    pub schluessel: String,
    /// The class set of the zone.
    pub klassen: Klassensatz,
    /// The condition of the zone.
    pub bedingung: Bedingung,
    /// The keys of the routes of the zone.
    pub routen: Vec<String>,
}

/// A release of the admins: customer data of this customer and project may go to this route of a zone with the
/// condition `freigabe`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjektFreigabe {
    /// The key of the route.
    pub route: String,
    /// The key of the customer.
    pub kunde: String,
    /// The key of the project.
    pub projekt: String,
}

/// An evaluation exception for a blocked zone.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ausnahme {
    /// The key of the blocked zone.
    pub zone: String,
    /// The users the exception names.
    pub nutzer: Vec<String>,
    /// The groups the exception names.
    pub gruppen: Vec<String>,
    /// The key of the admin who set it.
    pub gesetzt_von: String,
    /// The purpose.
    pub zweck: String,
    /// The deadline in seconds since 1970-01-01 00:00:00 UTC; at `bis` the exception has expired.
    pub bis: u64,
}

/// The level of a rule.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ebene {
    /// A rule for a group.
    Gruppe,
    /// A rule for a user.
    Nutzer,
}

/// A rule of a group or a user: it can only tighten a zone.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Regel {
    /// The level of the rule.
    pub ebene: Ebene,
    /// The key of the group or the user.
    pub wer: String,
    /// The key of the zone.
    pub zone: String,
    /// A class set; it may not admit more than the class set of the zone.
    pub klassen: Option<Klassensatz>,
    /// True if the rule blocks the zone; false means "does not block".
    pub gesperrt: bool,
}

/// The first violation found by [`Zonen::neu`] (Z10).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ZonenFehler {
    /// A key does not have the form `^[a-z0-9][a-z0-9._-]{0,63}$`.
    SchluesselForm(String),
    /// A zone key occurs twice.
    ZoneDoppelt(String),
    /// The first zone has the condition `nachrangig`.
    NachrangigZuerst(String),
    /// A zone names a route that is not in the settings.
    RouteUnbekannt {
        /// The key of the zone.
        zone: String,
        /// The key of the route.
        route: String,
    },
    /// A route stands in two zones or twice in one zone.
    RouteDoppelt(String),
    /// A release names a route without a zone.
    FreigabeOhneZone(String),
    /// A release names a route whose zone does not have the condition `freigabe`.
    FreigabeOhneBedingung(String),
    /// A release names an unknown customer.
    FreigabeKundeUnbekannt(String),
    /// An exception names a zone that does not exist.
    AusnahmeZoneUnbekannt(String),
    /// An exception names a zone without the condition `gesperrt`.
    AusnahmeZoneNichtGesperrt(String),
    /// An exception names neither users nor groups.
    AusnahmeOhneWen(String),
    /// The purpose of an exception has no character other than whitespace.
    ZweckLeer(String),
    /// A rule names a zone that does not exist.
    RegelZoneUnbekannt(String),
    /// A rule has neither a class set nor a block.
    RegelOhneWirkung(String),
    /// A rule admits more classes than its zone.
    RegelLockert {
        /// The key of the group or user of the rule.
        wer: String,
        /// The key of the zone.
        zone: String,
    },
}

impl std::fmt::Display for ZonenFehler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SchluesselForm(s) => write!(f, "key {s:?} does not have the key form"),
            Self::ZoneDoppelt(s) => write!(f, "zone {s:?} occurs twice"),
            Self::NachrangigZuerst(s) => {
                write!(f, "the first zone {s:?} has the condition nachrangig")
            }
            Self::RouteUnbekannt { zone, route } => {
                write!(f, "zone {zone:?} names the unknown route {route:?}")
            }
            Self::RouteDoppelt(s) => write!(f, "route {s:?} stands in more than one zone place"),
            Self::FreigabeOhneZone(s) => write!(f, "release names the route {s:?} without a zone"),
            Self::FreigabeOhneBedingung(s) => {
                write!(
                    f,
                    "release names the route {s:?} whose zone is not freigabe"
                )
            }
            Self::FreigabeKundeUnbekannt(s) => {
                write!(f, "release names the unknown customer {s:?}")
            }
            Self::AusnahmeZoneUnbekannt(s) => write!(f, "exception names the unknown zone {s:?}"),
            Self::AusnahmeZoneNichtGesperrt(s) => {
                write!(f, "exception names the zone {s:?} that is not gesperrt")
            }
            Self::AusnahmeOhneWen(s) => {
                write!(f, "exception for zone {s:?} names neither users nor groups")
            }
            Self::ZweckLeer(s) => write!(f, "exception for zone {s:?} has an empty purpose"),
            Self::RegelZoneUnbekannt(s) => write!(f, "rule names the unknown zone {s:?}"),
            Self::RegelOhneWirkung(s) => {
                write!(f, "rule of {s:?} has neither klassen nor gesperrt")
            }
            Self::RegelLockert { wer, zone } => {
                write!(f, "rule of {wer:?} loosens zone {zone:?}")
            }
        }
    }
}

impl std::error::Error for ZonenFehler {}

/// True if `s` has a character other than space, tab, LF and CR (the contract's whitespace).
fn hat_zeichen(s: &str) -> bool {
    s.chars().any(|c| !matches!(c, ' ' | '\t' | '\n' | '\r'))
}

/// A key of the form of contract 3, or the error.
fn schluessel(s: &str) -> Result<(), ZonenFehler> {
    if ist_schluessel(s) {
        Ok(())
    } else {
        Err(ZonenFehler::SchluesselForm(s.to_owned()))
    }
}

/// The checked model zones; they can only be made by [`Zonen::neu`].
#[derive(Clone, Debug)]
pub struct Zonen {
    an: bool,
    /// The zones in order of precedence.
    liste: Vec<Zone>,
    freigaben: Vec<ProjektFreigabe>,
    ausnahmen: Vec<Ausnahme>,
    regeln: Vec<Regel>,
}

impl Zonen {
    /// Builds the model zones for the settings `v` and checks section 3 of the partial contract; the error names the
    /// first violation. `an` is the option `modellzonen.an`; an invalid zone is refused also with the option off.
    ///
    /// # Errors
    ///
    /// Returns the first violation of section 3 (Z10), among them a rule that admits more classes than its zone.
    pub fn neu(
        v: &Vorgabe,
        an: bool,
        zonen: Vec<Zone>,
        freigaben: Vec<ProjektFreigabe>,
        ausnahmen: Vec<Ausnahme>,
        regeln: Vec<Regel>,
    ) -> Result<Self, ZonenFehler> {
        let mut belegt: Vec<&str> = Vec::new();
        for (i, z) in zonen.iter().enumerate() {
            schluessel(&z.schluessel)?;
            if zonen[..i].iter().any(|x| x.schluessel == z.schluessel) {
                return Err(ZonenFehler::ZoneDoppelt(z.schluessel.clone()));
            }
            if i == 0 && z.bedingung == Bedingung::Nachrangig {
                return Err(ZonenFehler::NachrangigZuerst(z.schluessel.clone()));
            }
            for r in &z.routen {
                if !v.routen.iter().any(|x| x.schluessel == *r) {
                    return Err(ZonenFehler::RouteUnbekannt {
                        zone: z.schluessel.clone(),
                        route: r.clone(),
                    });
                }
                if belegt.contains(&r.as_str()) {
                    return Err(ZonenFehler::RouteDoppelt(r.clone()));
                }
                belegt.push(r);
            }
        }
        let zone_der = |route: &str| zonen.iter().find(|z| z.routen.iter().any(|r| r == route));
        let zone_mit = |s: &str| zonen.iter().find(|z| z.schluessel == s);
        for f in &freigaben {
            match zone_der(&f.route) {
                None => return Err(ZonenFehler::FreigabeOhneZone(f.route.clone())),
                Some(z) if z.bedingung != Bedingung::Freigabe => {
                    return Err(ZonenFehler::FreigabeOhneBedingung(f.route.clone()));
                }
                Some(_) => {}
            }
            if !v.kunden.iter().any(|k| k.schluessel == f.kunde) {
                return Err(ZonenFehler::FreigabeKundeUnbekannt(f.kunde.clone()));
            }
            schluessel(&f.projekt)?;
        }
        for a in &ausnahmen {
            match zone_mit(&a.zone) {
                None => return Err(ZonenFehler::AusnahmeZoneUnbekannt(a.zone.clone())),
                Some(z) if z.bedingung != Bedingung::Gesperrt => {
                    return Err(ZonenFehler::AusnahmeZoneNichtGesperrt(a.zone.clone()));
                }
                Some(_) => {}
            }
            for s in a.nutzer.iter().chain(&a.gruppen) {
                schluessel(s)?;
            }
            if a.nutzer.is_empty() && a.gruppen.is_empty() {
                return Err(ZonenFehler::AusnahmeOhneWen(a.zone.clone()));
            }
            schluessel(&a.gesetzt_von)?;
            if !hat_zeichen(&a.zweck) {
                return Err(ZonenFehler::ZweckLeer(a.zone.clone()));
            }
        }
        for r in &regeln {
            schluessel(&r.wer)?;
            let Some(z) = zone_mit(&r.zone) else {
                return Err(ZonenFehler::RegelZoneUnbekannt(r.zone.clone()));
            };
            match r.klassen {
                None if !r.gesperrt => return Err(ZonenFehler::RegelOhneWirkung(r.wer.clone())),
                Some(k) if k.grenze(v) > z.klassen.grenze(v) => {
                    return Err(ZonenFehler::RegelLockert {
                        wer: r.wer.clone(),
                        zone: r.zone.clone(),
                    });
                }
                _ => {}
            }
        }
        Ok(Self {
            an,
            liste: zonen,
            freigaben,
            ausnahmen,
            regeln,
        })
    }

    /// The zone of a route, if it has one.
    fn zone_der_route(&self, route: &str) -> Option<&Zone> {
        self.liste
            .iter()
            .find(|z| z.routen.iter().any(|r| r == route))
    }

    /// The first exception that holds for zone `zone` and the context (section 6).
    fn geltende_ausnahme(&self, zone: &str, k: &Kontext<'_>) -> Option<&Ausnahme> {
        // An exception may hold through a group without a user (addendum 2); the answer is still Z1, the log names it.
        let jetzt = k.jetzt?;
        self.ausnahmen.iter().find(|a| {
            a.zone == zone
                && jetzt < a.bis
                && (a.nutzer.iter().any(|n| k.nutzer == Some(n.as_str()))
                    || k.gruppen.iter().any(|g| a.gruppen.iter().any(|x| x == g)))
        })
    }

    /// The rules of zone `zone` that hold for the user or one of the groups of the context.
    fn geltende_regeln<'z>(
        &'z self,
        zone: &'z str,
        k: &'z Kontext<'_>,
    ) -> impl Iterator<Item = &'z Regel> {
        self.regeln.iter().filter(move |r| {
            r.zone == zone
                && match r.ebene {
                    Ebene::Nutzer => k.nutzer == Some(r.wer.as_str()),
                    Ebene::Gruppe => k.gruppen.contains(&r.wer.as_str()),
                }
        })
    }
}

/// The context of a request; it comes from the start of the session, never from the agent's request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Kontext<'a> {
    /// The key of the user of the session.
    pub nutzer: Option<&'a str>,
    /// The groups of the user.
    pub gruppen: &'a [&'a str],
    /// The time of the check in seconds since 1970-01-01 00:00:00 UTC; the gate has no clock.
    pub jetzt: Option<u64>,
    /// The reason for choosing a zone of lower precedence.
    pub grund: Option<&'a str>,
}

/// The reason of a [`ZonenAntwort`]: a reason of contract 3 or one of the new rules Z1 to Z7.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ZonenGrund {
    /// A reason of contract 3; `Tor(Grund::Frei)` means allowed.
    Tor(Grund),
    /// Z1: the request has no user.
    NutzerFehlt,
    /// Z2: the route is not in the settings or in no zone.
    RouteOhneZone,
    /// Z3: a rule of the user or one of the groups blocks the zone.
    ZoneDurchRegelGesperrt,
    /// Z4: the zone is blocked and no exception holds.
    ZoneGesperrt,
    /// Z5: the zone is of lower precedence and the reason is not valid.
    GrundFehlt,
    /// Z6: the effective class lies above the limit of the zone.
    KlasseUeberZone,
    /// Z7: customer data to a zone with the condition `freigabe` without a release for route, customer and project.
    ZoneOhneFreigabe,
}

impl ZonenGrund {
    /// The code of the reason, in key form.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::Tor(g) => g.code(),
            Self::NutzerFehlt => "nutzer-fehlt",
            Self::RouteOhneZone => "route-ohne-zone",
            Self::ZoneDurchRegelGesperrt => "zone-durch-regel-gesperrt",
            Self::ZoneGesperrt => "zone-gesperrt",
            Self::GrundFehlt => "grund-fehlt",
            Self::KlasseUeberZone => "klasse-ueber-zone",
            Self::ZoneOhneFreigabe => "zone-ohne-freigabe",
        }
    }
}

/// What the log line names with the option on (Z8).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Protokoll {
    /// The key of the zone of the route.
    pub zone: Option<String>,
    /// The reason for the zone of lower precedence, exactly as passed.
    pub grund: Option<String>,
    /// The evaluation exception that holds.
    pub ausnahme: Option<Ausnahme>,
}

/// The answer of [`darf_zonen`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ZonenAntwort {
    /// The reason; `ZonenGrund::Tor(Grund::Frei)` means allowed.
    pub grund: ZonenGrund,
    /// The effective class (D5), or `None` without a unit or without a valid class.
    pub klasse: Option<String>,
    /// The log data; `None` with the option off.
    pub protokoll: Option<Protokoll>,
}

impl ZonenAntwort {
    /// True exactly if the reason is `frei`.
    #[must_use]
    pub fn darf(&self) -> bool {
        self.grund == ZonenGrund::Tor(Grund::Frei)
    }

    /// The fixed message of the reason; `{route}` is the route as it was passed.
    #[must_use]
    pub fn meldung(&self, route: Option<&str>) -> String {
        let r = route.unwrap_or_default();
        match self.grund {
            ZonenGrund::Tor(grund) => crate::Antwort {
                grund,
                klasse: None,
            }
            .meldung(route),
            ZonenGrund::NutzerFehlt => "Anfrage ohne Nutzer".to_owned(),
            ZonenGrund::RouteOhneZone => format!("Route {r} hat keine Zone"),
            ZonenGrund::ZoneDurchRegelGesperrt => {
                format!("Zone der Route {r} ist durch eine Regel gesperrt")
            }
            ZonenGrund::ZoneGesperrt => format!("Zone der Route {r} ist gesperrt"),
            ZonenGrund::GrundFehlt => {
                format!("Route {r} braucht einen Grund für die nachrangige Zone")
            }
            ZonenGrund::KlasseUeberZone => {
                format!("Klasse der Einheit liegt über der Zone der Route {r}")
            }
            ZonenGrund::ZoneOhneFreigabe => {
                format!("Kundendaten an Route {r} ohne Freigabe für Kunde und Projekt")
            }
        }
    }
}

/// Decides whether `einheit` may go to `route` under the model zones; without a unit it is the run check.
///
/// With the option off this is exactly [`darf`] (with a unit) or D2 and D3 (without one), and the log is `None`. With
/// the option on the rules of section 5 run in table order; the first that applies decides. `kunde` and `projekt` are
/// the customer and project of the unit; `k` is the context of the session. The function is pure and never panics.
#[must_use]
#[allow(
    clippy::many_single_char_names,
    reason = "the names of the contract's signature"
)]
pub fn darf_zonen(
    v: &Vorgabe,
    z: &Zonen,
    einheit: Option<&Einheit<'_>>,
    route: Option<&str>,
    kunde: Option<&str>,
    projekt: Option<&str>,
    k: &Kontext<'_>,
) -> ZonenAntwort {
    if !z.an {
        return match einheit {
            Some(e) => {
                let a = darf(v, e, route, kunde);
                ZonenAntwort {
                    grund: ZonenGrund::Tor(a.grund),
                    klasse: a.klasse,
                    protokoll: None,
                }
            }
            None => ZonenAntwort {
                grund: ZonenGrund::Tor(lauf_ohne_zonen(v, route)),
                klasse: None,
                protokoll: None,
            },
        };
    }
    let route = route.filter(|r| !r.is_empty());
    let zone = route.and_then(|r| z.zone_der_route(r));
    let ausnahme = zone
        .filter(|x| x.bedingung == Bedingung::Gesperrt)
        .and_then(|x| z.geltende_ausnahme(&x.schluessel, k));
    let grund = zone
        .filter(|x| x.bedingung == Bedingung::Nachrangig)
        .and(k.grund)
        .filter(|g| hat_zeichen(g));
    let wirksam = einheit.and_then(|e| wirksamer_rang(v, e, kunde));
    let pruefung = Pruefung {
        v,
        z,
        k,
        einheit,
        wirksam,
        kunde,
        projekt,
        ausnahme: ausnahme.is_some(),
        grund: grund.is_some(),
    };
    ZonenAntwort {
        grund: pruefung.grund(route, zone),
        klasse: wirksam.and_then(|w| v.klassen.get(w)).cloned(),
        protokoll: Some(Protokoll {
            zone: zone.map(|x| x.schluessel.clone()),
            grund: grund.map(str::to_owned),
            ausnahme: ausnahme.cloned(),
        }),
    }
}

/// The run check with the option off: only D2 and D3.
fn lauf_ohne_zonen(v: &Vorgabe, route: Option<&str>) -> Grund {
    let Some(route) = route.filter(|r| !r.is_empty()) else {
        return Grund::RouteFehlt;
    };
    match v.routen.iter().find(|r| r.schluessel == route) {
        Some(r) if r.klasse.is_some() => Grund::Frei,
        _ => Grund::RouteOhneKlasse,
    }
}

/// The inputs of the rules of section 5 for one request.
struct Pruefung<'p> {
    v: &'p Vorgabe,
    z: &'p Zonen,
    k: &'p Kontext<'p>,
    einheit: Option<&'p Einheit<'p>>,
    wirksam: Option<usize>,
    kunde: Option<&'p str>,
    projekt: Option<&'p str>,
    /// An exception holds for the zone.
    ausnahme: bool,
    /// The zone is of lower precedence and the reason is valid.
    grund: bool,
}

impl Pruefung<'_> {
    /// The rules of section 5 in table order.
    fn grund(&self, route: Option<&str>, zone: Option<&Zone>) -> ZonenGrund {
        let Some(route) = route else {
            return ZonenGrund::Tor(Grund::RouteFehlt);
        };
        if self.k.nutzer.is_none_or(str::is_empty) {
            return ZonenGrund::NutzerFehlt;
        }
        let (Some(zone), Some(eintrag)) =
            (zone, self.v.routen.iter().find(|r| r.schluessel == route))
        else {
            return ZonenGrund::RouteOhneZone;
        };
        if self.einheit.is_some() && self.wirksam.is_none() {
            return ZonenGrund::Tor(Grund::EinheitOhneKlasse);
        }
        if self
            .z
            .geltende_regeln(&zone.schluessel, self.k)
            .any(|r| r.gesperrt)
        {
            return ZonenGrund::ZoneDurchRegelGesperrt;
        }
        if zone.bedingung == Bedingung::Gesperrt && !self.ausnahme {
            return ZonenGrund::ZoneGesperrt;
        }
        if zone.bedingung == Bedingung::Nachrangig && !self.grund {
            return ZonenGrund::GrundFehlt;
        }
        let Some(w) = self.wirksam else {
            return ZonenGrund::Tor(Grund::Frei);
        };
        let grenze = self
            .z
            .geltende_regeln(&zone.schluessel, self.k)
            .filter_map(|r| r.klassen)
            .map(|s| s.grenze(self.v))
            .fold(zone.klassen.grenze(self.v), std::cmp::min);
        if Some(w) > grenze {
            return ZonenGrund::KlasseUeberZone;
        }
        if let Some(rk) = eintrag.klasse.as_deref()
            && self.v.rang(rk).is_none_or(|r| w > r)
        {
            return ZonenGrund::Tor(Grund::KlasseUeberRoute);
        }
        if self.kunde.is_none() && w != self.v.hoechste() {
            return ZonenGrund::Tor(Grund::Frei);
        }
        let Some(kunde) = self.kunde else {
            return ZonenGrund::Tor(Grund::KundeFehlt);
        };
        if let Some(g) = kunde_pruefen(self.v, route, kunde) {
            return ZonenGrund::Tor(g);
        }
        if zone.bedingung == Bedingung::Freigabe {
            let frei = self.projekt.is_some_and(|p| {
                self.z
                    .freigaben
                    .iter()
                    .any(|f| f.route == route && f.kunde == kunde && f.projekt == p)
            });
            if !frei {
                return ZonenGrund::ZoneOhneFreigabe;
            }
        } else if eintrag.art == Art::Cloud && !a50_freigabe(self.v, route, kunde) {
            return ZonenGrund::Tor(Grund::KundendatenAnCloud);
        }
        ZonenGrund::Tor(Grund::Frei)
    }
}
