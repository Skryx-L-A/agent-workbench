//! Class depth in the gate (partial contract `rust/spec/datentor/klassen/VERTRAG.md`): inheritance by place or project
//! (A32), splitting (A33) and lowering by the automatic (A39 to A41), with the log data of a lowering.
//!
//! [`wirksame_klasse`] computes, from the own class, the inherited class, the kind of the unit, the findings of the filter
//! core and the options, the class that then goes to [`darf`](crate::darf). [`ganze_klasse`] computes the class of a unit
//! with parts that stays whole. Both are pure and never panic.

use crate::{Einheit, Vorgabe, ist_schluessel, wirksam};

/// The four kinds of A39: the default of the list "Herabstufbare Arten".
pub const VORGABE_ARTEN: [&str; 4] = [
    "regel",
    "bedienwissen",
    "oeffentliche-quelle",
    "leere-vorlage",
];

/// The never list of A40: fixed and not configurable. A kind of this list in `arten` or `streicht` is a load error (T1).
/// The last eleven kinds came with the addendum of 26.11.2026 (answer of the company to plan 10.7).
pub const NIE_ARTEN: [&str; 26] = [
    "personenbezug",
    "kundendaten",
    "geheimnis",
    "zugangsdaten",
    "vertrag",
    "preis",
    "finanzen",
    "netz-und-system",
    "gespraechsprotokoll",
    "sitzungsprotokoll",
    "entscheidung",
    "plan",
    "organisation",
    "quelltext",
    "entwurf",
    "angebot",
    "schaetzung",
    "kalkulation",
    "ticket",
    "testfall",
    "fehlerbericht",
    "sicherheitsbefund",
    "konfiguration",
    "lizenz",
    "kundenbezug",
    "vertragsregel",
];

/// The kinds that sink only with a verdict of a local model (H9b, addendum 26.11.2026): the filter core cannot see own
/// assessments mixed into a summary of public sources, nor fields of a template that show internal procedures.
pub const NUR_MIT_MODELL: [&str; 2] = ["oeffentliche-quelle", "leere-vorlage"];

/// A group of the lowering options: it can only strike kinds (A41).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Gruppe {
    /// The key of the group.
    pub schluessel: String,
    /// The kinds the group strikes from the company list.
    pub streicht: Vec<String>,
}

/// The first violation found by [`Tiefe::neu`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TiefeFehler {
    /// A kind or a group key does not have the key form.
    SchluesselForm(String),
    /// T1: a kind of the never list stands in `arten` or `streicht`.
    NieArt(String),
    /// A kind stands twice in `arten`.
    ArtDoppelt(String),
    /// A group key occurs twice.
    GruppeDoppelt(String),
}

impl std::fmt::Display for TiefeFehler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SchluesselForm(s) => write!(f, "key {s:?} does not have the key form"),
            Self::NieArt(s) => write!(
                f,
                "kind {s:?} is on the never list and can never be lowered"
            ),
            Self::ArtDoppelt(s) => write!(f, "kind {s:?} occurs twice in arten"),
            Self::GruppeDoppelt(s) => write!(f, "group {s:?} occurs twice"),
        }
    }
}

impl std::error::Error for TiefeFehler {}

/// The checked options of the class depth; made by [`Tiefe::neu`] or [`Tiefe::default`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tiefe {
    aufteilen: bool,
    herabstufen: bool,
    arten: Vec<String>,
    gruppen: Vec<Gruppe>,
}

impl Default for Tiefe {
    /// Splitting on, lowering off, the four kinds of A39, no group.
    fn default() -> Self {
        Self {
            aufteilen: true,
            herabstufen: false,
            arten: VORGABE_ARTEN.iter().map(|a| (*a).to_owned()).collect(),
            gruppen: Vec::new(),
        }
    }
}

/// A kind of `arten` or `streicht`: key form and not on the never list (T1).
fn art_pruefen(art: &str) -> Result<(), TiefeFehler> {
    if !ist_schluessel(art) {
        return Err(TiefeFehler::SchluesselForm(art.to_owned()));
    }
    if NIE_ARTEN.contains(&art) {
        return Err(TiefeFehler::NieArt(art.to_owned()));
    }
    Ok(())
}

