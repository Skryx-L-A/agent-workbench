//! The data gate `datentor` (contract `rust/spec/VERTRAEGE.md`, contract 3 of the walking skeleton).
//!
//! The gate is a pure library without input, output, clock or randomness. It gets a loaded [`Vorgabe`] and, per call, a
//! unit, a route and a customer. It answers with an [`Antwort`]: allowed or not, and the reason. A unit is seen only by
//! its class. The caller hands the model only the number of blocked units, never reason, class or message.
//!
//! The rules run in the order of the contract table; the first rule that applies decides (D12). Keys are compared byte
//! for byte (D14). [`darf`] is a pure function and never panics (D15).

pub mod klassen;
pub mod zonen;

/// The kind of a route: it decides only whether customer data need a release (rule D11).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Art {
    /// A route to a cloud model.
    Cloud,
    /// A route to a local model.
    Lokal,
}

/// A route of the settings: key, kind and the highest class the route may see.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Route {
    /// The key of the route.
    pub schluessel: String,
    /// The kind of the route.
    pub art: Art,
    /// The highest class the route may see; `None` means the route has no class (rule D3).
    pub klasse: Option<String>,
}

/// A customer of the settings with the routes it allows and forbids.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Kunde {
    /// The key of the customer.
    pub schluessel: String,
    /// The keys of the routes this customer allows.
    pub erlaubt: Vec<String>,
    /// The keys of the routes this customer forbids; forbidden goes before allowed (rule D9).
    pub verboten: Vec<String>,
}

/// A release of the admins: customer data of this customer may go to this cloud route (rule D11).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Freigabe {
    /// The key of the cloud route.
    pub route: String,
    /// The key of the customer.
    pub kunde: String,
}

/// The first violation found by [`Vorgabe::neu`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VorgabeFehler {
    /// `klassen` is empty.
    KlassenLeer,
    /// A key does not have the form `^[a-z0-9][a-z0-9._-]{0,63}$`.
    SchluesselForm(String),
    /// A key occurs twice in its list.
    SchluesselDoppelt(String),
    /// `vorgabestufe` is not in `klassen`.
    VorgabestufeUnbekannt(String),
    /// The class of a route is not in `klassen`.
    RouteKlasseUnbekannt {
        /// The key of the route.
        route: String,
        /// The class that is not in `klassen`.
        klasse: String,
    },
    /// `erlaubt` or `verboten` of a customer names a route that does not exist.
    RouteUnbekannt {
        /// The key of the customer.
        kunde: String,
        /// The key of the route that does not exist.
        route: String,
    },
    /// A release names a route that does not exist.
    FreigabeRouteUnbekannt(String),
    /// A release names a route of the kind `lokal`.
    FreigabeRouteLokal(String),
    /// A release names a customer that does not exist.
    FreigabeKundeUnbekannt(String),
}

impl std::fmt::Display for VorgabeFehler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::KlassenLeer => write!(f, "klassen is empty"),
            Self::SchluesselForm(s) => write!(f, "key {s:?} does not have the key form"),
            Self::SchluesselDoppelt(s) => write!(f, "key {s:?} occurs twice"),
            Self::VorgabestufeUnbekannt(s) => write!(f, "vorgabestufe {s:?} is not in klassen"),
            Self::RouteKlasseUnbekannt { route, klasse } => {
                write!(f, "class {klasse:?} of route {route:?} is not in klassen")
            }
            Self::RouteUnbekannt { kunde, route } => {
                write!(f, "customer {kunde:?} names the unknown route {route:?}")
            }
            Self::FreigabeRouteUnbekannt(s) => write!(f, "release names the unknown route {s:?}"),
            Self::FreigabeRouteLokal(s) => write!(f, "release names the local route {s:?}"),
            Self::FreigabeKundeUnbekannt(s) => {
                write!(f, "release names the unknown customer {s:?}")
            }
        }
    }
}

impl std::error::Error for VorgabeFehler {}

