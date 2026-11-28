//! `setzen` and `sicht` (contract, sections 15.3 to 15.6 and 15.11): one setting is checked and taken or refused with
//! exactly one fixed code, and every check gives one audit line; the view gives per key of the catalogue the effective
//! value, its origin, whether the viewer may change it and why not.

use std::collections::BTreeMap;

use serde_json::{Map, Value};

use crate::katalog::{self, Eintrag, KATALOG, Platz, Richtung, Typ};

/// The fixed codes of `setzen` (15.4). The order of the variants is the order of the stages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Code {
    /// Stage 1: the key is no text or not in the catalogue.
    EinstellungUnbekannt,
    /// Stage 2: the area is not the area of the key.
    BereichFalsch,
    /// Stage 3: the level has none of the three forms.
    EbeneUnbekannt,
    /// Stage 4: the group is not in `gruppen` of the company.
    GruppeUnbekannt,
    /// Stage 5: the key may not stand on this level.
    EbeneGesperrt,
    /// Stage 6: level `firma` or a group, and the actor is no admin.
    NichtBefugt,
    /// Stage 7: the value violates type, values or limits.
    WertUngueltig,
    /// Stage 8: the value is looser than above.
    Lockerung,
    /// Stage 9: harness or model is in no class open for choice.
    AusserGrenze,
}

impl Code {
    /// The fixed text of the code.
    #[must_use]
    pub fn text(self) -> &'static str {
        match self {
            Code::EinstellungUnbekannt => "ebenen.einstellung_unbekannt",
            Code::BereichFalsch => "ebenen.bereich_falsch",
            Code::EbeneUnbekannt => "ebenen.ebene_unbekannt",
            Code::GruppeUnbekannt => "ebenen.gruppe_unbekannt",
            Code::EbeneGesperrt => "ebenen.ebene_gesperrt",
            Code::NichtBefugt => "ebenen.nicht_befugt",
            Code::WertUngueltig => "ebenen.wert_ungueltig",
            Code::Lockerung => "ebenen.lockerung",
            Code::AusserGrenze => "ebenen.ausser_grenze",
        }
    }

    /// Every code, in the order of the stages.
    pub const ALLE: [Code; 9] = [
        Code::EinstellungUnbekannt,
        Code::BereichFalsch,
        Code::EbeneUnbekannt,
        Code::GruppeUnbekannt,
        Code::EbeneGesperrt,
        Code::NichtBefugt,
        Code::WertUngueltig,
        Code::Lockerung,
        Code::AusserGrenze,
    ];
}

/// The values per place (15.3): a missing key means that no value stands on that level.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Werte {
    /// The level `firma`.
    pub firma: Map<String, Value>,
    /// Per key of a group, the level of the group.
    pub gruppen: BTreeMap<String, Map<String, Value>>,
    /// Per pseudonym, the level of the person.
    pub nutzer: BTreeMap<String, Map<String, Value>>,
}

/// The state that `setzen` and `sicht` see (15.3).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Stand {
    /// The classes of the company file, the lowest first.
    pub klassen: Vec<String>,
    /// The pseudonyms of the admins; the service builds them from the list of admin accounts (15.2).
    pub admins: Vec<String>,
    /// The groups of the company file with their members (pseudonyms), in the order of the file.
    pub gruppen: Vec<(String, Vec<String>)>,
    /// The display names of the groups (key of the group, name), from `gruppennamen` of the company file (3.1); empty without
    /// names. `setzen` and `sicht` do not read them (15.3).
    pub gruppennamen: Vec<(String, String)>,
    /// The values per place.
    pub werte: Werte,
}

fn texte(w: &Value) -> Option<Vec<String>> {
    w.as_array()?
        .iter()
        .map(|x| x.as_str().map(str::to_owned))
        .collect()
}

fn plaetze(w: &Value) -> Option<BTreeMap<String, Map<String, Value>>> {
    w.as_object()?
        .iter()
        .map(|(k, v)| Some((k.clone(), v.as_object()?.clone())))
        .collect()
}

impl Stand {
    /// The state from its JSON form in 15.3 (`klassen`, `admins`, `gruppen`, the optional `gruppennamen`, `werte` with
    /// `firma`, `gruppen` and `nutzer`). `None` if a part has another type, or if `gruppennamen` is not valid after 3.1
    /// ([`katalog::gruppennamen_gut`]: a name without a group, a name twice). The values themselves are not checked here.
    #[must_use]
    pub fn aus_wert(w: &Value) -> Option<Stand> {
        let werte = w.get("werte")?;
        let gruppen: Vec<(String, Vec<String>)> = w
            .get("gruppen")?
            .as_object()?
            .iter()
            .map(|(g, m)| Some((g.clone(), texte(m)?)))
            .collect::<Option<_>>()?;
        let gruppennamen: Vec<(String, String)> = match w.get("gruppennamen") {
            None => Vec::new(),
            Some(n) => n
                .as_object()?
                .iter()
                .map(|(g, v)| Some((g.clone(), v.as_str()?.to_owned())))
                .collect::<Option<_>>()?,
        };
        if !katalog::gruppennamen_gut(&gruppennamen, &gruppen) {
            return None;
        }
        Some(Stand {
            klassen: texte(w.get("klassen")?)?,
            admins: texte(w.get("admins")?)?,
            gruppen,
            gruppennamen,
            werte: Werte {
                firma: werte.get("firma")?.as_object()?.clone(),
                gruppen: plaetze(werte.get("gruppen")?)?,
                nutzer: plaetze(werte.get("nutzer")?)?,
            },
        })
    }