impl Tiefe {
    /// Builds the options and checks section 2 with T1; the error names the first violation. A kind of the never list
    /// is refused also with the switch off.
    ///
    /// # Errors
    ///
    /// Returns the first violation: a kind or group key without the key form, a kind of the never list, a kind twice in
    /// `arten` or a group key twice.
    pub fn neu(
        aufteilen: bool,
        herabstufen: bool,
        arten: Vec<String>,
        gruppen: Vec<Gruppe>,
    ) -> Result<Self, TiefeFehler> {
        for (i, a) in arten.iter().enumerate() {
            art_pruefen(a)?;
            if arten[..i].contains(a) {
                return Err(TiefeFehler::ArtDoppelt(a.clone()));
            }
        }
        for (i, g) in gruppen.iter().enumerate() {
            if !ist_schluessel(&g.schluessel) {
                return Err(TiefeFehler::SchluesselForm(g.schluessel.clone()));
            }
            if gruppen[..i].iter().any(|x| x.schluessel == g.schluessel) {
                return Err(TiefeFehler::GruppeDoppelt(g.schluessel.clone()));
            }
            for a in &g.streicht {
                art_pruefen(a)?;
            }
        }
        Ok(Self {
            aufteilen,
            herabstufen,
            arten,
            gruppen,
        })
    }

    /// The option "Aufteilen": a unit with parts gets one class per part (K4).
    #[must_use]
    pub fn aufteilen(&self) -> bool {
        self.aufteilen
    }
}

/// A class field that may hold any JSON value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Angabe<'a> {
    /// Missing or `null`.
    Keine,
    /// A text.
    Text(&'a str),
    /// A value other than a text and `null`; it makes the unit invalid (K1).
    KeinText,
}

/// The findings of the filter core. A check that did not run counts as a hit: the caller sets `true` (fail closed).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "the eight findings of the contract"
)]
pub struct Befund {
    /// A secret was found.
    pub geheimnis: bool,
    /// Personal data were found.
    pub personenbezug: bool,
    /// An internal name was found.
    pub interner_name: bool,
    /// A host name or an address of an own or customer system was found (addendum 26.11.2026).
    pub host: bool,
    /// A path of an own or customer system was found (addendum 26.11.2026).
    pub pfad: bool,
    /// A customer identifier was found: project key, customer number, product name of a customer (addendum 26.11.2026).
    pub kundenkennung: bool,
    /// A configuration, version, licence or access value of a system was found (addendum 26.11.2026).
    pub konfiguration: bool,
    /// The filter core could not check the content completely (addendum 26.11.2026).
    pub zweifel: bool,
}

impl Befund {
    /// Nothing found and nothing in doubt: the only state in which H8 lets a lowering pass.
    #[must_use]
    pub fn frei(&self) -> bool {
        !(self.geheimnis
            || self.personenbezug
            || self.interner_name
            || self.host
            || self.pfad
            || self.kundenkennung
            || self.konfiguration
            || self.zweifel)
    }
}

/// The verdict of a local model.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Modell {
    /// Nothing against lowering.
    Frei,
    /// The model prevents the lowering.
    Verhindert,
    /// The model proposes lowering; it triggers nothing.
    Senken,
}

/// The setting of the owner for the lowering of his units (A41).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NutzerSenkung {
    /// The owner follows the company and group.
    Folgt,
    /// The owner switched the lowering off.
    Aus,
    /// Every lowering needs the owner's confirmation.
    Bestaetigung,
}

/// The place of the whole unit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Lage<'a> {
    /// The class inherited from place or project (A32).
    pub geerbt: Angabe<'a>,
    /// The group of the owner (A41).
    pub gruppe: Option<&'a str>,
    /// The owner's setting (A41).
    pub nutzer: NutzerSenkung,
    /// The owner confirmed this lowering.
    pub bestaetigt: bool,
}

