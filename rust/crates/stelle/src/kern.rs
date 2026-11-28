//! `entscheide`: one event against the state, in the fixed order of stages (contract 6, section 4).

use formate::kette::{kanonisch, sha256_hex};
use serde_json::{Map, Value, json};

use crate::start::{self, Sicht};
use crate::{
    Anfrage, Anfragezustand, Antwort, Code, Ende, Ereignis, Ergebnis, Offen, Protokollzeile,
    Schreibaufruf, Sitzung, Vorgang, ZEIT_MAX, Zusage, Zustand, form,
};

/// Largest deadline in milliseconds (contract 2: 1 to 86400000).
const FRIST_MAX: i64 = 86_400_000;

/// Largest `argumente` in RFC 8785, in bytes.
const ARGUMENTE_MAX: usize = 1_048_576;
/// Largest preview in bytes (UTF-8).
const VORSCHAU_MAX: usize = 65_536;
/// Longest proof in characters.
const NACHWEIS_MAX: usize = 4096;
/// Largest number `n` of a request identifier `mf-<laden_ms>-<n>`: the form allows at most ten digits.
pub(crate) const ANFRAGE_MAX: u64 = 9_999_999_999;
/// Largest number `n` of a promise identifier `zs-<laden_ms>-<n>` (section 15.3, stage 11).
pub(crate) const ZUSAGE_MAX: u64 = 9_999_999_999;
/// How long a promise holds (section 15.3, stage 12).
pub(crate) const ZUSAGE_FRIST_MS: i64 = 30_000;

type Pruefung = fn(&Value) -> bool;

fn ist_kennung(v: &Value) -> bool {
    v.as_str().is_some_and(form::kennung)
}

fn ist_werkzeug(v: &Value) -> bool {
    v.as_str().is_some_and(form::werkzeug)
}

fn ist_anfrage(v: &Value) -> bool {
    v.as_str().is_some_and(form::anfrage)
}

fn ist_modell(v: &Value) -> bool {
    v.as_str().is_some_and(form::modell)
}

fn ist_kunde(v: &Value) -> bool {
    v.is_null() || ist_kennung(v)
}

fn ist_pseudonym(v: &Value) -> bool {
    v.as_str().is_some_and(form::pseudonym)
}

fn ist_argumente(v: &Value) -> bool {
    v.is_object() && kanonisch(v).is_ok_and(|t| t.len() <= ARGUMENTE_MAX)
}

fn ist_vorschau(v: &Value) -> bool {
    v.is_null() || v.as_str().is_some_and(|t| t.len() <= VORSCHAU_MAX)
}

fn ist_frist(v: &Value) -> bool {
    form::ganzzahl_in(v, 1, FRIST_MAX)
}

fn ist_entscheidung(v: &Value) -> bool {
    matches!(v.as_str(), Some("erteilen" | "ablehnen"))
}

fn ist_nachweis(v: &Value) -> bool {
    v.as_str()
        .is_some_and(|t| (1..=NACHWEIS_MAX).contains(&t.chars().count()))
}

const ANFRAGEN: [(&str, Pruefung); 7] = [
    ("argumente", ist_argumente),
    ("frist_ms", ist_frist),
    ("person", ist_pseudonym),
    ("server", ist_kennung),
    ("sitzung", ist_kennung),
    ("vorschau", ist_vorschau),
    ("werkzeug", ist_werkzeug),
];
const ENTSCHEIDEN: [(&str, Pruefung); 3] = [
    ("anfrage", ist_anfrage),
    ("entscheidung", ist_entscheidung),
    ("nachweis", ist_nachweis),
];
const EINLOESEN: [(&str, Pruefung); 4] = [
    ("anfrage", ist_anfrage),
    ("argumente", ist_argumente),
    ("server", ist_kennung),
    ("werkzeug", ist_werkzeug),
];
const STAND: [(&str, Pruefung); 1] = [("anfrage", ist_anfrage)];
const PSEUDONYM: [(&str, Pruefung); 1] = [("kennung", ist_kennung)];
const WORKER: [(&str, Pruefung); 8] = [
    ("effort", ist_kennung),
    ("harness", ist_kennung),
    ("kunde", ist_kunde),
    ("laufend", ist_laufend),
    ("modell", ist_modell),
    ("rolle", ist_kennung),
    ("route", ist_kennung),
    ("sitzung", ist_kennung),
];

