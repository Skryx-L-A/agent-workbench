//! The server under Linux: one thread per connection, one request after the other on a connection (section 3), the
//! peer check before the first line (section 2.3), the order of checks of [`crate::pruefung`] and the execution of
//! stages 28 to 31 with the leak lock (section 8). The tickets redeemed are held for all connections under one lock.

use std::collections::{BTreeMap, BTreeSet};
use std::io::{self, Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use rustix::event::{PollFd, PollFlags, poll};
use serde_json::{Map, Value, json};

use crate::ablage::Ablage;
use crate::antwort;
use crate::antwortform::Sammler;
use crate::b64;
use crate::code::Code;
use crate::form::Op;
use crate::geheim::Geheim;
use crate::kette;
use crate::konfig::{Konfig, Ziel};
use crate::leck::{self, Sperre};
use crate::mcp;
use crate::peer;
use crate::pruefung::{self, Aufrufer, Eintrag, Geprueft, Rolle, Sicht, ZEILE_MAX};
use crate::rahmen::Rahmenleser;
use crate::suche;
use crate::ziel::{self, FRIST_KOPF, Koerper, Leitung};

/// Deadline for the first line after the connection.
pub const FRIST_ERSTE: Duration = Duration::from_secs(10);
/// Deadline for every further line after the last answer.
pub const FRIST_WEITERE: Duration = Duration::from_secs(300);
/// Most payload bytes of one line `teil`.
pub const TEIL_MAX: usize = 65_536;

/// The clock in ms since 1970.
#[must_use]
pub fn jetzt_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX))
}

/// The state of the holder shared by all connections.
pub struct Dienst {
    /// `halter.json`.
    pub konfig: Konfig,
    /// The store.
    pub ablage: Ablage,
    /// The own uid.
    pub konto: u32,
    /// End of step 4.
    pub start_ms: i64,
    /// The role groups: effective gid of a peer to its role (section 2.3a).
    pub rollen: BTreeMap<u32, Rolle>,
    /// Folder of the chain, `None` if it could not be made (then no line is written).
    pub kette: Option<PathBuf>,
    /// The clock (ms since 1970); [`jetzt_ms`] in the delivered build.
    pub uhr: fn() -> i64,
    benutzt: Mutex<BTreeMap<String, i64>>,
    leck_treffer: AtomicU64,
    /// Set after the first fixed line on a chain that cannot be continued (one line per run, F16).
    kette_gemeldet: AtomicBool,
    /// How many journal lines of the answer form of the first start were written in this run.
    antwortform_zahl: AtomicU64,
    /// The last of them.
    antwortform_letzte: Mutex<Option<String>>,
}

enum Zeile {
    Da(Vec<u8>),
    ZuGross,
    Ende,
}

/// The reader of lines of one connection.
struct Leser {
    vorrat: Vec<u8>,
    /// So many bytes of `vorrat` are known to hold no LF (the search never starts at the front again).
    ohne_lf: usize,
}

impl Leser {
    /// The next complete line in `vorrat` without its LF, if one is there.
    fn aus_vorrat(&mut self) -> Option<Vec<u8>> {
        let start = self.ohne_lf.min(self.vorrat.len());
        let Some(p) = self.vorrat[start..].iter().position(|b| *b == b'\n') else {
            self.ohne_lf = self.vorrat.len();
            return None;
        };
        let rest = self.vorrat.split_off(start + p + 1);
        let mut z = std::mem::replace(&mut self.vorrat, rest);
        z.pop();
        self.ohne_lf = 0;
        Some(z)
    }
}

impl Leser {
    /// The next complete line of `vorrat` (or `ZuGross` if it or the bytes without LF are over [`ZEILE_MAX`]), `None`
    /// if more bytes are needed. The reader of requests and the open stream share it, so one test holds both borders.
    fn fertige(&mut self) -> Option<Zeile> {
        if let Some(z) = self.aus_vorrat() {
            if z.len() > ZEILE_MAX {
                return Some(Zeile::ZuGross);
            }
            return Some(Zeile::Da(z));
        }
        (self.vorrat.len() > ZEILE_MAX).then_some(Zeile::ZuGross)
    }

    /// The next line from `strom` within `frist`; `setzen` sets the deadline of one read on `strom` (for a socket
    /// `UnixStream::set_read_timeout`, which refuses a zero deadline).
    fn naechste<Q: Read>(
        &mut self,
        strom: &mut Q,
        frist: Duration,
        setzen: impl Fn(&Q, Option<Duration>) -> io::Result<()>,
    ) -> Zeile {
        let ende = Instant::now() + frist;
        let mut buf = vec![0u8; 65_536];
        loop {
            if let Some(z) = self.fertige() {
                return z;
            }
            let rest = ende.saturating_duration_since(Instant::now());
            if rest.is_zero() || setzen(strom, Some(rest)).is_err() {
                return Zeile::Ende;
            }
            match strom.read(&mut buf) {
                Ok(0) => return Zeile::Ende,
                Ok(n) => self.vorrat.extend_from_slice(&buf[..n]),
                Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                Err(_) => return Zeile::Ende,
            }
        }
    }
}

fn senden(strom: &mut UnixStream, wert: &Value) -> io::Result<()> {
    strom.write_all(&antwort::zeile(wert))
}