/// The content of a unit or of one part.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Inhalt<'a> {
    /// The own class, set by a person or an admin rule.
    pub klasse: Angabe<'a>,
    /// The customer, as in contract 3.
    pub kunde: Option<&'a str>,
    /// The kind; `None` means not reliably recognised.
    pub art: Option<&'a str>,
    /// The findings; `None` means no finding is present.
    pub befund: Option<Befund>,
    /// The verdict of a local model; `None` means no model ran.
    pub modell: Option<Modell>,
}

/// Where the class comes from (A32).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Herkunft {
    /// The own class.
    Eigen,
    /// The inherited class.
    Geerbt,
    /// The `vorgabestufe`.
    Vorgabestufe,
    /// Lowered by one level below the inherited class.
    Gesenkt,
    /// Raised to the highest class by a customer (K2).
    Kunde,
    /// The highest class of the parts of a whole unit (K4).
    Teile,
    /// No valid class.
    Ungueltig,
}

impl Herkunft {
    /// The code of the origin.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::Eigen => "eigen",
            Self::Geerbt => "geerbt",
            Self::Vorgabestufe => "vorgabestufe",
            Self::Gesenkt => "gesenkt",
            Self::Kunde => "kunde",
            Self::Teile => "teile",
            Self::Ungueltig => "ungueltig",
        }
    }
}

/// The reason of the lowering: H0 to H11 with H9b, an invalid class or a whole unit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Senkung {
    /// H0: lowered by one level.
    Gesenkt,
    /// H1: the switch is off.
    Aus,
    /// H2: the class is not inherited.
    NichtGeerbt,
    /// H3: customer data.
    Kundendaten,
    /// H4: the inherited class is the lowest.
    Niedrigste,
    /// H5: the kind is not reliably recognised.
    ArtUnbekannt,
    /// H6: the group has no entry.
    GruppeOhneEintrag,
    /// H7: the kind may not be lowered.
    ArtNichtHerabstufbar,
    /// H8: no finding or a finding is true.
    Befund,
    /// H9: the local model prevents it.
    ModellVerhindert,
    /// H9b: the kind sinks only with a verdict of a local model, and none ran.
    ModellFehlt,
    /// H10: the owner switched it off.
    NutzerAus,
    /// H11: the confirmation is missing.
    BestaetigungFehlt,
    /// K1: the class is invalid.
    KlasseUngueltig,
    /// K4: the unit stays whole.
    NichtAufgeteilt,
}

impl Senkung {
    /// The code of the reason.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::Gesenkt => "gesenkt",
            Self::Aus => "aus",
            Self::NichtGeerbt => "nicht-geerbt",
            Self::Kundendaten => "kundendaten",
            Self::Niedrigste => "niedrigste",
            Self::ArtUnbekannt => "art-unbekannt",
            Self::GruppeOhneEintrag => "gruppe-ohne-eintrag",
            Self::ArtNichtHerabstufbar => "art-nicht-herabstufbar",
            Self::Befund => "befund",
            Self::ModellVerhindert => "modell-verhindert",
            Self::ModellFehlt => "modell-fehlt",
            Self::NutzerAus => "nutzer-aus",
            Self::BestaetigungFehlt => "bestaetigung-fehlt",
            Self::KlasseUngueltig => "klasse-ungueltig",
            Self::NichtAufgeteilt => "nicht-aufgeteilt",
        }
    }
}

/// The level of the rule that lowered.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ebene {
    /// The company list.
    Firma,
    /// The company list with the strikes of the owner's group.
    Gruppe,
}

/// The log data of a lowering (A39, A104); they name no path, title, name or customer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Senkungsangaben {
    /// The kind of the unit.
    pub art: String,
    /// How it was recognised, in fixed order.
    pub erkannt_durch: Vec<String>,
    /// The level of the rule.
    pub regel_ebene: Ebene,
    /// The inherited class.
    pub klasse_alt: String,
    /// The lowered class.
    pub klasse_neu: String,
}