fn ist_laufend(v: &Value) -> bool {
    form::ganzzahl_in(v, 0, crate::worker::LAUFEND_MAX)
}

/// The input is an object with exactly these fields, each with its form.
fn felder(eingabe: &Value, pruefungen: &[(&str, Pruefung)]) -> bool {
    eingabe.as_object().is_some_and(|o| {
        o.len() == pruefungen.len()
            && pruefungen
                .iter()
                .all(|(k, pruefe)| o.get(*k).is_some_and(pruefe))
    })
}

/// The input of `sitzung.starten`: `{"start": {"text": …}}` or `{"start": {"b64": …}}`, with `orchestrator` and `zusage` as
/// optional fields of the operation (section 15.2). `zusage` without `orchestrator` is no valid input.
struct Starteingabe {
    bytes: Vec<u8>,
    orchestrator: Option<String>,
    zusage: Option<String>,
}

fn starteingabe(eingabe: &Value) -> Option<Starteingabe> {
    let o = eingabe.as_object()?;
    if o.keys()
        .any(|k| !matches!(k.as_str(), "start" | "orchestrator" | "zusage"))
    {
        return None;
    }
    let text = |k: &str, gilt: fn(&str) -> bool| match o.get(k) {
        None => Some(None),
        Some(v) => v.as_str().filter(|t| gilt(t)).map(|t| Some(t.to_owned())),
    };
    let orchestrator = text("orchestrator", form::kennung)?;
    let zusage = text("zusage", form::zusage)?;
    if zusage.is_some() && orchestrator.is_none() {
        return None;
    }
    let start = o.get("start")?.as_object().filter(|s| s.len() == 1)?;
    let bytes = match start.iter().next()? {
        (k, Value::String(t)) if k == "text" => Some(t.clone().into_bytes()),
        (k, Value::String(t)) if k == "b64" => form::base64(t),
        _ => None,
    }?;
    Some(Starteingabe {
        bytes,
        orchestrator,
        zusage,
    })
}

/// The text of a field of a checked input.
fn feld<'a>(eingabe: &'a Value, schluessel: &str) -> &'a str {
    eingabe
        .get(schluessel)
        .and_then(Value::as_str)
        .unwrap_or_default()
}

/// sha256 over RFC 8785 of `{"argumente", "server", "werkzeug"}` (section 6).
fn digest(eingabe: &Value) -> String {
    let wert = json!({
        "argumente": eingabe.get("argumente"),
        "server": feld(eingabe, "server"),
        "werkzeug": feld(eingabe, "werkzeug"),
    });
    sha256_hex(kanonisch(&wert).unwrap_or_default().as_bytes())
}

/// The challenge of the human (section 6) as bytes.
fn herausforderung(kennung: &str, a: &Anfrage, entscheidung: &str) -> Vec<u8> {
    crate::pruefer::herausforderung(
        kennung,
        &a.digest,
        entscheidung,
        a.vorschau_sha256.as_deref(),
    )
}

/// The line `mcp.schreibaufruf` for the end of a request.
fn endzeile(kennung: String, a: &Anfrage, ergebnis: Ende) -> Protokollzeile {
    Protokollzeile::Schreibaufruf(Schreibaufruf {
        anfrage: kennung,
        digest: a.digest.clone(),
        entscheidend: a.entscheidend.clone(),
        ergebnis,
        person: a.person.clone(),
        server: a.server.clone(),
        sitzung: a.sitzung.clone(),
        werkzeug: a.werkzeug.clone(),
        zeit_anfrage_ms: a.angefragt_ms,
        zeit_entscheidung_ms: a.entschieden_ms,
    })
}