fn teile_senden(strom: &mut UnixStream, id: &Value, daten: &[u8]) -> io::Result<()> {
    for t in daten.chunks(TEIL_MAX) {
        senden(strom, &antwort::teil(id, t))?;
    }
    Ok(())
}

/// A head of `beginn` holds a secret (name or value).
fn kopf_leck(werte: &[Geheim], kopf: &Map<String, Value>) -> bool {
    kopf.iter().any(|(n, w)| {
        leck::trifft(werte, n.as_bytes())
            || w.as_str()
                .is_some_and(|w| leck::trifft(werte, w.as_bytes()))
    })
}

fn s<'a>(o: &'a Map<String, Value>, k: &str) -> &'a str {
    o.get(k).and_then(Value::as_str).unwrap_or("")
}

impl Dienst {
    /// The state after step 4.
    #[must_use]
    pub fn neu(
        konfig: Konfig,
        ablage: Ablage,
        konto: u32,
        start_ms: i64,
        rollen: BTreeMap<u32, Rolle>,
        kette: Option<PathBuf>,
        uhr: fn() -> i64,
    ) -> Self {
        Dienst {
            konfig,
            ablage,
            konto,
            start_ms,
            rollen,
            kette,
            uhr,
            benutzt: Mutex::new(BTreeMap::new()),
            leck_treffer: AtomicU64::new(0),
            kette_gemeldet: AtomicBool::new(false),
            antwortform_zahl: AtomicU64::new(0),
            antwortform_letzte: Mutex::new(None),
        }
    }

    /// A hit of the leak lock for the request `g`: the fixed journal line on stderr and the line `halter.leck_gesperrt`
    /// in the chain (section 9), counted. The chain line names only the op and the target (`ziel` or `server` from
    /// `halter.json`), never a value or a part of the hit.
    fn leck(&self, g: &Geprueft) {
        self.leck_treffer.fetch_add(1, Ordering::Relaxed);
        kette::leck_gesperrt(&mut io::stderr());
        let Some(k) = &self.kette else {
            return;
        };
        let o = &g.anfrage;
        let ziel = o
            .get("ziel")
            .or_else(|| o.get("server"))
            .and_then(Value::as_str)
            .unwrap_or("");
        if let Err(z) = kette::leck_zeile(k, (self.uhr)(), s(o, "op"), ziel)
            && self.erste_meldung()
        {
            let mut e = io::stderr();
            let _ = writeln!(e, "{z}");
        }
    }

    /// How many hits of the leak lock wrote their journal line since the start.
    #[must_use]
    pub fn leck_treffer(&self) -> u64 {
        self.leck_treffer.load(Ordering::Relaxed)
    }

    fn eigenes_konto(&self) -> String {
        self.konto.to_string()
    }

    /// The secrets with their values, read anew, and the entries of the view.
    fn geheimnisse(&self) -> (Vec<Eintrag>, Vec<(String, Geheim)>) {
        let alle = self.ablage.alle();
        let konto = self.eigenes_konto();
        let person = self.konfig.konten.get(&konto).cloned().flatten();
        let eintraege = alle
            .iter()
            .map(|(n, _)| {
                let maschine = n.starts_with("modell.");
                Eintrag {
                    name: n.clone(),
                    konto: (!maschine).then(|| konto.clone()),
                    person: if n.starts_with("mcp.") || n.starts_with("mail.") {
                        person.clone()
                    } else {
                        None
                    },
                }
            })
            .collect();
        (eintraege, alle)
    }

    fn sicht(&self, aufrufer: &Aufrufer, eintraege: Vec<Eintrag>, strom: Option<&Value>) -> Sicht {
        let benutzt: BTreeSet<String> = self
            .benutzt
            .lock()
            .map(|b| b.keys().cloned().collect())
            .unwrap_or_default();
        Sicht {
            aufrufer: aufrufer.clone(),
            jetzt_ms: (self.uhr)(),
            start_ms: self.start_ms,
            kids: self.konfig.kids.clone(),
            benutzt,
            // the form of the human proof is open (OPEN 1): no proof is valid, so setzen and loeschen never pass
            nachweis_gueltig: false,
            ziele: self
                .konfig
                .ziele
                .iter()
                .map(|(n, z)| (n.clone(), z.regel.clone()))
                .collect(),
            server: self
                .konfig
                .server
                .iter()
                .map(|(n, z)| (n.clone(), z.regel.clone()))
                .collect(),
            geheimnisse: eintraege,
            strom: strom.cloned(),
        }
    }

    /// Redeems `einmal` under the one lock; `false` if it was redeemed already.
    fn einloesen(&self, einmal: &str, ablauf: i64) -> bool {
        let Ok(mut b) = self.benutzt.lock() else {
            return false;
        };
        let jetzt = (self.uhr)();
        b.retain(|_, a| *a > jetzt);
        if b.contains_key(einmal) {
            return false;
        }
        b.insert(einmal.to_owned(), ablauf);
        true
    }

    /// How many journal lines of the answer form (section 4, "Antwortform im Erststart") were written in this run;
    /// 0 without the switch `erststart_antwortform`.
    #[must_use]
    pub fn antwortform_zahl(&self) -> u64 {
        self.antwortform_zahl.load(Ordering::Relaxed)
    }

    /// The last journal line of the answer form of this run, `None` before the first.
    #[must_use]
    pub fn antwortform_letzte(&self) -> Option<String> {
        self.antwortform_letzte.lock().ok().and_then(|z| z.clone())
    }