/// The result of [`wirksame_klasse`] or [`ganze_klasse`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Klassenergebnis {
    /// The class that goes to `darf`, or `None` if it is invalid.
    pub klasse: Option<String>,
    /// The customer that goes to `darf`.
    pub kunde: Option<String>,
    /// Where the class comes from.
    pub herkunft: Herkunft,
    /// The reason of the lowering.
    pub senkung: Senkung,
    /// Only at `BestaetigungFehlt`: the class one level below the inherited one.
    pub vorschlag: Option<String>,
    /// Only at `Gesenkt`: the log data.
    pub protokoll: Option<Senkungsangaben>,
}

impl Klassenergebnis {
    /// K6: the unit for `darf`; an invalid class becomes the empty text, which `darf` blocks.
    #[must_use]
    pub fn einheit(&self) -> Einheit<'_> {
        Einheit {
            klasse: Some(self.klasse.as_deref().unwrap_or_default()),
        }
    }

    /// The result for an invalid class.
    fn ungueltig(kunde: Option<&str>) -> Self {
        Self {
            klasse: None,
            kunde: kunde.map(str::to_owned),
            herkunft: Herkunft::Ungueltig,
            senkung: Senkung::KlasseUngueltig,
            vorschlag: None,
            protokoll: None,
        }
    }
}

/// The position of a class field: `Ok(None)` if missing, `Err` if it is no text of `klassen`.
fn angabe_rang(v: &Vorgabe, a: Angabe<'_>) -> Result<Option<usize>, ()> {
    match a {
        Angabe::Keine => Ok(None),
        Angabe::Text(t) => v.rang(t).map(Some).ok_or(()),
        Angabe::KeinText => Err(()),
    }
}

/// K1: the base class as a position and its origin; `Err` if the own or the inherited class is invalid.
fn grundklasse(v: &Vorgabe, lage: &Lage<'_>, inhalt: &Inhalt<'_>) -> Result<(usize, Herkunft), ()> {
    let eigen = angabe_rang(v, inhalt.klasse)?;
    let geerbt = angabe_rang(v, lage.geerbt)?;
    Ok(match (eigen, geerbt) {
        (Some(e), _) => (e, Herkunft::Eigen),
        (None, Some(g)) => (g, Herkunft::Geerbt),
        (None, None) => (v.vorgabestufe, Herkunft::Vorgabestufe),
    })
}

/// K3: the reason of the lowering by H1 to H11 in table order (H9b after H9), H0 otherwise; `i` is the base class.
fn senkgrund(
    v: &Vorgabe,
    t: &Tiefe,
    lage: &Lage<'_>,
    inhalt: &Inhalt<'_>,
    stufe: usize,
    herkunft: Herkunft,
) -> Senkung {
    if !t.herabstufen {
        return Senkung::Aus;
    }
    if herkunft != Herkunft::Geerbt {
        return Senkung::NichtGeerbt;
    }
    if inhalt.kunde.is_some() || stufe == v.hoechste() {
        return Senkung::Kundendaten;
    }
    if stufe == 0 {
        return Senkung::Niedrigste;
    }
    let Some(art) = inhalt.art else {
        return Senkung::ArtUnbekannt;
    };
    let gruppe = match lage.gruppe {
        None => None,
        Some(g) => match t.gruppen.iter().find(|x| x.schluessel == g) {
            None => return Senkung::GruppeOhneEintrag,
            Some(x) => Some(x),
        },
    };
    if !t.arten.iter().any(|a| a == art)
        || gruppe.is_some_and(|g| g.streicht.iter().any(|a| a == art))
    {
        return Senkung::ArtNichtHerabstufbar;
    }
    if !inhalt.befund.is_some_and(|b| b.frei()) {
        return Senkung::Befund;
    }
    if inhalt.modell == Some(Modell::Verhindert) {
        return Senkung::ModellVerhindert;
    }
    if inhalt.modell.is_none() && NUR_MIT_MODELL.contains(&art) {
        return Senkung::ModellFehlt;
    }
    match lage.nutzer {
        NutzerSenkung::Aus => Senkung::NutzerAus,
        NutzerSenkung::Bestaetigung if !lage.bestaetigt => Senkung::BestaetigungFehlt,
        _ => Senkung::Gesenkt,
    }
}