    /// `true` if `wer` is in `admins`.
    #[must_use]
    pub fn ist_admin(&self, wer: &str) -> bool {
        self.admins.iter().any(|a| a == wer)
    }

    /// The key of the first group that lists `wer`.
    #[must_use]
    pub fn gruppe_des(&self, wer: &str) -> Option<&str> {
        self.gruppen
            .iter()
            .find(|(_, m)| m.iter().any(|x| x == wer))
            .map(|(g, _)| g.as_str())
    }

    /// The display name of the group `g`, `None` without one (3.1, `gruppennamen`).
    #[must_use]
    pub fn gruppenname(&self, g: &str) -> Option<&str> {
        self.gruppennamen
            .iter()
            .find(|(k, _)| k == g)
            .map(|(_, n)| n.as_str())
    }

    /// The names for the message `Einstellungen` (15.10, proposal): for an admin every group with a name, for another viewer only
    /// the group in which `wer` stands (if it has a name). A group without a name is not in the list; the view then shows the key.
    #[must_use]
    pub fn gruppennamen_fuer(&self, wer: &str) -> Vec<(&str, &str)> {
        if self.ist_admin(wer) {
            return self
                .gruppennamen
                .iter()
                .map(|(g, n)| (g.as_str(), n.as_str()))
                .collect();
        }
        self.gruppe_des(wer)
            .and_then(|g| self.gruppenname(g).map(|n| (g, n)))
            .into_iter()
            .collect()
    }

    fn hat_gruppe(&self, g: &str) -> bool {
        self.gruppen.iter().any(|(x, _)| x == g)
    }
}

/// One audit line of a check (15.6, in the form of contract 5, section 6.6). `antrag`, `pflicht` and `zweiter` are
/// always `null`; chain, `fassung`, `zeit` and `vorher` are set by the writer of contract 5.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Zeile {
    /// `firma`, `gruppe` or `nutzer`; `None` if the level has none of the three forms.
    pub ebene: Option<&'static str>,
    /// The key of the group, the pseudonym of the actor at `nutzer`, else `None`.
    pub ziel: Option<String>,
    /// The pseudonym of the actor.
    pub handelnd: String,
    /// sha256 over RFC 8785 of `{"schluessel", "wert"}` of the old value of the place, `None` without one.
    pub alt: Option<String>,
    /// sha256 over RFC 8785 of `{"schluessel", "wert"}` of the submitted value.
    pub neu: String,
    /// The code, `None` when taken.
    pub code: Option<&'static str>,
}

impl Zeile {
    /// The same line with another code, for a refusal after `setzen` (stages 0a to 4 of 15.12.3, a code
    /// `firmenvorgabe.*` of the service or the write helper).
    #[must_use]
    pub fn mit_code(self, code: &'static str) -> Zeile {
        Zeile {
            code: Some(code),
            ..self
        }
    }

    /// The fields of the line as a JSON object.
    #[must_use]
    pub fn als_wert(&self) -> Value {
        let text = |t: Option<&str>| t.map_or(Value::Null, |t| Value::String(t.to_owned()));
        let (aktion, ergebnis) = if self.code.is_some() {
            ("regel.abgelehnt", "abgelehnt")
        } else {
            ("regel.geaendert", "geaendert")
        };
        serde_json::json!({
            "aktionsname": aktion,
            "alt": text(self.alt.as_deref()),
            "antrag": null,
            "code": text(self.code),
            "ebene": text(self.ebene),
            "ergebnis": ergebnis,
            "handelnd": self.handelnd,
            "neu": self.neu,
            "pflicht": null,
            "ziel": text(self.ziel.as_deref()),
            "zweiter": null,
        })
    }
}

/// The outcome of `setzen`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pruefung {
    /// The code of the first failed stage, `None` when taken.
    pub code: Option<Code>,
    /// The audit line, for a refusal as well.
    pub zeile: Zeile,
}

impl Pruefung {
    /// `true` when taken.
    #[must_use]
    pub fn angenommen(&self) -> bool {
        self.code.is_none()
    }

    /// The result as in `erwartet-setzen/`: `{"angenommen", "code", "zeile"}`.
    #[must_use]
    pub fn als_wert(&self) -> Value {
        serde_json::json!({
            "angenommen": self.angenommen(),
            "code": self.code.map(Code::text),
            "zeile": self.zeile.als_wert(),
        })
    }
}

/// Why the viewer may not change a key, or may only make it stricter (15.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Grund {
    /// Only stricter than above.
    NurStrenger,
    /// Only an admin changes it.
    NurAdmin,
    /// Nobody changes it in the pilot.
    Fest,
}

impl Grund {
    /// The fixed text of the reason.
    #[must_use]
    pub fn text(self) -> &'static str {
        match self {
            Grund::NurStrenger => "nur_strenger",
            Grund::NurAdmin => "nur_admin",
            Grund::Fest => "fest",
        }
    }
}