    /// Writes the journal line of the answer form to stderr, counts it and keeps it as the last one.
    fn antwortform_schreiben(&self, zeile: String) {
        let mut e = io::stderr();
        let _ = e.write_all(zeile.as_bytes());
        self.antwortform_zahl.fetch_add(1, Ordering::Relaxed);
        if let Ok(mut z) = self.antwortform_letzte.lock() {
            *z = Some(zeile);
        }
    }

    /// Whether the fixed journal line of a chain that cannot be continued was written in this run (F16).
    #[must_use]
    pub fn kette_gemeldet(&self) -> bool {
        self.kette_gemeldet.load(Ordering::Relaxed)
    }

    /// `true` only the first time in a run: the fixed line of a chain that cannot be continued is written once (F16).
    fn erste_meldung(&self) -> bool {
        !self.kette_gemeldet.swap(true, Ordering::Relaxed)
    }

    /// Writes the line of a refused ticket to the chain. When the chain cannot be continued (a broken anchor or any
    /// other failure), the first such failure of the run writes one fixed line to the journal (F16), no path, no content.
    fn kette_schreiben(&self, code: Code, sch: &pruefung::ScheinAbgelehnt) {
        let Some(k) = &self.kette else {
            return;
        };
        if let Err(z) = kette::schein_abgelehnt(k, (self.uhr)(), code, sch)
            && self.erste_meldung()
        {
            let mut e = io::stderr();
            let _ = writeln!(e, "{z}");
        }
    }

    /// Writes the line `halter.pfad_verboten` of a refused path to the chain (section 9); a chain that cannot be
    /// continued gives the fixed journal line once per run, as for a refused ticket.
    fn pfad_schreiben(&self, p: &pruefung::PfadAbgelehnt) {
        let Some(k) = &self.kette else {
            return;
        };
        if let Err(z) = kette::pfad_zeile(k, (self.uhr)(), p)
            && self.erste_meldung()
        {
            let mut e = io::stderr();
            let _ = writeln!(e, "{z}");
        }
    }

    /// The role of the peer of `strom` (stage 2).
    #[must_use]
    pub fn aufrufer(&self, strom: &UnixStream) -> Aufrufer {
        let konto = self.eigenes_konto();
        let rolle = peer::messen(strom).ok().and_then(|p| {
            if !self.konfig.konten.contains_key(&p.uid.to_string()) {
                return None;
            }
            peer::rolle(&p, self.konto, &self.rollen)
        });
        Aufrufer {
            rolle,
            konto,
            vertreter: false,
        }
    }

    /// Serves one connection until it ends.
    pub fn verbindung(&self, mut strom: UnixStream) {
        let aufrufer = self.aufrufer(&strom);
        let mut leser = Leser {
            vorrat: Vec::new(),
            ohne_lf: 0,
        };
        let mut frist = FRIST_ERSTE;
        loop {
            let zeile = match leser.naechste(&mut strom, frist, UnixStream::set_read_timeout) {
                Zeile::Da(z) => z,
                Zeile::ZuGross => {
                    let _ = senden(&mut strom, &antwort::fehler(None, Code::ZuGross));
                    return;
                }
                Zeile::Ende => return,
            };
            if !self.anfrage(&mut strom, &mut leser, &aufrufer, &zeile) {
                return;
            }
            frist = FRIST_WEITERE;
        }
    }