/// The loaded and checked settings, the gate's only state (A44). It can only be made by [`Vorgabe::neu`].
#[derive(Clone, Debug)]
pub struct Vorgabe {
    /// The classes, the lowest first; never empty.
    klassen: Vec<String>,
    /// The position of the class for a unit without a class.
    vorgabestufe: usize,
    routen: Vec<Route>,
    kunden: Vec<Kunde>,
    freigaben: Vec<Freigabe>,
}

/// True if `s` has the key form `^[a-z0-9][a-z0-9._-]{0,63}$`.
fn ist_schluessel(s: &str) -> bool {
    let b = s.as_bytes();
    let Some((erstes, rest)) = b.split_first() else {
        return false;
    };
    b.len() <= 64
        && (erstes.is_ascii_lowercase() || erstes.is_ascii_digit())
        && rest.iter().all(|c| {
            c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, b'.' | b'_' | b'-')
        })
}

/// Checks the form of every key and that no key occurs twice.
fn schluessel_pruefen<'a>(schluessel: impl Iterator<Item = &'a str>) -> Result<(), VorgabeFehler> {
    let mut gesehen: Vec<&str> = Vec::new();
    for s in schluessel {
        if !ist_schluessel(s) {
            return Err(VorgabeFehler::SchluesselForm(s.to_owned()));
        }
        if gesehen.contains(&s) {
            return Err(VorgabeFehler::SchluesselDoppelt(s.to_owned()));
        }
        gesehen.push(s);
    }
    Ok(())
}

impl Vorgabe {
    /// Builds the settings and checks section 2 of the contract; the error names the first violation.
    ///
    /// `vorgabestufe` is the class of a unit without a class; `None` means the highest class (A35). The fixed value
    /// `vorgabe: "nie"` of `kundendaten_an_cloud` is not a parameter: it has only this one value (A50).
    ///
    /// # Errors
    ///
    /// Returns the first violation: no class, a key of the wrong form or twice in its list, a `vorgabestufe` or a class
    /// of a route outside `klassen`, a customer list with an unknown route, or a release for an unknown route, a local
    /// route or an unknown customer.
    pub fn neu(
        klassen: Vec<String>,
        vorgabestufe: Option<String>,
        routen: Vec<Route>,
        kunden: Vec<Kunde>,
        freigaben: Vec<Freigabe>,
    ) -> Result<Self, VorgabeFehler> {
        if klassen.is_empty() {
            return Err(VorgabeFehler::KlassenLeer);
        }
        schluessel_pruefen(klassen.iter().map(String::as_str))?;
        let stufe = match vorgabestufe {
            Some(s) => match klassen.iter().position(|k| *k == s) {
                Some(i) => i,
                None => return Err(VorgabeFehler::VorgabestufeUnbekannt(s)),
            },
            None => klassen.len() - 1,
        };
        schluessel_pruefen(routen.iter().map(|r| r.schluessel.as_str()))?;
        for r in &routen {
            if let Some(k) = &r.klasse
                && !klassen.contains(k)
            {
                return Err(VorgabeFehler::RouteKlasseUnbekannt {
                    route: r.schluessel.clone(),
                    klasse: k.clone(),
                });
            }
        }
        schluessel_pruefen(kunden.iter().map(|k| k.schluessel.as_str()))?;
        for k in &kunden {
            for r in k.erlaubt.iter().chain(&k.verboten) {
                if !routen.iter().any(|x| &x.schluessel == r) {
                    return Err(VorgabeFehler::RouteUnbekannt {
                        kunde: k.schluessel.clone(),
                        route: r.clone(),
                    });
                }
            }
        }
        for f in &freigaben {
            match routen.iter().find(|r| r.schluessel == f.route) {
                None => return Err(VorgabeFehler::FreigabeRouteUnbekannt(f.route.clone())),
                Some(r) if r.art == Art::Lokal => {
                    return Err(VorgabeFehler::FreigabeRouteLokal(f.route.clone()));
                }
                Some(_) => {}
            }
            if !kunden.iter().any(|k| k.schluessel == f.kunde) {
                return Err(VorgabeFehler::FreigabeKundeUnbekannt(f.kunde.clone()));
            }
        }
        Ok(Self {
            klassen,
            vorgabestufe: stufe,
            routen,
            kunden,
            freigaben,
        })
    }