/// One entry of the view (15.5).
#[derive(Debug, Clone, PartialEq)]
pub struct Sichteintrag {
    /// The effective value.
    pub wert: Value,
    /// `firma`, `gruppe <schluessel>` or `nutzer`.
    pub herkunft: String,
    /// Whether the viewer may change the key.
    pub aenderbar: bool,
    /// The reason of a lock, `None` without one.
    pub grund: Option<Grund>,
}

/// The view: one entry per key of the catalogue, in its order.
#[derive(Debug, Clone, PartialEq)]
pub struct Einstellungen(pub Vec<(&'static str, Sichteintrag)>);

impl Einstellungen {
    /// The entry of `schluessel`.
    #[must_use]
    pub fn eintrag(&self, schluessel: &str) -> Option<&Sichteintrag> {
        self.0
            .iter()
            .find(|(s, _)| *s == schluessel)
            .map(|(_, e)| e)
    }

    /// The view as the object `werte` of the message `Einstellungen` (15.10).
    #[must_use]
    pub fn als_wert(&self) -> Value {
        Value::Object(
            self.0
                .iter()
                .map(|(s, e)| {
                    (
                        (*s).to_owned(),
                        serde_json::json!({
                            "wert": e.wert,
                            "herkunft": e.herkunft,
                            "aenderbar": e.aenderbar,
                            "grund": e.grund.map(Grund::text),
                        }),
                    )
                })
                .collect(),
        )
    }
}

/// The place of a change: its kind and its target (the key of the group or the pseudonym of the person).
struct Ziel<'a> {
    art: Platz,
    ziel: Option<&'a str>,
}

fn ziel_von<'a>(ebene: &'a Value, wer: &'a str) -> Option<Ziel<'a>> {
    match ebene.as_str()? {
        "firma" => Some(Ziel {
            art: Platz::Firma,
            ziel: None,
        }),
        "nutzer" => Some(Ziel {
            art: Platz::Nutzer,
            ziel: Some(wer),
        }),
        t => {
            let g = t.strip_prefix("gruppe ")?;
            katalog::ist_kennung(g).then_some(Ziel {
                art: Platz::Gruppe,
                ziel: Some(g),
            })
        }
    }
}

/// The levels from the place `firma` down to the place `art` of `ziel`, each with its origin and its values.
fn kette<'s>(
    stand: &'s Stand,
    art: Platz,
    ziel: Option<&str>,
) -> Vec<(String, Option<&'s Map<String, Value>>)> {
    let w = &stand.werte;
    let mut k = vec![("firma".to_owned(), Some(&w.firma))];
    let gruppe = |g: &str| (format!("gruppe {g}"), w.gruppen.get(g));
    match (art, ziel) {
        (Platz::Gruppe, Some(g)) => k.push(gruppe(g)),
        (Platz::Nutzer, Some(n)) => {
            if let Some(g) = stand.gruppe_des(n) {
                k.push(gruppe(g));
            }
            k.push(("nutzer".to_owned(), w.nutzer.get(n)));
        }
        _ => {}
    }
    k
}

/// Truth of a value as the reference reads it (`wert and neu`): `false`, `null`, `0`, `""`, `[]` and `{}` are false.
fn wahr(w: &Value) -> bool {
    match w {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().is_some_and(|z| z != 0.0),
        Value::String(s) => !s.is_empty(),
        Value::Array(l) => !l.is_empty(),
        Value::Object(o) => !o.is_empty(),
    }
}

/// The lists per class of the level `firma`. An entry that is no object restricts every class to nothing (fail
/// closed); `setzen` never stores one.
fn schnitt_der_firma(neu: &Value, klassen: &[String]) -> Value {
    if neu.is_object() {
        return neu.clone();
    }
    Value::Object(
        klassen
            .iter()
            .map(|k| (k.clone(), Value::Array(Vec::new())))
            .collect(),
    )
}

/// The intersection of the lists per class `wert` with the entry `neu` of a lower level: a class of `neu` that is in
/// `wert` keeps only the elements of both, a class that is not in `wert` takes the list of `neu`. A list that is no
/// list counts as the empty list (fail closed).
fn schneiden(wert: &mut Map<String, Value>, neu: &Value, klassen: &[String]) {
    let Some(neu) = neu.as_object() else {
        for k in klassen {
            wert.insert(k.clone(), Value::Array(Vec::new()));
        }
        return;
    };
    for (k, liste) in neu {
        let liste = liste.as_array().map_or(&[][..], Vec::as_slice);
        match wert.get_mut(k) {
            Some(Value::Array(oben)) => oben.retain(|x| liste.contains(x)),
            Some(anders) => *anders = Value::Array(Vec::new()),
            None => {
                wert.insert(k.clone(), Value::Array(liste.to_vec()));
            }
        }
    }
}