    /// One request; `false` ends the connection.
    fn anfrage(
        &self,
        strom: &mut UnixStream,
        leser: &mut Leser,
        aufrufer: &Aufrufer,
        zeile: &[u8],
    ) -> bool {
        let (eintraege, alle) = self.geheimnisse();
        let sicht = self.sicht(aufrufer, eintraege, None);
        let g = match pruefung::pruefen(zeile, &sicht) {
            Ok(g) => g,
            Err(a) => {
                if let Some(sch) = &a.schein {
                    self.kette_schreiben(a.code, sch);
                }
                if let Some(p) = &a.pfad {
                    self.pfad_schreiben(p);
                }
                let weiter = a.code != Code::Peer;
                return senden(strom, &antwort::fehler(a.id.as_ref(), a.code)).is_ok() && weiter;
            }
        };
        let werte: Vec<Geheim> = alle
            .iter()
            .map(|(_, w)| Geheim::neu(w.bytes().to_vec()))
            .collect();
        let wert_von = |name: &str| alle.iter().find(|(n, _)| n == name).map(|(_, w)| w);
        let o = &g.anfrage;
        match g.op {
            Op::Vorhanden => {
                // by `ziel` the holder takes the name of the secret from `halter.json` (stage 14 knows the target)
                let name = match o.get("ziel").and_then(Value::as_str) {
                    Some(z) => self
                        .konfig
                        .ziele
                        .get(z)
                        .map_or("", |z| z.regel.schluessel.as_str()),
                    None => s(o, "name"),
                };
                let da = if name.starts_with("modell.") {
                    sicht
                        .geheimnisse
                        .iter()
                        .any(|e| e.name == name && e.konto.is_none())
                } else {
                    !aufrufer.vertreter
                        && sicht.geheimnisse.iter().any(|e| {
                            e.name == name && e.konto.as_deref() == Some(aufrufer.konto.as_str())
                        })
                };
                senden(strom, &antwort::ok(&g.id, Some(("vorhanden", json!(da))))).is_ok()
            }
            Op::ModellSuche => {
                let Some(z) = self.konfig.ziele.get(s(o, "ziel")) else {
                    return senden(strom, &antwort::fehler(Some(&g.id), Code::Intern)).is_ok();
                };
                let Some(wert) = wert_von(&z.regel.schluessel) else {
                    return senden(strom, &antwort::fehler(Some(&g.id), Code::KeinSchluessel))
                        .is_ok();
                };
                let pfade: Vec<PathBuf> = o
                    .get("pfade")
                    .and_then(Value::as_array)
                    .map(|l| {
                        l.iter()
                            .filter_map(Value::as_str)
                            .map(PathBuf::from)
                            .collect()
                    })
                    .unwrap_or_default();
                let wurzeln: Vec<&Path> = pfade.iter().map(PathBuf::as_path).collect();
                let e = suche::durchsuchen(wert.bytes(), &wurzeln, suche::PUFFER);
                let feld = json!({"dateien": e.dateien, "nicht_lesbar": e.nicht_lesbar,
                    "treffer": e.treffer, "uebersprungen": e.uebersprungen});
                senden(strom, &antwort::ok(&g.id, Some(("suche", feld)))).is_ok()
            }
            Op::McpAufruf => self.mcp(strom, &g, &werte, &wert_von),
            Op::ModellAufruf | Op::ModellStrom => {
                let Some(z) = self.konfig.ziele.get(s(o, "ziel")) else {
                    return senden(strom, &antwort::fehler(Some(&g.id), Code::Intern)).is_ok();
                };
                let Some(wert) = wert_von(&z.regel.schluessel) else {
                    return senden(strom, &antwort::fehler(Some(&g.id), Code::KeinSchluessel))
                        .is_ok();
                };
                self.modell(strom, leser, aufrufer, &g, z, wert, &werte)
            }
            // setzen and loeschen never pass stage 27 (no valid proof, OPEN 1); strom.teil and strom.ende never pass
            // stage 7a without an open stream
            Op::Setzen | Op::Loeschen | Op::StromTeil | Op::StromEnde => {
                senden(strom, &antwort::fehler(Some(&g.id), Code::Intern)).is_ok()
            }
        }
    }