    /// The position of a class in `klassen`, the lowest class at 0, compared byte for byte (D14); `None` if the class
    /// is not in `klassen`.
    #[must_use]
    pub fn rang(&self, klasse: &str) -> Option<usize> {
        self.klassen.iter().position(|k| k == klasse)
    }

    /// The position of the highest class.
    fn hoechste(&self) -> usize {
        self.klassen.len() - 1
    }
}

/// The unit as the gate sees it: only the class.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Einheit<'a> {
    /// The class of the unit; `None` means the unit has none (it gets the `vorgabestufe`, rule D5).
    pub klasse: Option<&'a str>,
}

/// The reason of an [`Antwort`], in the order of the rules D2 to D11; `Frei` is D1.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Grund {
    /// D1: the unit may go to the route.
    Frei,
    /// D2: the route is missing or empty.
    RouteFehlt,
    /// D3: the route is not in the settings or has no class.
    RouteOhneKlasse,
    /// D4: the class of the unit is given but is not in `klassen`.
    EinheitOhneKlasse,
    /// D6: the effective class of the unit lies above the class of the route.
    KlasseUeberRoute,
    /// D7: customer data without a customer.
    KundeFehlt,
    /// D8: the customer is not in the settings.
    KundeOhneEintrag,
    /// D9: the customer forbids the route.
    RouteFuerKundeVerboten,
    /// D10: the customer does not allow the route.
    RouteFuerKundeNichtErlaubt,
    /// D11: customer data to a cloud route without a release for this route and customer.
    KundendatenAnCloud,
}

impl Grund {
    /// The code of the reason, in key form (the contract's table, column "Code").
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::Frei => "frei",
            Self::RouteFehlt => "route-fehlt",
            Self::RouteOhneKlasse => "route-ohne-klasse",
            Self::EinheitOhneKlasse => "einheit-ohne-klasse",
            Self::KlasseUeberRoute => "klasse-ueber-route",
            Self::KundeFehlt => "kunde-fehlt",
            Self::KundeOhneEintrag => "kunde-ohne-eintrag",
            Self::RouteFuerKundeVerboten => "route-fuer-kunde-verboten",
            Self::RouteFuerKundeNichtErlaubt => "route-fuer-kunde-nicht-erlaubt",
            Self::KundendatenAnCloud => "kundendaten-an-cloud",
        }
    }
}

/// The answer of the gate.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Antwort {
    /// The reason; `Grund::Frei` means allowed.
    pub grund: Grund,
    /// The effective class of the unit (rule D5), or `None` if the unit has no valid class. It is set for every reason.
    pub klasse: Option<String>,
}

impl Antwort {
    /// True exactly if the reason is `Grund::Frei`.
    #[must_use]
    pub fn darf(&self) -> bool {
        self.grund == Grund::Frei
    }

    /// The fixed message of the reason; `{route}` is the route as it was passed. It never names content, title, path or
    /// customer of the unit.
    #[must_use]
    pub fn meldung(&self, route: Option<&str>) -> String {
        let r = route.unwrap_or_default();
        match self.grund {
            Grund::Frei => "frei".to_owned(),
            Grund::RouteFehlt => "Anfrage ohne Route".to_owned(),
            Grund::RouteOhneKlasse => format!("Route {r} hat keine Klasse"),
            Grund::EinheitOhneKlasse => "Einheit ohne gültige Klasse".to_owned(),
            Grund::KlasseUeberRoute => format!("Klasse der Einheit liegt über Route {r}"),
            Grund::KundeFehlt => "Kundendaten ohne Kunde".to_owned(),
            Grund::KundeOhneEintrag => "Kunde ohne Eintrag in der Kundenvorgabe".to_owned(),
            Grund::RouteFuerKundeVerboten => format!("Route {r} ist für diesen Kunden verboten"),
            Grund::RouteFuerKundeNichtErlaubt => {
                format!("Route {r} ist für diesen Kunden nicht erlaubt")
            }
            Grund::KundendatenAnCloud => format!("Kundendaten an Cloud-Route {r} ohne Freigabe"),
        }
    }
}