/// The effective value of `e` and its origin over `kette` (the highest level first), section 5.2. The default of the
/// catalogue counts as `firma`. A free choice takes the origin of the lowest level with a value; the other directions
/// change the origin only where the value really changes.
fn zusammen(
    e: &Eintrag,
    kette: &[(String, Option<&Map<String, Value>>)],
    klassen: &[String],
) -> (Value, String) {
    let mut wert = e.vorgabe.wert();
    let mut herkunft = "firma".to_owned();
    for (name, werte) in kette {
        let Some(neu) = werte.and_then(|w| w.get(e.schluessel)) else {
            continue;
        };
        let firma = name == "firma";
        let vorher = wert.clone();
        match e.richtung {
            Richtung::Fest => {}
            Richtung::Frei => {
                wert = neu.clone();
                herkunft.clone_from(name);
            }
            Richtung::NurFirma => {
                if firma {
                    wert = neu.clone();
                }
            }
            Richtung::AusStrenger => {
                if firma || wahr(&wert) {
                    wert = neu.clone();
                }
            }
            Richtung::Schnitt => {
                if firma {
                    wert = schnitt_der_firma(neu, klassen);
                } else if let Value::Object(o) = &mut wert {
                    schneiden(o, neu, klassen);
                }
            }
        }
        if wert != vorher {
            herkunft.clone_from(name);
        }
    }
    if e.richtung == Richtung::Schnitt
        && let Value::Object(o) = &wert
    {
        wert = Value::Object(
            klassen
                .iter()
                .filter_map(|k| o.get(k).map(|l| (k.clone(), l.clone())))
                .collect(),
        );
    }
    (wert, herkunft)
}

/// `true` if `neu` is looser than `oben` (section 5.2); only the directions `aus_strenger` and `schnitt` can loosen.
fn ist_lockerung(e: &Eintrag, oben: &Value, neu: &Value) -> bool {
    match e.richtung {
        Richtung::AusStrenger => *neu == Value::Bool(true) && *oben == Value::Bool(false),
        Richtung::Schnitt => {
            let (Some(oben), Some(neu)) = (oben.as_object(), neu.as_object()) else {
                return false;
            };
            neu.iter().any(|(k, liste)| {
                oben.get(k).is_some_and(|o| {
                    let o = o.as_array().map_or(&[][..], Vec::as_slice);
                    liste
                        .as_array()
                        .is_some_and(|l| l.iter().any(|x| !o.contains(x)))
                })
            })
        }
        _ => false,
    }
}

fn pruefsumme(schluessel: &Value, wert: &Value) -> String {
    let objekt = serde_json::json!({"schluessel": schluessel, "wert": wert});
    // A JSON value of serde_json holds no number that is not finite, so `kanonisch` cannot fail here.
    let text = formate::kette::kanonisch(&objekt).unwrap_or_default();
    formate::kette::sha256_hex(text.as_bytes())
}

fn stufen(
    stand: &Stand,
    wer: &str,
    bereich: &Value,
    ziel: Option<&Ziel<'_>>,
    schluessel: &Value,
    wert: &Value,
) -> Result<(), Code> {
    let e = schluessel
        .as_str()
        .and_then(katalog::eintrag)
        .ok_or(Code::EinstellungUnbekannt)?;
    if bereich.as_str() != Some(e.bereich.text()) {
        return Err(Code::BereichFalsch);
    }
    let ziel = ziel.ok_or(Code::EbeneUnbekannt)?;
    if ziel.art == Platz::Gruppe && !ziel.ziel.is_some_and(|g| stand.hat_gruppe(g)) {
        return Err(Code::GruppeUnbekannt);
    }
    if !e.erlaubt_auf(ziel.art) {
        return Err(Code::EbeneGesperrt);
    }
    if ziel.art != Platz::Nutzer && !stand.ist_admin(wer) {
        return Err(Code::NichtBefugt);
    }
    // `leseklassen` knows no `null`, not even as a withdrawal: "nobody reads" is the empty list (15.11).
    let ruecknahme = wert.is_null() && e.typ != Typ::Leseklassen;
    if !ruecknahme && !katalog::wert_gueltig(e, wert, &stand.klassen, &stand.gruppen) {
        return Err(Code::WertUngueltig);
    }
    if wert.is_null() {
        return Ok(());
    }
    let bis_hier = kette(stand, ziel.art, ziel.ziel);
    if ziel.art != Platz::Firma {
        let (oben, _) = zusammen(e, &bis_hier[..bis_hier.len() - 1], &stand.klassen);
        if ist_lockerung(e, &oben, wert) {
            return Err(Code::Lockerung);
        }
    }
    let grenze = match e.schluessel {
        "sitzung.harness" => katalog::eintrag("harnesses"),
        "sitzung.modell" => katalog::eintrag("modelle"),
        _ => None,
    };
    if let Some(g) = grenze {
        let (grenze, _) = zusammen(g, &bis_hier, &stand.klassen);
        let offen = stand.klassen.iter().any(|k| {
            grenze
                .get(k)
                .is_none_or(|l| l.as_array().is_some_and(|l| l.contains(wert)))
        });
        if !offen {
            return Err(Code::AusserGrenze);
        }
    }
    Ok(())
}