    fn mcp<'a>(
        &self,
        strom: &mut UnixStream,
        g: &Geprueft,
        werte: &[Geheim],
        wert_von: &dyn Fn(&str) -> Option<&'a Geheim>,
    ) -> bool {
        let o = &g.anfrage;
        let Some(server) = self.konfig.server.get(s(o, "server")) else {
            return senden(strom, &antwort::fehler(Some(&g.id), Code::Intern)).is_ok();
        };
        let Some(token) = wert_von(&server.regel.schluessel) else {
            return senden(strom, &antwort::fehler(Some(&g.id), Code::KeinSchluessel)).is_ok();
        };
        if let Some((einmal, ablauf)) = &g.einloesen
            && !self.einloesen(einmal, *ablauf)
        {
            if let Some(d) = &g.digest {
                let sch = pruefung::ScheinAbgelehnt {
                    digest: d.clone(),
                    lauf: s(o, "lauf").to_owned(),
                    server: s(o, "server").to_owned(),
                    werkzeug: s(o, "werkzeug").to_owned(),
                };
                self.kette_schreiben(Code::ScheinBenutzt, &sch);
            }
            return senden(strom, &antwort::fehler(Some(&g.id), Code::ScheinBenutzt)).is_ok();
        }
        let argumente = o.get("argumente").cloned().unwrap_or(Value::Null);
        let zeile = match mcp::aufrufen(server, s(o, "werkzeug"), &argumente, token, werte) {
            Ok(mcp::Ausgang::Ergebnis(e)) => {
                antwort::ok(&g.id, Some(("ergebnis", Value::Object(e))))
            }
            Ok(mcp::Ausgang::Fehler(n)) => antwort::mcp_fehler(&g.id, n),
            Err(c) => {
                if c == Code::LeckGesperrt {
                    self.leck(g);
                }
                antwort::fehler(Some(&g.id), c)
            }
        };
        senden(strom, &zeile).is_ok()
    }

    #[allow(
        clippy::too_many_arguments,
        clippy::many_single_char_names,
        clippy::similar_names,
        reason = "one call carries the connection, its reader and the checked request"
    )]
    fn modell(
        &self,
        strom: &mut UnixStream,
        leser: &mut Leser,
        aufrufer: &Aufrufer,
        g: &Geprueft,
        z: &Ziel,
        wert: &Geheim,
        werte: &[Geheim],
    ) -> bool {
        let o = &g.anfrage;
        let ist_strom = g.op == Op::ModellStrom;
        let methode = if ist_strom { "GET" } else { s(o, "methode") };
        let mut ziel_text = format!("{}{}", z.praefix, s(o, "pfad"));
        if let Some(a) = o.get("abfrage").and_then(Value::as_str) {
            ziel_text.push('?');
            ziel_text.push_str(a);
        }
        let koerper = if ist_strom {
            None
        } else {
            match b64::dekodieren(s(o, "koerper_b64")) {
                Some(k) => Some(k),
                None => return senden(strom, &antwort::fehler(Some(&g.id), Code::Intern)).is_ok(),
            }
        };
        let leer = Map::new();
        let kopf = o.get("kopf").and_then(Value::as_object).unwrap_or(&leer);
        let anfrage = Geheim::neu(ziel::anfrage(
            methode,
            &ziel_text,
            &z.adresse,
            kopf,
            wert.bytes(),
            koerper.as_deref(),
            ist_strom,
        ));
        drop(koerper);
        let mut l = match ziel::verbinden(&z.adresse) {
            Ok(l) => l,
            Err(c) => return senden(strom, &antwort::fehler(Some(&g.id), c)).is_ok(),
        };
        // one result for both steps (a failed write would also fail the head that follows, so `||` against `&&` could
        // never be seen)
        let gesendet = l
            .tcp()
            .set_write_timeout(Some(FRIST_KOPF))
            .and_then(|()| l.write_all(anfrage.bytes()));
        if gesendet.is_err() {
            return senden(strom, &antwort::fehler(Some(&g.id), Code::ZielFehler)).is_ok();
        }
        drop(anfrage);
        let (k, vorrat) = match ziel::kopf_lesen(l.as_mut(), FRIST_KOPF) {
            Ok(x) => x,
            Err(c) => return senden(strom, &antwort::fehler(Some(&g.id), c)).is_ok(),
        };
        let geschlossen = |strom: &mut UnixStream, c: Code| {
            senden(strom, &antwort::fehler(Some(&g.id), c)).is_ok()
        };
        if k.komprimiert() {
            return geschlossen(strom, Code::ZielKomprimiert);
        }
        if k.status == 101 && !ist_strom {
            return geschlossen(strom, Code::ZielFehler);
        }
        if k.status == 101 && k.hat("sec-websocket-extensions") {
            return geschlossen(strom, Code::ZielKomprimiert);
        }
        let weiter = match k.weiter() {
            Ok(w) => w,
            Err(c) => return geschlossen(strom, c),
        };
        if kopf_leck(werte, &weiter) {
            self.leck(g);
            return geschlossen(strom, Code::LeckGesperrt);
        }
        if senden(strom, &antwort::beginn(&g.id, k.status, weiter)).is_err() {
            return false;
        }
        if k.status == 101 {
            return self.strom_offen(strom, leser, aufrufer, g, l, &vorrat, werte);
        }
        let mut koerper = match Koerper::neu(l.as_mut(), &k, vorrat, methode) {
            Ok(k) => k,
            Err(c) => return senden(strom, &antwort::ende_fehler(&g.id, c)).is_ok(),
        };
        // the switch of the first start names exactly this rule: a copy of the body for the answer form (section 4);
        // the answer itself goes on through the leak lock
        let regel = format!("{methode} {}", s(o, "pfad"));
        let form = z.antwortform.contains(&regel).then(Sammler::neu);
        self.koerper_senden(strom, g, &mut koerper, werte, form, k.status)
    }

    /// The body of an answer after `beginn`: every piece through the leak lock to the bridge, then `ende`. With `form`
    /// (the switch of the first start) a copy of the body gives the journal line of the answer form after the last piece.
    fn koerper_senden(
        &self,
        strom: &mut UnixStream,
        g: &Geprueft,
        koerper: &mut Koerper<'_>,
        werte: &[Geheim],
        mut form: Option<Sammler>,
        status: u16,
    ) -> bool {
        let o = &g.anfrage;
        let mut sperre = Sperre::neu(werte);
        loop {
            match koerper.naechstes() {
                Ok(Some(b)) => {
                    if let Some(f) = &mut form {
                        f.schieben(&b);
                    }
                    let Ok(frei) = sperre.schieben(&b) else {
                        self.leck(g);
                        return senden(strom, &antwort::ende_fehler(&g.id, Code::LeckGesperrt))
                            .is_ok();
                    };
                    if teile_senden(strom, &g.id, &frei).is_err() {
                        return false;
                    }
                }
                Ok(None) => {
                    if let Some(f) = &form {
                        // only `modell.aufruf` reaches a body (a 101 opens the stream before)
                        self.antwortform_schreiben(f.zeile(
                            werte,
                            s(o, "ziel"),
                            s(o, "methode"),
                            s(o, "pfad"),
                            status,
                        ));
                    }
                    let rest = sperre.ende();
                    return teile_senden(strom, &g.id, &rest).is_ok()
                        && senden(strom, &antwort::ende(&g.id)).is_ok();
                }
                Err(c) => return senden(strom, &antwort::ende_fehler(&g.id, c)).is_ok(),
            }
        }
    }

    /// The open stream after the 101 (section 4, `modell.strom`). `true` if the connection serves requests again.
    #[allow(
        clippy::too_many_arguments,
        reason = "the stream needs the connection, its reader, the caller and the target"
    )]
    fn strom_offen(
        &self,
        strom: &mut UnixStream,
        leser: &mut Leser,
        aufrufer: &Aufrufer,
        g: &Geprueft,
        mut l: Box<dyn Leitung>,
        vorrat: &[u8],
        werte: &[Geheim],
    ) -> bool {
        let id = &g.id;
        let mut rahmen = Rahmenleser::neu(werte);
        let abriss = |strom: &mut UnixStream, c: Code| {
            if c == Code::LeckGesperrt {
                self.leck(g);
            }
            senden(strom, &antwort::ende_fehler(id, c)).is_ok()
        };
        match rahmen.schieben(vorrat) {
            Ok(frei) => {
                if teile_senden(strom, id, &frei).is_err() {
                    return false;
                }
            }
            Err(a) => return abriss(strom, a.code()),
        }
        if l.tcp()
            .set_nonblocking(true)
            .and_then(|()| strom.set_nonblocking(true))
            .is_err()
        {
            return abriss(strom, Code::Intern);
        }
        let ergebnis = self.strom_schleife(strom, leser, aufrufer, id, l.as_mut(), &mut rahmen);
        let _ = strom.set_nonblocking(false);
        let _ = l.tcp().shutdown(std::net::Shutdown::Both);
        match ergebnis {
            Schluss::Ende => match rahmen.ende() {
                Ok(frei) => {
                    teile_senden(strom, id, &frei).is_ok()
                        && senden(strom, &antwort::ende(id)).is_ok()
                }
                Err(a) => abriss(strom, a.code()),
            },
            Schluss::Abriss(c) => abriss(strom, c),
            Schluss::Weg => false,
        }
    }

    fn strom_schleife(
        &self,
        strom: &mut UnixStream,
        leser: &mut Leser,
        aufrufer: &Aufrufer,
        id: &Value,
        l: &mut dyn Leitung,
        rahmen: &mut Rahmenleser<'_>,
    ) -> Schluss {
        let mut buf = vec![0u8; 65_536];
        loop {
            // the target first: TLS may hold bytes the socket no longer shows
            loop {
                match l.read(&mut buf) {
                    Ok(0) => return Schluss::Ende,
                    Ok(n) => match rahmen.schieben(&buf[..n]) {
                        Ok(frei) => {
                            if blockierend(strom, |s| teile_senden(s, id, &frei)).is_err() {
                                return Schluss::Weg;
                            }
                        }
                        Err(a) => return Schluss::Abriss(a.code()),
                    },
                    Err(e) => {
                        if nur_warten(e.kind()) {
                            break;
                        }
                        return Schluss::Abriss(Code::ZielFehler);
                    }
                }
            }
            // then the lines of the bridge
            loop {
                match leser.fertige() {
                    Some(Zeile::Da(z)) => match self.strom_zeile(strom, aufrufer, id, l, &z) {
                        Some(s) => return s,
                        None => continue,
                    },
                    Some(Zeile::ZuGross | Zeile::Ende) => {
                        let _ = blockierend(strom, |s| {
                            senden(s, &antwort::fehler(None, Code::ZuGross))
                        });
                        return Schluss::Weg;
                    }
                    None => {}
                }
                match strom.read(&mut buf) {
                    Ok(0) => return Schluss::Weg,
                    Ok(n) => leser.vorrat.extend_from_slice(&buf[..n]),
                    Err(e) => {
                        if nur_warten(e.kind()) {
                            break;
                        }
                        return Schluss::Weg;
                    }
                }
            }
            let mut fds = [
                PollFd::new(l.tcp(), PollFlags::IN),
                PollFd::new(&*strom, PollFlags::IN),
            ];
            let warte = rustix::time::Timespec {
                tv_sec: 1,
                tv_nsec: 0,
            };
            let _ = poll(&mut fds, Some(&warte));
        }
    }

    /// One line of the bridge while the stream is open: `strom.teil`, `strom.ende` or a refused request.
    fn strom_zeile(
        &self,
        strom: &mut UnixStream,
        aufrufer: &Aufrufer,
        id: &Value,
        l: &mut dyn Leitung,
        z: &[u8],
    ) -> Option<Schluss> {
        let sicht = self.sicht(aufrufer, Vec::new(), Some(id));
        match pruefung::pruefen(z, &sicht) {
            Ok(g) if g.op == Op::StromTeil => {
                let daten = b64::dekodieren(s(&g.anfrage, "daten_b64"))?;
                if l.tcp().set_nonblocking(false).is_err() || l.write_all(&daten).is_err() {
                    return Some(Schluss::Abriss(Code::ZielFehler));
                }
                if l.tcp().set_nonblocking(true).is_err() {
                    return Some(Schluss::Abriss(Code::Intern));
                }
                None
            }
            Ok(_) => Some(Schluss::Ende),
            Err(a) => {
                if blockierend(strom, |s| {
                    senden(s, &antwort::fehler(a.id.as_ref(), a.code))
                })
                .is_err()
                {
                    return Some(Schluss::Weg);
                }
                None
            }
        }
    }
}