/// One event in work: the new state, the effective time and the lines so far.
struct Lauf<'a> {
    z: Zustand,
    jetzt: i64,
    zeilen: Vec<Protokollzeile>,
    ereignis: &'a Ereignis,
}

impl Lauf<'_> {
    fn fertig(self, antwort: Antwort) -> Ergebnis {
        Ergebnis {
            zustand: self.z,
            antwort,
            zeilen: self.zeilen,
            zeit_ms: self.jetzt,
        }
    }

    /// A refusal with its line (sections 7.2 and 7.3).
    fn nein(mut self, code: Code, sicht: Sicht) -> Ergebnis {
        let vorgang = &self.ereignis.vorgang;
        let zeile = if let Vorgang::SitzungStarten(_) = vorgang {
            Protokollzeile::SitzungAbgelehnt(code, sicht)
        } else {
            Protokollzeile::VorgangAbgelehnt {
                anfrage: vorgang
                    .eingabe()
                    .get("anfrage")
                    .and_then(Value::as_str)
                    .filter(|t| form::anfrage(t))
                    .map(str::to_owned),
                aufrufer: self.ereignis.aufrufer,
                grund: code,
                vorgang: vorgang.name(),
            }
        };
        self.zeilen.push(zeile);
        self.fertig(Antwort::Nein(code))
    }

    /// Stage 1: every open or released request whose deadline has come runs out, by number.
    fn fristen(&mut self) -> u64 {
        let mut abgelaufen = 0;
        for (n, a) in &mut self.z.anfragen {
            if matches!(a.zustand, Anfragezustand::Offen | Anfragezustand::Erteilt)
                && self.jetzt >= a.bis
            {
                a.zustand = Anfragezustand::Abgelaufen;
                a.vorschau = None;
                let kennung = format!("mf-{}-{n}", self.z.laden_ms);
                self.zeilen.push(endzeile(kennung, a, Ende::Abgelaufen));
                abgelaufen += 1;
            }
        }
        // section 15.3: a promise holds while the time is below its end; an expired one goes without a line
        let jetzt = self.jetzt;
        self.z.zusagen.retain(|_, z| jetzt < z.bis);
        abgelaufen
    }

    /// The number n of a promise identifier of this state that is still held, or `None`.
    fn zusage_nummer(&self, kennung: &str) -> Option<u64> {
        let n: u64 = kennung.rsplit('-').next()?.parse().ok()?;
        (self.z.zusage_kennung(n) == kennung && self.z.zusagen.contains_key(&n)).then_some(n)
    }

    /// The number n of a request identifier of this state, or `None`.
    fn nummer(&self, kennung: &str) -> Option<u64> {
        let n: u64 = kennung.rsplit('-').next()?.parse().ok()?;
        (self.z.kennung(n) == kennung && self.z.anfragen.contains_key(&n)).then_some(n)
    }

    fn starten(mut self, eingabe: &Value) -> Ergebnis {
        let Some(eingabe) = starteingabe(eingabe) else {
            return self.nein(Code::EingabeUngueltig, Sicht::default());
        };
        let start = match start::pruefen(&eingabe.bytes, start::Form::Datei, &self.z.einstellungen)
        {
            Ok(start) => start,
            Err(abgelehnt) => return self.nein(abgelehnt.code, *abgelehnt.sicht),
        };
        let sicht = Sicht {
            harness: Some(start.harness.clone()),
            kunde: start.kunde.clone(),
            route: Some(start.route.clone()),
            sitzung: start.sitzung.clone(),
        };
        let sitzung = start.sitzung.clone().unwrap_or_default();
        if self.z.sitzungen.contains_key(&sitzung) {
            return self.nein(Code::SitzungVergeben, sicht);
        }
        // section 15.2, stage 4: a worker starts only with a held promise of its orchestrator whose values the start matches
        // exactly; the start uses it up, a refusal does not
        if let Some(orchestrator) = &eingabe.orchestrator {
            let n = eingabe
                .zusage
                .as_deref()
                .and_then(|k| self.zusage_nummer(k))
                .filter(|n| {
                    self.z
                        .zusagen
                        .get(n)
                        .is_some_and(|z| z.bindet(orchestrator, &start))
                });
            let Some(n) = n else {
                return self.nein(Code::ZusageUngueltig, sicht);
            };
            self.z.zusagen.remove(&n);
        }
        self.z.sitzungen.insert(
            sitzung.clone(),
            Sitzung {
                route: start.route.clone(),
                kunde: start.kunde,
                harness: start.harness,
                orchestrator: eingabe.orchestrator,
            },
        );
        self.zeilen.push(Protokollzeile::SitzungGestartet(sicht));
        self.fertig(Antwort::Gestartet {
            sitzung,
            route: start.route,
            route_art: start.route_art,
        })
    }

    fn anfragen(mut self, eingabe: &Value) -> Ergebnis {
        let Some(server) = self
            .z
            .einstellungen
            .mcp
            .get(feld(eingabe, "server"))
            .cloned()
        else {
            return self.nein(Code::ServerUnbekannt, Sicht::default());
        };
        if !self.z.sitzungen.contains_key(feld(eingabe, "sitzung")) {
            return self.nein(Code::SitzungUnbekannt, Sicht::default());
        }
        let werkzeug = feld(eingabe, "werkzeug");
        if server.lesen.contains(werkzeug) {
            return self.fertig(Antwort::Lesend);
        }
        // Every way below takes the next identifier; past ANFRAGE_MAX it would leave its own form (contract 6, section 4).
        if self.z.zaehler >= ANFRAGE_MAX {
            return self.nein(Code::AnfragenErschoepft, Sicht::default());
        }
        let vorschau = eingabe
            .get("vorschau")
            .and_then(Value::as_str)
            .map(str::to_owned);
        let frist = eingabe
            .get("frist_ms")
            .and_then(Value::as_i64)
            .unwrap_or_default();
        let anfrage = Anfrage {
            server: feld(eingabe, "server").to_owned(),
            werkzeug: werkzeug.to_owned(),
            sitzung: feld(eingabe, "sitzung").to_owned(),
            person: feld(eingabe, "person").to_owned(),
            digest: digest(eingabe),
            vorschau_sha256: vorschau.as_deref().map(|t| sha256_hex(t.as_bytes())),
            vorschau,
            angefragt_ms: self.jetzt,
            entscheidend: None,
            entschieden_ms: None,
            zustand: Anfragezustand::Offen,
            bis: self.jetzt + frist,
            einloesefrist_ms: server.einloesefrist_ms,
        };
        if !server.freigabe {
            self.z.zaehler += 1;
            let kennung = self.z.kennung(self.z.zaehler);
            self.zeilen
                .push(endzeile(kennung.clone(), &anfrage, Ende::OhneFreigabe));
            return self.fertig(Antwort::OhneFreigabe {
                anfrage: kennung,
                digest: anfrage.digest,
            });
        }
        if frist > server.frist_ms {
            return self.nein(Code::FristUngueltig, Sicht::default());
        }
        self.z.zaehler += 1;
        let n = self.z.zaehler;
        let antwort = Antwort::Offen {
            anfrage: self.z.kennung(n),
            digest: anfrage.digest.clone(),
            frist_bis_ms: anfrage.bis,
            vorschau_sha256: anfrage.vorschau_sha256.clone(),
        };
        self.z.anfragen.insert(n, anfrage);
        self.fertig(antwort)
    }

    fn entscheiden(mut self, eingabe: &Value, n: u64) -> Ergebnis {
        let kennung = self.z.kennung(n);
        let pruefer = self.z.pruefer.clone();
        let Some(a) = self.z.anfragen.get_mut(&n) else {
            return self.nein(Code::AnfrageUnbekannt, Sicht::default());
        };
        match a.zustand {
            Anfragezustand::Offen => {}
            Anfragezustand::Abgelaufen => {
                return self.nein(Code::AnfrageAbgelaufen, Sicht::default());
            }
            _ => return self.nein(Code::BereitsEntschieden, Sicht::default()),
        }
        let entscheidung = feld(eingabe, "entscheidung");
        let bytes = herausforderung(&kennung, a, entscheidung);
        let Some(person) = pruefer
            .pruefe(&bytes, feld(eingabe, "nachweis"))
            .ok()
            .filter(|p| form::pseudonym(p))
        else {
            return self.nein(Code::NachweisUngueltig, Sicht::default());
        };
        a.entscheidend = Some(person);
        a.entschieden_ms = Some(self.jetzt);
        a.vorschau = None;
        if entscheidung == "ablehnen" {
            a.zustand = Anfragezustand::Abgelehnt;
            let zeile = endzeile(kennung.clone(), a, Ende::Abgelehnt);
            self.zeilen.push(zeile);
            return self.fertig(Antwort::Abgelehnt { anfrage: kennung });
        }
        a.zustand = Anfragezustand::Erteilt;
        a.bis = self.jetzt + a.einloesefrist_ms;
        let einloesen_bis_ms = a.bis;
        self.fertig(Antwort::Erteilt {
            anfrage: kennung,
            einloesen_bis_ms,
        })
    }

    fn einloesen(mut self, eingabe: &Value, n: u64) -> Ergebnis {
        let kennung = self.z.kennung(n);
        let neu = digest(eingabe);
        let Some(a) = self.z.anfragen.get_mut(&n) else {
            return self.nein(Code::AnfrageUnbekannt, Sicht::default());
        };
        let stopp = match a.zustand {
            Anfragezustand::Erteilt => None,
            Anfragezustand::Offen => Some(Code::NichtFreigegeben),
            Anfragezustand::Abgelehnt => Some(Code::AnfrageAbgelehnt),
            Anfragezustand::Abgelaufen => Some(Code::AnfrageAbgelaufen),
            Anfragezustand::Eingeloest => Some(Code::BereitsEingeloest),
        };
        if let Some(code) = stopp {
            return self.nein(code, Sicht::default());
        }
        if neu != a.digest {
            return self.nein(Code::DigestAbweichend, Sicht::default());
        }
        a.zustand = Anfragezustand::Eingeloest;
        let zeile = endzeile(kennung.clone(), a, Ende::Eingeloest);
        let digest = a.digest.clone();
        self.zeilen.push(zeile);
        self.fertig(Antwort::Eingeloest {
            anfrage: kennung,
            digest,
        })
    }

    fn stand(self, n: u64) -> Ergebnis {
        let Some(a) = self.z.anfragen.get(&n) else {
            return self.nein(Code::AnfrageUnbekannt, Sicht::default());
        };
        let laufend = matches!(a.zustand, Anfragezustand::Offen | Anfragezustand::Erteilt);
        let antwort = Antwort::Stand {
            anfrage: self.z.kennung(n),
            zustand: a.zustand,
            frist_bis_ms: laufend.then_some(a.bis),
        };
        self.fertig(antwort)
    }

    fn offene(self) -> Ergebnis {
        let offene = self
            .z
            .anfragen
            .iter()
            .filter(|(_, a)| a.zustand == Anfragezustand::Offen)
            .map(|(n, a)| Offen {
                anfrage: self.z.kennung(*n),
                server: a.server.clone(),
                sitzung: a.sitzung.clone(),
                werkzeug: a.werkzeug.clone(),
                vorschau: a.vorschau.clone(),
                vorschau_sha256: a.vorschau_sha256.clone(),
                digest: a.digest.clone(),
                frist_bis_ms: a.bis,
            })
            .collect();
        self.fertig(Antwort::Offene(offene))
    }

    /// `person.pseudonym`, stage 4 (section 14.4): the store of the shell, or `stelle.person_fehler`. A value of the
    /// store that is no pseudonym counts as a failure.
    fn pseudonym(self, eingabe: &Value) -> Ergebnis {
        let ergebnis = self
            .z
            .personen
            .as_ref()
            .and_then(|p| p.pseudonym(feld(eingabe, "kennung"), self.jetzt))
            .filter(|ps| form::pseudonym(ps));
        match ergebnis {
            Some(pseudonym) => self.fertig(Antwort::Pseudonym { pseudonym }),
            None => self.nein(Code::PersonFehler, Sicht::default()),
        }
    }

    /// The position of the class of a route in `klassen` (contract 2, the lowest first), or `None` for an unknown route.
    fn klasse(&self, route: &str) -> Option<usize> {
        let datei = &self.z.einstellungen.datei;
        let klasse = datei.route(route)?.klasse();
        datei.klassen().iter().position(|k| k == klasse)
    }

    /// The first stage of section 15.3 a request violates, after the rules (stage 1), in this order: session, worker,
    /// route, class, customer, harness, model, effort, count, promises. `None`: the worker is free.
    fn worker_grund(&self, eingabe: &Value) -> Option<Code> {
        let regeln = self.z.worker.as_ref()?;
        let Some(orchestrator) = self.z.sitzungen.get(feld(eingabe, "sitzung")) else {
            return Some(Code::SitzungUnbekannt);
        };
        if orchestrator.orchestrator.is_some() {
            return Some(Code::OrchestratorIstWorker);
        }
        let route = feld(eingabe, "route");
        let Some(klasse) = self.klasse(route) else {
            return Some(Code::RouteUnbekannt);
        };
        // only positions in `klassen` are compared; an orchestrator without a class position allows no worker (fail closed)
        if self
            .klasse(&orchestrator.route)
            .is_none_or(|eigene| klasse > eigene)
        {
            return Some(Code::WorkerKlasseHoeher);
        }
        let kunde = eingabe.get("kunde").and_then(Value::as_str);
        let laufend = eingabe
            .get("laufend")
            .and_then(Value::as_i64)
            .unwrap_or(i64::MAX);
        if kunde != orchestrator.kunde.as_deref() {
            Some(Code::WorkerFremderKunde)
        } else if !regeln.harness.contains(feld(eingabe, "harness")) {
            Some(Code::HarnessNichtErlaubt)
        } else if !regeln.modelle.contains(feld(eingabe, "modell")) {
            Some(Code::ModellNichtErlaubt)
        } else if !regeln.effort.contains(feld(eingabe, "effort")) {
            Some(Code::EffortNichtErlaubt)
        } else if laufend >= regeln.hoechstzahl {
            Some(Code::WorkerHoechstzahl)
        } else if self.z.zusage_zaehler >= ZUSAGE_MAX {
            Some(Code::ZusagenErschoepft)
        } else {
            None
        }
    }

    /// `worker.anfordern`, stage 4 (section 15.3): the checks, then a promise `zs-<laden_ms>-<n>` that holds 30 000 ms and
    /// keeps the orchestrator and the values the worker must start with.
    fn worker(mut self, eingabe: &Value) -> Ergebnis {
        if self.z.worker.is_none() {
            return self.nein(Code::VorgabeFehlt, Sicht::default());
        }
        if let Some(code) = self.worker_grund(eingabe) {
            return self.nein(code, Sicht::default());
        }
        self.z.zusage_zaehler += 1;
        let n = self.z.zusage_zaehler;
        let bis = self.jetzt + ZUSAGE_FRIST_MS;
        let sitzung = feld(eingabe, "sitzung").to_owned();
        self.z.zusagen.insert(
            n,
            Zusage {
                orchestrator: sitzung.clone(),
                harness: feld(eingabe, "harness").to_owned(),
                modell: feld(eingabe, "modell").to_owned(),
                effort: feld(eingabe, "effort").to_owned(),
                route: feld(eingabe, "route").to_owned(),
                kunde: eingabe
                    .get("kunde")
                    .and_then(Value::as_str)
                    .map(str::to_owned),
                bis,
            },
        );
        let zusage = self.z.zusage_kennung(n);
        self.fertig(Antwort::Frei {
            sitzung,
            zusage,
            zusage_bis_ms: bis,
        })
    }
}