/// Checks one setting and takes it or refuses it with exactly one code, the one of the first failed stage (15.4).
///
/// `wer` is the pseudonym of the actor, measured by the service (15.2); `bereich`, `ebene`, `schluessel` and `wert`
/// are the fields of `EinstellungSetzen` unchanged (15.9). A refusal leaves `stand` unchanged; a taken change writes
/// `wert` to its place or removes the entry at `null`. Every check gives one audit line (15.6).
pub fn setzen(
    stand: &mut Stand,
    wer: &str,
    bereich: &Value,
    ebene: &Value,
    schluessel: &Value,
    wert: &Value,
) -> Pruefung {
    let ziel = ziel_von(ebene, wer);
    let platz = ziel.as_ref().and_then(|z| match (z.art, z.ziel) {
        (Platz::Firma, _) => Some(&stand.werte.firma),
        (Platz::Gruppe, Some(g)) => stand.werte.gruppen.get(g),
        (Platz::Nutzer, Some(n)) => stand.werte.nutzer.get(n),
        _ => None,
    });
    let alt = schluessel
        .as_str()
        .and_then(|s| platz.and_then(|p| p.get(s)))
        .map(|a| pruefsumme(schluessel, a));
    let code = stufen(stand, wer, bereich, ziel.as_ref(), schluessel, wert).err();
    let zeile = Zeile {
        ebene: ziel.as_ref().map(|z| z.art.text()),
        ziel: ziel.as_ref().and_then(|z| z.ziel).map(str::to_owned),
        handelnd: wer.to_owned(),
        alt,
        neu: pruefsumme(schluessel, wert),
        code: code.map(Code::text),
    };
    if code.is_none()
        && let (Some(z), Some(s)) = (ziel.as_ref(), schluessel.as_str())
    {
        let w = &mut stand.werte;
        let ort = match (z.art, z.ziel) {
            (Platz::Gruppe, Some(g)) => w.gruppen.entry(g.to_owned()).or_default(),
            (Platz::Nutzer, Some(n)) => w.nutzer.entry(n.to_owned()).or_default(),
            _ => &mut w.firma,
        };
        if wert.is_null() {
            ort.remove(s);
        } else {
            ort.insert(s.to_owned(), wert.clone());
        }
    }
    Pruefung { code, zeile }
}