enum Schluss {
    /// The target closed or the bridge sent `strom.ende`.
    Ende,
    /// A break of the stream with its code.
    Abriss(Code),
    /// The bridge is gone.
    Weg,
}

/// Most connections at once (at least 32 of the role `bruecke`, section 3, besides hub and surface).
pub const VERBINDUNGEN_MAX: usize = 128;

/// What the accept loop does after an `accept` without a connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pause {
    /// Nothing waits (`WouldBlock`): wait in `poll` until a connection comes, at most 200 ms (then the stop flag is
    /// looked at again).
    Bereitschaft,
    /// Another error (such as too many open files): rest 50 ms, so that the loop does not spin.
    Ruhe,
}

/// The pause after an `accept` that failed with `art`.
#[must_use]
pub fn pause_nach(art: io::ErrorKind) -> Pause {
    match art {
        io::ErrorKind::WouldBlock => Pause::Bereitschaft,
        _ => Pause::Ruhe,
    }
}

/// The accept loop of the holder on the non-blocking `hoerer`: one thread per connection, at most `max` at once (a
/// connection above that is closed unanswered), until `halt` is set.
pub fn annehmen(d: &Arc<Dienst>, hoerer: &UnixListener, halt: &AtomicBool, max: usize) {
    let offen = Arc::new(AtomicUsize::new(0));
    while !halt.load(Ordering::Relaxed) {
        match hoerer.accept() {
            Ok((strom, _)) => {
                if strom.set_nonblocking(false).is_err() || offen.load(Ordering::Relaxed) >= max {
                    continue; // closed unanswered
                }
                offen.fetch_add(1, Ordering::Relaxed);
                let d = Arc::clone(d);
                let offen2 = Arc::clone(&offen);
                let gestartet = std::thread::Builder::new().spawn(move || {
                    d.verbindung(strom);
                    offen2.fetch_sub(1, Ordering::Relaxed);
                });
                if gestartet.is_err() {
                    offen.fetch_sub(1, Ordering::Relaxed);
                }
            }
            Err(e) => match pause_nach(e.kind()) {
                Pause::Bereitschaft => {
                    let mut fds = [PollFd::new(hoerer, PollFlags::IN)];
                    let t = rustix::time::Timespec {
                        tv_sec: 0,
                        tv_nsec: 200_000_000,
                    };
                    let _ = poll(&mut fds, Some(&t));
                }
                Pause::Ruhe => std::thread::sleep(Duration::from_millis(50)),
            },
        }
    }
}