/// K1 to K3 and K5 for one unit without parts, or for one part with the place of the whole unit.
///
/// The class after K3 and the customer go through [`wirksam`] (K2); an error there gives the origin `Ungueltig`. The
/// caller then calls `darf(v, &ergebnis.einheit(), route, ergebnis.kunde.as_deref())`. The function knows no earlier run:
/// the same call always gives the same class, never lower than one level below the inherited class.
#[must_use]
pub fn wirksame_klasse(
    v: &Vorgabe,
    t: &Tiefe,
    lage: &Lage<'_>,
    inhalt: &Inhalt<'_>,
) -> Klassenergebnis {
    let Ok((i, mut herkunft)) = grundklasse(v, lage, inhalt) else {
        return Klassenergebnis::ungueltig(inhalt.kunde);
    };
    let senkung = senkgrund(v, t, lage, inhalt, i, herkunft);
    let unter = i.checked_sub(1).and_then(|u| v.klassen.get(u));
    let mut stufe = i;
    let mut vorschlag = None;
    let mut protokoll = None;
    match (senkung, unter) {
        (Senkung::BestaetigungFehlt, Some(u)) => vorschlag = Some(u.clone()),
        (Senkung::Gesenkt, Some(u)) => {
            stufe -= 1;
            herkunft = Herkunft::Gesenkt;
            let art = inhalt.art.unwrap_or_default();
            let mut erkannt_durch = vec![format!("typ:{art}"), "filter:frei".to_owned()];
            if matches!(inhalt.modell, Some(Modell::Frei | Modell::Senken)) {
                erkannt_durch.push("modell:frei".to_owned());
            }
            protokoll = Some(Senkungsangaben {
                art: art.to_owned(),
                erkannt_durch,
                regel_ebene: if lage.gruppe.is_some() {
                    Ebene::Gruppe
                } else {
                    Ebene::Firma
                },
                klasse_alt: v.klassen.get(i).cloned().unwrap_or_default(),
                klasse_neu: u.clone(),
            });
        }
        _ => {}
    }
    let name = v.klassen.get(stufe).map_or("", String::as_str);
    let Ok(klasse) = wirksam(v, &Einheit { klasse: Some(name) }, inhalt.kunde) else {
        return Klassenergebnis::ungueltig(inhalt.kunde);
    };
    if inhalt.kunde.is_some() {
        herkunft = Herkunft::Kunde;
    }
    Klassenergebnis {
        klasse: Some(klasse.to_owned()),
        kunde: inhalt.kunde.map(str::to_owned),
        herkunft,
        senkung,
        vorschlag,
        protokoll,
    }
}

/// K4 with splitting off: the unit stays whole in the highest class of its parts after K1 and K2; nothing is lowered.
///
/// If one part is invalid, or there is no part, the whole unit is invalid. The customer is that of the parts if all
/// parts with a customer name the same one; if they name different ones it is the empty text, which `darf` blocks.
#[must_use]
pub fn ganze_klasse(v: &Vorgabe, lage: &Lage<'_>, teile: &[Inhalt<'_>]) -> Klassenergebnis {
    let mut kunden: Vec<&str> = Vec::new();
    for k in teile.iter().filter_map(|x| x.kunde) {
        if !kunden.contains(&k) {
            kunden.push(k);
        }
    }
    let kunde = match kunden.as_slice() {
        [] => None,
        [k] => Some(*k),
        _ => Some(""),
    };
    let mut hoechste: Option<usize> = None;
    for x in teile {
        let Ok((i, _)) = grundklasse(v, lage, x) else {
            return Klassenergebnis::ungueltig(kunde);
        };
        let stufe = if x.kunde.is_some() { v.hoechste() } else { i };
        hoechste = hoechste.max(Some(stufe));
    }
    let Some(klasse) = hoechste.and_then(|h| v.klassen.get(h)) else {
        return Klassenergebnis::ungueltig(kunde);
    };
    Klassenergebnis {
        klasse: Some(klasse.clone()),
        kunde: kunde.map(str::to_owned),
        herkunft: Herkunft::Teile,
        senkung: Senkung::NichtAufgeteilt,
        vorschlag: None,
        protokoll: None,
    }
}