/// Decides one event: stage 0 (time), stage 1 (deadlines), stage 2 (caller), stage 3 (input), stage 4 (the
/// operation). The given state stays as it is; the result holds the new one. No input makes it panic.
#[must_use]
pub fn entscheide(zustand: &Zustand, ereignis: &Ereignis, jetzt_ms: i64) -> Ergebnis {
    if !(0..=ZEIT_MAX).contains(&jetzt_ms) {
        return Ergebnis {
            zustand: zustand.clone(),
            antwort: Antwort::Nein(Code::ZeitUngueltig),
            zeilen: Vec::new(),
            zeit_ms: zustand.zeit_max,
        };
    }
    let mut z = zustand.clone();
    z.zeit_max = z.zeit_max.max(jetzt_ms);
    let mut lauf = Lauf {
        jetzt: z.zeit_max,
        z,
        zeilen: Vec::new(),
        ereignis,
    };
    let abgelaufen = lauf.fristen();
    let vorgang = &ereignis.vorgang;
    if ereignis.aufrufer != vorgang.erlaubt() {
        return lauf.nein(Code::AufruferNichtErlaubt, Sicht::default());
    }
    let eingabe = vorgang.eingabe();
    let leer = |e: &Value| e.as_object().is_some_and(Map::is_empty);
    let form = match vorgang {
        Vorgang::SitzungStarten(_) => starteingabe(eingabe).is_some(),
        Vorgang::McpAnfragen(_) => felder(eingabe, &ANFRAGEN),
        Vorgang::McpEntscheiden(_) => felder(eingabe, &ENTSCHEIDEN),
        Vorgang::McpEinloesen(_) => felder(eingabe, &EINLOESEN),
        Vorgang::McpStand(_) => felder(eingabe, &STAND),
        Vorgang::PersonPseudonym(_) => felder(eingabe, &PSEUDONYM),
        Vorgang::WorkerAnfordern(_) => felder(eingabe, &WORKER),
        Vorgang::McpOffene(_) | Vorgang::AblaufPruefen(_) => leer(eingabe),
    };
    if !form {
        return lauf.nein(Code::EingabeUngueltig, Sicht::default());
    }
    let nummer = lauf.nummer(feld(eingabe, "anfrage"));
    match (vorgang, nummer) {
        (Vorgang::SitzungStarten(_), _) => lauf.starten(eingabe),
        (Vorgang::McpAnfragen(_), _) => lauf.anfragen(eingabe),
        (Vorgang::McpOffene(_), _) => lauf.offene(),
        (Vorgang::AblaufPruefen(_), _) => lauf.fertig(Antwort::Geprueft { abgelaufen }),
        (Vorgang::PersonPseudonym(_), _) => lauf.pseudonym(eingabe),
        (Vorgang::WorkerAnfordern(_), _) => lauf.worker(eingabe),
        (_, None) => lauf.nein(Code::AnfrageUnbekannt, Sicht::default()),
        (Vorgang::McpEntscheiden(_), Some(n)) => lauf.entscheiden(eingabe, n),
        (Vorgang::McpEinloesen(_), Some(n)) => lauf.einloesen(eingabe, n),
        (Vorgang::McpStand(_), Some(n)) => lauf.stand(n),
    }
}

#[cfg(test)]
mod tests;