/// Decides whether `einheit` may go to `route`; `kunde` is the customer the unit belongs to, or none.
///
/// The route is the key of the session's route, never a value from the agent's request. The rules run in the order of the
/// contract table; the first that applies decides (D12). Customer data are units with a customer or of the highest
/// class; for them a cloud route also needs a release for exactly this route and customer (D11). Keys are compared byte
/// for byte (D14). The function is pure and never panics (D15).
#[must_use]
pub fn darf(
    v: &Vorgabe,
    einheit: &Einheit<'_>,
    route: Option<&str>,
    kunde: Option<&str>,
) -> Antwort {
    let wirksam = wirksamer_rang(v, einheit, kunde);
    let klasse = wirksam.and_then(|w| v.klassen.get(w)).cloned();
    Antwort {
        grund: pruefen(v, wirksam, route, kunde),
        klasse,
    }
}

/// D5, the effective class of `einheit` as a key of `klassen`: a unit with a customer has the highest class, a unit
/// without a class has the `vorgabestufe`, otherwise its own class counts.
///
/// # Errors
///
/// Returns `Grund::EinheitOhneKlasse` if the class of the unit is given but not in `klassen` (D4); this is the only
/// error.
pub fn wirksam<'v>(
    v: &'v Vorgabe,
    einheit: &Einheit<'_>,
    kunde: Option<&str>,
) -> Result<&'v str, Grund> {
    wirksamer_rang(v, einheit, kunde)
        .and_then(|w| v.klassen.get(w))
        .map(String::as_str)
        .ok_or(Grund::EinheitOhneKlasse)
}

/// D5 as a position in `klassen`; `None` for a class that is given but unknown.
fn wirksamer_rang(v: &Vorgabe, einheit: &Einheit<'_>, kunde: Option<&str>) -> Option<usize> {
    let eigene = match einheit.klasse {
        None => Some(v.vorgabestufe),
        Some(k) => v.rang(k),
    };
    eigene.map(|e| if kunde.is_some() { v.hoechste() } else { e })
}

/// The rules D2 to D11 in table order; `wirksam` is the effective class, `None` for a class that is given but unknown.
fn pruefen(v: &Vorgabe, wirksam: Option<usize>, route: Option<&str>, kunde: Option<&str>) -> Grund {
    let route = match route {
        None | Some("") => return Grund::RouteFehlt,
        Some(r) => r,
    };
    let Some(eintrag) = v.routen.iter().find(|r| r.schluessel == route) else {
        return Grund::RouteOhneKlasse;
    };
    let Some(routenrang) = eintrag.klasse.as_deref().and_then(|k| v.rang(k)) else {
        return Grund::RouteOhneKlasse;
    };
    let Some(wirksam) = wirksam else {
        return Grund::EinheitOhneKlasse;
    };
    if wirksam > routenrang {
        return Grund::KlasseUeberRoute;
    }
    if wirksam != v.hoechste() {
        return Grund::Frei;
    }
    let Some(kunde) = kunde else {
        return Grund::KundeFehlt;
    };
    if let Some(g) = kunde_pruefen(v, route, kunde) {
        return g;
    }
    if eintrag.art == Art::Cloud && !a50_freigabe(v, route, kunde) {
        return Grund::KundendatenAnCloud;
    }
    Grund::Frei
}

/// The rules D8 to D10 for customer data of `kunde` on `route`; `None` if none applies.
fn kunde_pruefen(v: &Vorgabe, route: &str, kunde: &str) -> Option<Grund> {
    let Some(k) = v.kunden.iter().find(|k| k.schluessel == kunde) else {
        return Some(Grund::KundeOhneEintrag);
    };
    if k.verboten.iter().any(|r| r == route) {
        return Some(Grund::RouteFuerKundeVerboten);
    }
    if !k.erlaubt.iter().any(|r| r == route) {
        return Some(Grund::RouteFuerKundeNichtErlaubt);
    }
    None
}

/// True if a release of contract 3 exists for exactly this route and customer (D11).
fn a50_freigabe(v: &Vorgabe, route: &str, kunde: &str) -> bool {
    v.freigaben
        .iter()
        .any(|f| f.route == route && f.kunde == kunde)
}