/// `true` for a read on a non-blocking socket that only has nothing yet (`WouldBlock`) or was interrupted: the open
/// stream waits in `poll`; every other error ends it. A pure function, so that a test can give every kind of error.
#[must_use]
pub fn nur_warten(art: io::ErrorKind) -> bool {
    matches!(art, io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted)
}

fn blockierend(
    strom: &mut UnixStream,
    f: impl FnOnce(&mut UnixStream) -> io::Result<()>,
) -> io::Result<()> {
    strom.set_nonblocking(false)?;
    let r = f(strom);
    strom.set_nonblocking(true)?;
    r
}

#[cfg(test)]
mod tests {
    /// A source of bytes for the reader: the parts in order, then the end (0 bytes).
    struct Quelle(std::collections::VecDeque<std::io::Result<Vec<u8>>>);

    impl std::io::Read for Quelle {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            match self.0.pop_front() {
                None => Ok(0),
                Some(Err(e)) => Err(e),
                Some(Ok(mut t)) => {
                    let n = t.len().min(buf.len());
                    buf[..n].copy_from_slice(&t[..n]);
                    if n < t.len() {
                        self.0.push_front(Ok(t.split_off(n)));
                    }
                    Ok(n)
                }
            }
        }
    }

    fn leser() -> super::Leser {
        super::Leser {
            vorrat: Vec::new(),
            ohne_lf: 0,
        }
    }

    fn lesen(teile: Vec<std::io::Result<Vec<u8>>>, frist: Duration) -> Vec<String> {
        let mut q = Quelle(teile.into());
        let mut l = leser();
        let mut aus = Vec::new();
        loop {
            assert!(aus.len() < 1000, "the reader gives lines without end");
            match l.naechste(&mut q, frist, |_, _| Ok(())) {
                super::Zeile::Da(z) => aus.push(String::from_utf8(z).expect("utf-8")),
                super::Zeile::ZuGross => {
                    aus.push("<zu gross>".to_owned());
                    return aus;
                }
                super::Zeile::Ende => return aus,
            }
        }
    }

    use std::io::{Error, ErrorKind};
    use std::time::Duration;

    const S: Duration = Duration::from_secs(10);

    #[test]
    fn der_vorrat_gibt_zeilen_ohne_lf_und_behaelt_den_rest() {
        let mut l = leser();
        assert_eq!(l.aus_vorrat(), None);
        l.vorrat.extend_from_slice(b"ab\ncd\nef");
        assert_eq!(l.aus_vorrat().as_deref(), Some(&b"ab"[..]));
        assert_eq!(l.aus_vorrat().as_deref(), Some(&b"cd"[..]));
        assert_eq!(l.aus_vorrat(), None);
        assert_eq!(l.ohne_lf, 2);
        l.vorrat.extend_from_slice(b"g\n\n");
        assert_eq!(l.aus_vorrat().as_deref(), Some(&b"efg"[..]));
        assert_eq!(l.aus_vorrat().as_deref(), Some(&b""[..]));
        assert_eq!(l.aus_vorrat(), None);
        assert_eq!(l.vorrat, Vec::<u8>::new());
    }

    #[test]
    fn zeilen_ueber_teilgrenzen_und_das_ende() {
        let teile = vec![Ok(b"a".to_vec()), Ok(b"b\nc".to_vec()), Ok(b"\n".to_vec())];
        assert_eq!(lesen(teile, S), ["ab", "c"]);
    }

    #[test]
    fn eine_unterbrechung_liest_weiter_ein_anderer_fehler_endet() {
        let teile = vec![
            Ok(b"a".to_vec()),
            Err(Error::from(ErrorKind::Interrupted)),
            Ok(b"\n".to_vec()),
        ];
        assert_eq!(lesen(teile, S), ["a"]);
        let teile = vec![
            Ok(b"a\n".to_vec()),
            Err(Error::from(ErrorKind::ConnectionReset)),
            Ok(b"b\n".to_vec()),
        ];
        assert_eq!(lesen(teile, S), ["a"]);
    }

    #[test]
    fn eine_verstrichene_frist_liest_nicht_mehr() {
        assert_eq!(
            lesen(vec![Ok(b"a\n".to_vec())], Duration::ZERO),
            Vec::<String>::new()
        );
        // the deadline that cannot be set ends the connection too
        let mut q = Quelle(vec![Ok(b"a\n".to_vec())].into());
        let r = leser().naechste(&mut q, S, |_, _| Err(Error::from(ErrorKind::InvalidInput)));
        assert!(matches!(r, super::Zeile::Ende));
    }

    #[test]
    fn genau_zeile_max_ohne_lf_wartet_auf_das_lf() {
        let max = crate::pruefung::ZEILE_MAX;
        // exactly ZEILE_MAX bytes without LF, then the LF: a line of ZEILE_MAX bytes
        let mut l = leser();
        let mut q = Quelle(vec![Ok(vec![b'x'; max]), Ok(b"\n".to_vec())].into());
        match l.naechste(&mut q, S, |_, _| Ok(())) {
            super::Zeile::Da(z) => assert_eq!(z.len(), max),
            _ => panic!("no line of exactly ZEILE_MAX bytes"),
        }
        // one byte more without LF is too big, before the LF comes
        let mut l = leser();
        let mut q = Quelle(vec![Ok(vec![b'x'; max + 1]), Ok(b"\n".to_vec())].into());
        assert!(matches!(
            l.naechste(&mut q, S, |_, _| Ok(())),
            super::Zeile::ZuGross
        ));
        // a line with LF but one byte too long is too big
        let mut l = leser();
        let mut z = vec![b'x'; max + 1];
        z.push(b'\n');
        let mut q = Quelle(vec![Ok(z)].into());
        assert!(matches!(
            l.naechste(&mut q, S, |_, _| Ok(())),
            super::Zeile::ZuGross
        ));
    }

    #[test]
    fn der_strom_wartet_nur_ohne_daten_oder_nach_einer_unterbrechung() {
        assert!(super::nur_warten(ErrorKind::WouldBlock));
        assert!(super::nur_warten(ErrorKind::Interrupted));
        for art in [
            ErrorKind::ConnectionReset,
            ErrorKind::BrokenPipe,
            ErrorKind::TimedOut,
            ErrorKind::UnexpectedEof,
            ErrorKind::Other,
        ] {
            assert!(!super::nur_warten(art), "{art:?}");
        }
    }

    static UHR: std::sync::atomic::AtomicI64 = std::sync::atomic::AtomicI64::new(0);

    fn uhr() -> i64 {
        UHR.load(std::sync::atomic::Ordering::Relaxed)
    }

    fn dienst_ohne_ziele() -> super::Dienst {
        let k = crate::konfig::lesen(
            br#"{"fassung":1,"konten":{"1000":{"person":null}},"rollen":{},"ziele":{},"server":{},"kids":{}}"#,
        )
        .expect("konfig");
        super::Dienst::neu(
            k,
            crate::ablage::Ablage::neu(std::path::Path::new("/srv/beispiel/heim"), 1000),
            1000,
            0,
            std::collections::BTreeMap::new(),
            None,
            uhr,
        )
    }

    /// A redeemed `einmal` is kept while its `ablauf` lies after now: at `ablauf` itself it is gone.
    #[test]
    fn eingeloestes_gilt_bis_vor_dem_ablauf() {
        let d = dienst_ohne_ziele();
        UHR.store(1_000, std::sync::atomic::Ordering::Relaxed);
        assert!(d.einloesen("e-1", 2_000));
        assert!(!d.einloesen("e-1", 2_000));
        UHR.store(1_999, std::sync::atomic::Ordering::Relaxed);
        assert!(!d.einloesen("e-1", 3_000));
        UHR.store(2_000, std::sync::atomic::Ordering::Relaxed);
        assert!(d.einloesen("e-1", 3_000));
    }

    #[test]
    fn die_kette_meldet_ihren_fehler_einmal() {
        let d = dienst_ohne_ziele();
        assert!(d.erste_meldung());
        assert!(!d.erste_meldung());
        assert!(!d.erste_meldung());
    }

    #[test]
    fn nur_ein_leerer_hoerer_wartet_in_poll() {
        use super::{Pause, pause_nach};
        use std::io::ErrorKind;
        assert_eq!(pause_nach(ErrorKind::WouldBlock), Pause::Bereitschaft);
        for art in [
            ErrorKind::Interrupted,
            ErrorKind::ConnectionAborted,
            ErrorKind::OutOfMemory,
            ErrorKind::Other,
        ] {
            assert_eq!(pause_nach(art), Pause::Ruhe, "{art:?}");
        }
    }

    #[test]
    fn uhr_ist_die_echte() {
        let ms = super::jetzt_ms();
        // after 2026-01-01 and before 2100-01-01
        assert!(ms > 1_767_225_600_000 && ms < 4_102_444_800_000, "{ms}");
    }
}