/// The view for `wer` (15.5): per key of the catalogue the effective value over the company, the group of `wer` and
/// the level of `wer`, its origin, whether `wer` may change it, and the reason of a lock. For `leseklassen` a viewer who
/// is no admin sees only the entries that name him: his own and the one of his group, in the order of the list.
#[must_use]
pub fn sicht(stand: &Stand, wer: &str) -> Einstellungen {
    let admin = stand.ist_admin(wer);
    let k = kette(stand, Platz::Nutzer, Some(wer));
    let gruppe = stand.gruppe_des(wer);
    Einstellungen(
        KATALOG
            .iter()
            .map(|e| {
                let (mut wert, herkunft) = zusammen(e, &k, &stand.klassen);
                let (aenderbar, grund) = if e.richtung == Richtung::Fest {
                    (false, Some(Grund::Fest))
                } else if admin || e.bereich == katalog::Bereich::Nutzer {
                    (true, None)
                } else if !e.erlaubt_auf(Platz::Nutzer)
                    || (e.richtung == Richtung::AusStrenger && wert == Value::Bool(false))
                {
                    (false, Some(Grund::NurAdmin))
                } else {
                    (true, Some(Grund::NurStrenger))
                };
                if e.typ == Typ::Leseklassen
                    && !admin
                    && let Value::Array(l) = &mut wert
                {
                    l.retain(|x| match x.get("ebene").and_then(Value::as_str) {
                        Some("nutzer") => x.get("wer").and_then(Value::as_str) == Some(wer),
                        Some("gruppe") => {
                            gruppe.is_some_and(|g| x.get("wer").and_then(Value::as_str) == Some(g))
                        }
                        _ => false,
                    });
                }
                (
                    e.schluessel,
                    Sichteintrag {
                        wert,
                        herkunft,
                        aenderbar,
                        grund,
                    },
                )
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const U: &str = "ps_00000000000000000000000000000c01";
    const A: &str = "ps_00000000000000000000000000000b01";

    #[allow(clippy::needless_pass_by_value)]
    fn stand(firma: Value, gruppe: Value, nutzer: Value) -> Stand {
        Stand::aus_wert(&json!({
            "klassen": ["oeffentlich", "intern"], "admins": [A], "gruppen": {"g-eins": [U]},
            "werte": {"firma": firma, "gruppen": {"g-eins": gruppe}, "nutzer": {U: nutzer}}
        }))
        .expect("state")
    }

    fn stand_mit_namen(namen: &Value) -> Option<Stand> {
        Stand::aus_wert(&json!({
            "klassen": ["oeffentlich", "intern"], "admins": [A],
            "gruppen": {"g-eins": [U], "g-zwei": ["ps_00000000000000000000000000000c02"], "g-drei": []},
            "gruppennamen": namen,
            "werte": {"firma": {}, "gruppen": {"g-eins": {"terminal.schriftgroesse": 15}}, "nutzer": {}}
        }))
    }

    #[test]
    fn gruppennamen_im_stand() {
        let namen = json!({"g-eins": "Gruppe Entwicklung", "g-zwei": "Vertrieb"});
        let s = stand_mit_namen(&namen).expect("state");
        assert_eq!(s.gruppenname("g-eins"), Some("Gruppe Entwicklung"));
        assert_eq!(s.gruppenname("g-drei"), None);
        // the origin stays the key of the group, the name changes nothing
        assert_eq!(
            wert_von(&s, U, "terminal.schriftgroesse"),
            (json!(15), "gruppe g-eins".to_owned())
        );
        // a viewer in a group sees only its own name; a group without a name has none; an admin sees every name
        assert_eq!(
            s.gruppennamen_fuer(U),
            vec![("g-eins", "Gruppe Entwicklung")]
        );
        assert_eq!(
            s.gruppennamen_fuer("ps_00000000000000000000000000000c02"),
            vec![("g-zwei", "Vertrieb")]
        );
        assert_eq!(
            s.gruppennamen_fuer(A),
            vec![("g-eins", "Gruppe Entwicklung"), ("g-zwei", "Vertrieb")]
        );
        assert_eq!(
            s.gruppennamen_fuer("ps_00000000000000000000000000000c03"),
            Vec::<(&str, &str)>::new()
        );
        // without names the key is missing in the state and the list is empty
        let ohne = stand(json!({}), json!({}), json!({}));
        assert!(ohne.gruppennamen.is_empty() && ohne.gruppennamen_fuer(U).is_empty());
        // `setzen` on the group takes the key, names or not
        let mut t = stand_mit_namen(&namen).expect("state");
        assert!(
            setzen(
                &mut t,
                A,
                &json!("nutzer"),
                &json!("gruppe g-eins"),
                &json!("aussehen.thema"),
                &json!("hell")
            )
            .angenommen()
        );
        // a name without a group, a name twice and a name of another type are no state
        for schlecht in [
            json!({"g-vier": "Es gibt sie nicht"}),
            json!({"g-eins": "Vertrieb", "g-zwei": "Vertrieb"}),
            json!({"g-eins": 5}),
            json!(["g-eins"]),
        ] {
            assert!(stand_mit_namen(&schlecht).is_none(), "{schlecht}");
        }
    }

    fn wert_von(s: &Stand, wer: &str, schluessel: &str) -> (Value, String) {
        let e = sicht(s, wer);
        let x = e.eintrag(schluessel).expect("entry");
        (x.wert.clone(), x.herkunft.clone())
    }

    #[test]
    fn codes_und_gruende() {
        let t: Vec<&str> = Code::ALLE.iter().map(|c| c.text()).collect();
        assert_eq!(
            t,
            [
                "ebenen.einstellung_unbekannt",
                "ebenen.bereich_falsch",
                "ebenen.ebene_unbekannt",
                "ebenen.gruppe_unbekannt",
                "ebenen.ebene_gesperrt",
                "ebenen.nicht_befugt",
                "ebenen.wert_ungueltig",
                "ebenen.lockerung",
                "ebenen.ausser_grenze"
            ]
        );
        let mut s = Code::ALLE.to_vec();
        s.sort();
        assert_eq!(s, Code::ALLE.to_vec(), "order of the stages");
        assert_eq!(
            [Grund::NurStrenger, Grund::NurAdmin, Grund::Fest].map(Grund::text),
            ["nur_strenger", "nur_admin", "fest"]
        );
    }

    #[test]
    fn wahr_wie_die_referenz() {
        for (w, soll) in [
            (json!(null), false),
            (json!(false), false),
            (json!(true), true),
            (json!(0), false),
            (json!(0.0), false),
            (json!(2), true),
            (json!(""), false),
            (json!("a"), true),
            (json!([]), false),
            (json!([0]), true),
            (json!({}), false),
            (json!({"a": 0}), true),
        ] {
            assert_eq!(wahr(&w), soll, "{w}");
        }
    }

    #[test]
    fn stand_aus_wert_verlangt_jede_form() {
        let gut = json!({"klassen": [], "admins": [], "gruppen": {}, "werte": {"firma": {}, "gruppen": {}, "nutzer": {}}});
        assert_eq!(Stand::aus_wert(&gut), Some(Stand::default()));
        for (pfad, w) in [
            ("/klassen", json!([1])),
            ("/admins", json!("a")),
            ("/gruppen", json!({"g": "a"})),
            ("/gruppen", json!([])),
            ("/werte/firma", json!([])),
            ("/werte/gruppen", json!({"g": 1})),
            ("/werte/nutzer", json!([])),
        ] {
            let mut x = gut.clone();
            *x.pointer_mut(pfad).expect("path") = w;
            assert_eq!(Stand::aus_wert(&x), None, "{pfad}");
        }
        let mut ohne = gut.clone();
        ohne.as_object_mut().expect("object").remove("werte");
        assert_eq!(Stand::aus_wert(&ohne), None);
    }

    #[test]
    fn ungueltige_listen_schliessen() {
        let leer = json!({"oeffentlich": [], "intern": []});
        // a value that is no object restricts every class to nothing, on the company level and below
        let s = stand(json!({"harnesses": "pi"}), json!({}), json!({}));
        assert_eq!(wert_von(&s, U, "harnesses").0, leer);
        let s = stand(
            json!({"harnesses": {"intern": ["pi"]}}),
            json!({}),
            json!({"harnesses": 1}),
        );
        assert_eq!(wert_von(&s, U, "harnesses"), (leer, "nutzer".to_owned()));
        // a list that is no list counts as the empty list
        let s = stand(
            json!({"harnesses": {"intern": ["pi"]}}),
            json!({"harnesses": {"intern": "pi"}}),
            json!({}),
        );
        assert_eq!(
            wert_von(&s, U, "harnesses"),
            (json!({"intern": []}), "gruppe g-eins".to_owned())
        );
        let s = stand(
            json!({"harnesses": {"intern": "pi"}}),
            json!({"harnesses": {"intern": ["pi"]}}),
            json!({}),
        );
        assert_eq!(wert_von(&s, U, "harnesses").0, json!({"intern": []}));
        let s = stand(
            json!({}),
            json!({"harnesses": {"intern": "pi", "geheim": ["pi"]}}),
            json!({}),
        );
        assert_eq!(
            wert_von(&s, U, "harnesses"),
            (json!({"intern": []}), "gruppe g-eins".to_owned())
        );
        // equal intersection keeps the origin above
        let s = stand(
            json!({"harnesses": {"intern": ["pi"]}}),
            json!({"harnesses": {"intern": ["pi", "claude"]}}),
            json!({}),
        );
        assert_eq!(
            wert_von(&s, U, "harnesses"),
            (json!({"intern": ["pi"]}), "firma".to_owned())
        );
    }

    #[test]
    fn schalter_und_feste_werte() {
        let s = stand(
            json!({"optionen.herabstufen": true, "optionen.herstellerzugang": true, "optionen.lokale_ca": true}),
            json!({"optionen.herabstufen": false, "optionen.lokale_ca": false}),
            json!({"optionen.herabstufen": true}),
        );
        assert_eq!(
            wert_von(&s, U, "optionen.herabstufen"),
            (json!(false), "gruppe g-eins".to_owned())
        );
        assert_eq!(
            wert_von(&s, U, "optionen.herstellerzugang"),
            (json!(false), "firma".to_owned())
        );
        assert_eq!(
            wert_von(&s, U, "optionen.lokale_ca"),
            (json!(true), "firma".to_owned())
        );
        let s = stand(
            json!({"optionen.herabstufen": true}),
            json!({"optionen.herabstufen": true}),
            json!({}),
        );
        assert_eq!(
            wert_von(&s, U, "optionen.herabstufen"),
            (json!(true), "firma".to_owned())
        );
        let s = stand(
            json!({"aussehen.thema": "hell"}),
            json!({"aussehen.thema": "hell"}),
            json!({}),
        );
        assert_eq!(
            wert_von(&s, U, "aussehen.thema"),
            (json!("hell"), "gruppe g-eins".to_owned())
        );
    }

    #[test]
    fn lockerung_nur_bei_echter_liste() {
        let h = katalog::eintrag("harnesses").expect("key");
        assert!(ist_lockerung(
            h,
            &json!({"intern": ["pi"]}),
            &json!({"intern": ["claude"]})
        ));
        assert!(!ist_lockerung(
            h,
            &json!({"intern": ["pi"]}),
            &json!({"intern": ["pi"]})
        ));
        assert!(!ist_lockerung(
            h,
            &json!({"intern": ["pi"]}),
            &json!({"oeffentlich": ["claude"]})
        ));
        assert!(!ist_lockerung(
            h,
            &json!({"intern": ["pi"]}),
            &json!({"intern": "claude"})
        ));
        assert!(ist_lockerung(
            h,
            &json!({"intern": "pi"}),
            &json!({"intern": ["pi"]})
        ));
        assert!(!ist_lockerung(h, &json!([]), &json!({"intern": ["pi"]})));
        assert!(!ist_lockerung(h, &json!({"intern": []}), &json!([])));
        let o = katalog::eintrag("optionen.herabstufen").expect("key");
        assert!(ist_lockerung(o, &json!(false), &json!(true)));
        assert!(!ist_lockerung(o, &json!(true), &json!(true)));
        assert!(!ist_lockerung(o, &json!(false), &json!(false)));
        let f = katalog::eintrag("aussehen.thema").expect("key");
        assert!(!ist_lockerung(f, &json!("hell"), &json!("dunkel")));
    }

    #[test]
    fn ebene_und_zeile() {
        for (e, soll) in [
            (json!("firma"), Some((Platz::Firma, None))),
            (json!("nutzer"), Some((Platz::Nutzer, Some(U)))),
            (
                json!("gruppe g-eins"),
                Some((Platz::Gruppe, Some("g-eins"))),
            ),
            (json!("gruppe G"), None),
            (json!("gruppe "), None),
            (json!("gruppeg"), None),
            (json!(1), None),
        ] {
            assert_eq!(ziel_von(&e, U).map(|z| (z.art, z.ziel)), soll, "{e}");
        }
        let mut s = stand(json!({}), json!({"aussehen.thema": "hell"}), json!({}));
        let p = setzen(
            &mut s,
            A,
            &json!("nutzer"),
            &json!("gruppe g-eins"),
            &json!("aussehen.thema"),
            &json!("dunkel"),
        );
        assert!(p.angenommen());
        assert_eq!(p.zeile.ebene, Some("gruppe"));
        assert_eq!(p.zeile.ziel.as_deref(), Some("g-eins"));
        assert_eq!(
            p.zeile.alt,
            Some(pruefsumme(&json!("aussehen.thema"), &json!("hell")))
        );
        assert_eq!(s.werte.gruppen["g-eins"]["aussehen.thema"], json!("dunkel"));
        let z = p.zeile.mit_code("firmenvorgabe.veraltet").als_wert();
        assert_eq!(
            (z["code"].clone(), z["aktionsname"].clone()),
            (json!("firmenvorgabe.veraltet"), json!("regel.abgelehnt"))
        );
        let p = setzen(
            &mut s,
            A,
            &json!("nutzer"),
            &json!(1),
            &json!(7),
            &json!(null),
        );
        assert_eq!(
            (p.code, p.zeile.ebene, p.zeile.alt.clone()),
            (Some(Code::EinstellungUnbekannt), None, None)
        );
        // a new place of a person and of a group comes into being on the first value
        let p = setzen(
            &mut s,
            U,
            &json!("nutzer"),
            &json!("nutzer"),
            &json!("aussehen.thema"),
            &json!("hell"),
        );
        assert!(p.angenommen());
        let mut t = Stand {
            klassen: vec!["intern".to_owned()],
            admins: vec![A.to_owned()],
            gruppen: vec![("g".to_owned(), vec![])],
            ..Stand::default()
        };
        assert!(
            setzen(
                &mut t,
                A,
                &json!("nutzer"),
                &json!("gruppe g"),
                &json!("aussehen.thema"),
                &json!("hell")
            )
            .angenommen()
        );
        assert!(
            setzen(
                &mut t,
                U,
                &json!("nutzer"),
                &json!("nutzer"),
                &json!("aussehen.thema"),
                &json!("hell")
            )
            .angenommen()
        );
        assert_eq!(t.werte.gruppen["g"]["aussehen.thema"], json!("hell"));
        assert_eq!(t.werte.nutzer[U]["aussehen.thema"], json!("hell"));
        assert!(t.werte.firma.is_empty());
    }

    /// The person is checked against company and group, never against the own old value (15.4): an own tightening
    /// can be taken back up to the level above.
    #[test]
    fn nutzer_nimmt_eigene_verschaerfung_zurueck() {
        let mut s = stand(
            json!({"harnesses": {"intern": ["pi", "claude"]}, "optionen.herabstufen": true}),
            json!({}),
            json!({"harnesses": {"intern": ["pi"]}, "optionen.herabstufen": false}),
        );
        let n = |s: &mut Stand, k: &str, w: Value| {
            setzen(s, U, &json!("admin"), &json!("nutzer"), &json!(k), &w).code
        };
        assert_eq!(
            n(&mut s, "harnesses", json!({"intern": ["pi", "claude"]})),
            None
        );
        assert_eq!(n(&mut s, "optionen.herabstufen", json!(true)), None);
        assert_eq!(
            n(&mut s, "harnesses", json!({"intern": ["vi"]})),
            Some(Code::WertUngueltig)
        );
        let mut t = stand(
            json!({"harnesses": {"intern": ["pi"]}}),
            json!({}),
            json!({}),
        );
        assert_eq!(
            n(&mut t, "harnesses", json!({"intern": ["claude"]})),
            Some(Code::Lockerung)
        );
    }

    #[test]
    fn grenze_ohne_klassen_und_ohne_liste() {
        let mut s = Stand {
            klassen: vec![],
            ..Stand::default()
        };
        let p = setzen(
            &mut s,
            U,
            &json!("nutzer"),
            &json!("nutzer"),
            &json!("sitzung.harness"),
            &json!("pi"),
        );
        assert_eq!(p.code, Some(Code::AusserGrenze), "no class, no choice");
        let mut s = stand(
            json!({"harnesses": {"intern": "pi"}, "modelle": {"intern": ["m"]}}),
            json!({}),
            json!({}),
        );
        let p = setzen(
            &mut s,
            U,
            &json!("nutzer"),
            &json!("nutzer"),
            &json!("sitzung.harness"),
            &json!("pi"),
        );
        assert_eq!(p.code, None, "the class oeffentlich is open");
        let mut s = stand(
            json!({"harnesses": {"intern": "pi", "oeffentlich": []}}),
            json!({}),
            json!({}),
        );
        let p = setzen(
            &mut s,
            U,
            &json!("nutzer"),
            &json!("nutzer"),
            &json!("sitzung.harness"),
            &json!("pi"),
        );
        assert_eq!(p.code, Some(Code::AusserGrenze));
        let p = setzen(
            &mut s,
            U,
            &json!("nutzer"),
            &json!("nutzer"),
            &json!("sitzung.modell"),
            &json!("m"),
        );
        assert_eq!(p.code, None);
    }

    #[test]
    fn sicht_der_leseklassen() {
        let liste = json!([
            {"ebene": "gruppe", "wer": "g-eins", "klasse": "intern"},
            {"ebene": "nutzer", "wer": U, "klasse": "oeffentlich"},
            {"ebene": "nutzer", "wer": A, "klasse": "intern"},
            {"wer": U},
            {"ebene": "gruppe"},
            {"ebene": "rolle", "wer": U}
        ]);
        let s = stand(json!({"leseklassen": liste}), json!({}), json!({}));
        assert_eq!(
            wert_von(&s, U, "leseklassen").0,
            json!([{"ebene": "gruppe", "wer": "g-eins", "klasse": "intern"}, {"ebene": "nutzer", "wer": U, "klasse": "oeffentlich"}])
        );
        let fremd = "ps_00000000000000000000000000000c09";
        assert_eq!(
            wert_von(&s, fremd, "leseklassen").0,
            json!([]),
            "no group: no entry without wer"
        );
        assert_eq!(
            wert_von(&s, A, "leseklassen").0,
            liste,
            "an admin sees the whole list"
        );
        let e = sicht(&s, U);
        let l = e.eintrag("leseklassen").expect("entry");
        assert_eq!((l.aenderbar, l.grund), (false, Some(Grund::NurAdmin)));
        assert!(e.eintrag("admins").is_none());
        assert_eq!(e.als_wert().as_object().map(Map::len), Some(KATALOG.len()));
    }
}
