//! The server under Linux: one thread per connection, one request after the other on a connection (section 3), the
//! peer check before the first line (section 2.3), the order of checks of [`crate::pruefung`] and the execution of
//! stages 28 to 31 with the leak lock (section 8). The tickets redeemed are held for all connections under one lock.

use std::collections::{BTreeMap, BTreeSet};
use std::io::{self, Read, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use rustix::event::{PollFd, PollFlags, poll};
use serde_json::{Map, Value, json};

use crate::ablage::Ablage;
use crate::antwort;
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
    kette_gemeldet: std::sync::atomic::AtomicBool,
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
    fn naechste(&mut self, strom: &mut UnixStream, frist: Duration) -> Zeile {
        let ende = Instant::now() + frist;
        let mut buf = vec![0u8; 65_536];
        loop {
            if let Some(z) = self.aus_vorrat() {
                if z.len() > ZEILE_MAX {
                    return Zeile::ZuGross;
                }
                return Zeile::Da(z);
            }
            if self.vorrat.len() > ZEILE_MAX {
                return Zeile::ZuGross;
            }
            let rest = ende.saturating_duration_since(Instant::now());
            if rest.is_zero() || strom.set_read_timeout(Some(rest)).is_err() {
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
            kette_gemeldet: std::sync::atomic::AtomicBool::new(false),
        }
    }

    /// A hit of the leak lock: the fixed journal line on stderr (section 9), counted.
    fn leck(&self) {
        self.leck_treffer.fetch_add(1, Ordering::Relaxed);
        kette::leck_gesperrt(&mut io::stderr());
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

    /// Writes the line of a refused ticket to the chain. When the chain cannot be continued (a broken anchor or any
    /// other failure), the first such failure of the run writes one fixed line to the journal (F16), no path, no content.
    fn kette_schreiben(&self, code: Code, sch: &pruefung::ScheinAbgelehnt) {
        let Some(k) = &self.kette else {
            return;
        };
        if let Err(z) = kette::schein_abgelehnt(k, (self.uhr)(), code, sch)
            && !self.kette_gemeldet.swap(true, Ordering::Relaxed)
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
            let zeile = match leser.naechste(&mut strom, frist) {
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
                let name = s(o, "name");
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
                    self.leck();
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
        if l.tcp().set_write_timeout(Some(FRIST_KOPF)).is_err()
            || l.write_all(anfrage.bytes()).is_err()
        {
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
            self.leck();
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
        let mut sperre = Sperre::neu(werte);
        loop {
            match koerper.naechstes() {
                Ok(Some(b)) => {
                    let Ok(frei) = sperre.schieben(&b) else {
                        self.leck();
                        return senden(strom, &antwort::ende_fehler(&g.id, Code::LeckGesperrt))
                            .is_ok();
                    };
                    if teile_senden(strom, &g.id, &frei).is_err() {
                        return false;
                    }
                }
                Ok(None) => {
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
                self.leck();
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
        if l.tcp().set_nonblocking(true).is_err() || strom.set_nonblocking(true).is_err() {
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
                    Err(e)
                        if matches!(
                            e.kind(),
                            io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                        ) =>
                    {
                        break;
                    }
                    Err(_) => return Schluss::Abriss(Code::ZielFehler),
                }
            }
            // then the lines of the bridge
            loop {
                if let Some(z) = leser.aus_vorrat() {
                    if z.len() > ZEILE_MAX {
                        let _ = blockierend(strom, |s| {
                            senden(s, &antwort::fehler(None, Code::ZuGross))
                        });
                        return Schluss::Weg;
                    }
                    match self.strom_zeile(strom, aufrufer, id, l, &z) {
                        Some(s) => return s,
                        None => continue,
                    }
                }
                if leser.vorrat.len() > ZEILE_MAX {
                    let _ =
                        blockierend(strom, |s| senden(s, &antwort::fehler(None, Code::ZuGross)));
                    return Schluss::Weg;
                }
                match strom.read(&mut buf) {
                    Ok(0) => return Schluss::Weg,
                    Ok(n) => leser.vorrat.extend_from_slice(&buf[..n]),
                    Err(e)
                        if matches!(
                            e.kind(),
                            io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                        ) =>
                    {
                        break;
                    }
                    Err(_) => return Schluss::Weg,
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
    #[test]
    fn uhr_ist_die_echte() {
        let ms = super::jetzt_ms();
        // after 2026-01-01 and before 2100-01-01
        assert!(ms > 1_767_225_600_000 && ms < 4_102_444_800_000, "{ms}");
    }
}
