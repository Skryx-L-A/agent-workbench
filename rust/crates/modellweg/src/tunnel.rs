//! The service of mode 2 `kopf-einsetzen` (contract 8, section 13): CONNECT on the socket, TLS as a server with a leaf of
//! the throw-away CA, the requests in the tunnel one after another, each with its own connection to the target, the key
//! header only at bound targets, the answer with the leak lock, a WebSocket handed through. Linux only (OpenSSL).

use std::io::{self, Read, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use openssl::ssl::{NameType, SniError, SslAcceptor, SslAlert, SslMethod, SslStream};

use crate::betrieb::{Abbruch, Betrieb, Huelle, Server, TUNNEL_MAX};
use crate::betriebsart::kopf_einsetzen::{self, Meldung, Rahmen, an_ziel, antwortkopf_bleibt};
use crate::betriebsart::zielliste::{Ziel, Zielliste};
use crate::ca::Ca;
use crate::halter;
use crate::http::{self, Antwortkopf, KoerperLeser, Lesefehler, name_ok};
use crate::probelauf::Zaehlung;
use crate::protokoll::{self, KopfZeile};
use crate::sperre::{Sperre, enthaelt};
use crate::websocket::{Abriss, Filter, Rahmensperre, Zaehlrahmen};
use crate::ziel::Verbinder;

/// Deadline of each read from the harness.
const HARNESS_FRIST: Duration = Duration::from_secs(60);
/// Deadline of one round of polling both ways of a switched connection.
const TAKT: Duration = Duration::from_millis(20);
const BLOCK: usize = 16_384;

/// The service of one process of mode 2.
pub struct Tunnel {
    zielliste: Zielliste,
    ca: Ca,
    verbinder: Vec<Option<Box<dyn Verbinder>>>,
    sitzung: String,
    route: String,
    protokoll: PathBuf,
    probelauf: Option<Zaehlung>,
    pfad_voll: bool,
    eigene_uid: u32,
    halter_socket: PathBuf,
    betrieb: Betrieb,
}

/// What one tunnel or request ends with, for its line.
struct Ende {
    ergebnis: &'static str,
    grund: Option<&'static str>,
    status: Option<u16>,
    meldung: Meldung,
    weiter: bool,
}

impl Ende {
    fn weiter(status: u16, meldung: Meldung, offen: bool) -> Self {
        Self {
            ergebnis: "weitergereicht",
            grund: None,
            status: Some(status),
            meldung,
            weiter: offen,
        }
    }

    fn leck(status: u16, meldung: Meldung) -> Self {
        Self {
            ergebnis: "leck_gesperrt",
            grund: Some("mw.leck_gesperrt"),
            status: Some(status),
            meldung,
            weiter: false,
        }
    }

    /// A switched connection torn down (section 13.4, W7): the reason of the tear-down (a key gives `leck_gesperrt`, the
    /// trial-run reasons `abgelehnt`, the others `ziel_fehler`), status 101.
    fn abriss(a: Abriss, meldung: Meldung) -> Self {
        let mut e = Self::ablehnung(a.code(), meldung);
        e.status = Some(101);
        e
    }

    /// A failure of the holder after the head of the answer already went to the harness: the status stays the one
    /// that went out, and the line names the own code (section 13.4).
    fn spaet(code: &'static str, status: u16, meldung: Meldung) -> Self {
        let mut e = Self::ablehnung(code, meldung);
        e.status = Some(status);
        e
    }

    fn ablehnung(code: &'static str, meldung: Meldung) -> Self {
        let ergebnis = match code {
            "mw.halter_kein_schluessel"
            | "mw.halter_fehler"
            | "mw.halter_frist"
            | "mw.ziel_nicht_erreichbar"
            | "mw.ziel_frist"
            | "mw.antwort_komprimiert" => "ziel_fehler",
            "mw.leck_gesperrt" => "leck_gesperrt",
            _ => "abgelehnt",
        };
        Self {
            ergebnis,
            grund: Some(code),
            status: kopf_einsetzen::ablehnung(code).map(|(s, _)| s),
            meldung,
            weiter: false,
        }
    }
}

impl Server for Tunnel {
    fn bediene(&self, harness: UnixStream) {
        let _ = harness.set_read_timeout(Some(HARNESS_FRIST));
        let Some(anmeldung) = self.betrieb.anmelden(&harness) else {
            return;
        };
        let mut huelle = anmeldung.abbruch.huelle(harness);
        self.tunnel(&mut huelle, &anmeldung.abbruch);
        let _ = huelle.shutdown(std::net::Shutdown::Both);
        drop(anmeldung);
    }

    fn betrieb(&self) -> &Betrieb {
        &self.betrieb
    }
}

impl Tunnel {
    /// The service for a checked target list and CA; `verbinder` has one connector per target, in the order of the
    /// list (`None` for a bound target, whose way out belongs to the holder, and for an unbound target that got no
    /// address at the start).
    ///
    /// `halter_socket` is the endpoint of the holder. The program itself always hands in the fixed path
    /// ([`halter::fester_pfad`], the only call), never an option and never a value of the caller; a test hands in the
    /// path of its own holder. The check of the counterparty runs on it as it does on the fixed one.
    #[must_use]
    #[allow(
        clippy::too_many_arguments,
        reason = "one value per part of the start, each named"
    )]
    pub fn neu(
        zielliste: Zielliste,
        ca: Ca,
        verbinder: Vec<Option<Box<dyn Verbinder>>>,
        sitzung: String,
        route: String,
        protokoll: PathBuf,
        probelauf: Option<Zaehlung>,
        pfad_voll: bool,
        eigene_uid: u32,
        halter_socket: PathBuf,
        betrieb: Betrieb,
    ) -> Self {
        Self {
            zielliste,
            ca,
            verbinder,
            sitzung,
            route,
            protokoll,
            probelauf,
            pfad_voll,
            eigene_uid,
            halter_socket,
            betrieb,
        }
    }

    /// Section 13.1: the CONNECT, then the requests in the tunnel.
    fn tunnel(&self, h: &mut Huelle, abbruch: &Abbruch) {
        if self.betrieb.protokoll_kaputt.load(Ordering::SeqCst) {
            let _ = http::lies_bis_leerzeile(h, Vec::new());
            let _ = h.write_all(&kopf_einsetzen::antwort("mw.protokoll_fehlt"));
            return;
        }
        let ohne = || Meldung::default();
        let connect = http::lies_bis_leerzeile(h, Vec::new())
            .map_or(Connect::Unlesbar, |(kopf, _)| connect_ziel(&kopf));
        if connect == Connect::Unlesbar {
            return self.abweisen(h, None, "mw.anfrage_unlesbar");
        }
        let Some(_platz) = self.betrieb.platz(TUNNEL_MAX) else {
            return self.abweisen(h, None, "mw.ausgelastet");
        };
        let Connect::Ziel(ziel_text) = connect else {
            return self.abweisen(h, None, "mw.nur_connect");
        };
        let Some(nummer) = self
            .zielliste
            .ziele
            .iter()
            .position(|z| ziel_text == format!("{}:{}", z.host, z.port))
        else {
            return self.abweisen(h, None, "mw.ziel_nicht_erlaubt");
        };
        let ziel = &self.zielliste.ziele[nummer];
        // Stage 6 is for an unbound target that got no address at the start. A bound target has no connector of its
        // own: its way out belongs to the holder (section 5.1).
        if !ziel.gebunden() && self.verbinder[nummer].is_none() {
            return self.abweisen(h, Some(ziel), "mw.ziel_nicht_erreichbar");
        }
        if h.write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n")
            .is_err()
        {
            return;
        }
        let Ok(strom) = h.try_clone() else { return };
        let sni_falsch = Arc::new(AtomicBool::new(false));
        let Some(mut tls) = self.tls_annehmen(strom, &ziel.host, &sni_falsch) else {
            if sni_falsch.load(Ordering::SeqCst) {
                let mut e = Ende::ablehnung("mw.sni_falsch", ohne());
                e.status = None;
                self.zeile(Some(ziel), &e);
            }
            return;
        };
        loop {
            let e = self.anfrage(&mut tls, nummer, abbruch);
            let Some(e) = e else { return };
            self.zeile(Some(ziel), &e);
            if !e.weiter {
                return;
            }
        }
    }

    /// An own answer before the tunnel and its line (`ziel` null).
    fn abweisen(&self, h: &mut Huelle, ziel: Option<&Ziel>, code: &'static str) {
        let _ = h.write_all(&kopf_einsetzen::antwort(code));
        self.zeile(ziel, &Ende::ablehnung(code, Meldung::default()));
    }

    /// TLS as a server with the leaf for `host`; only the name `host` in the greeting is accepted, only `http/1.1`.
    fn tls_annehmen(
        &self,
        strom: Huelle,
        host: &str,
        sni_falsch: &Arc<AtomicBool>,
    ) -> Option<SslStream<Huelle>> {
        let blatt = self.ca.blatt(host).ok()?;
        let mut b = SslAcceptor::mozilla_intermediate_v5(SslMethod::tls_server()).ok()?;
        b.set_private_key(&blatt.1).ok()?;
        b.set_certificate(&blatt.0).ok()?;
        b.add_extra_chain_cert(self.ca.zertifikat().clone()).ok()?;
        b.set_alpn_select_callback(|_, angebot| {
            openssl::ssl::select_next_proto(b"\x08http/1.1", angebot)
                .ok_or(openssl::ssl::AlpnError::NOACK)
        });
        let soll = host.to_owned();
        let merker = Arc::clone(sni_falsch);
        b.set_servername_callback(move |ssl, alert| {
            if ssl.servername(NameType::HOST_NAME) == Some(soll.as_str()) {
                Ok(())
            } else {
                merker.store(true, Ordering::SeqCst);
                *alert = SslAlert::UNRECOGNIZED_NAME;
                Err(SniError::ALERT_FATAL)
            }
        });
        let annehmer = b.build();
        let tls = annehmer.accept(strom).ok()?;
        if tls.ssl().servername(NameType::HOST_NAME) == Some(host) {
            Some(tls)
        } else {
            sni_falsch.store(true, Ordering::SeqCst);
            None
        }
    }

    /// One request in the tunnel (section 13.2) and its answer (section 13.3). `None`: the tunnel ended without a request.
    fn anfrage(
        &self,
        tls: &mut SslStream<Huelle>,
        nummer: usize,
        abbruch: &Abbruch,
    ) -> Option<Ende> {
        let ziel = &self.zielliste.ziele[nummer];
        let Ok((kopf, rest)) = http::lies_bis_leerzeile(tls, Vec::new()) else {
            return None;
        };
        let ablehnen = |tls: &mut SslStream<Huelle>, code: &'static str, m: Meldung| {
            let _ = tls.write_all(&kopf_einsetzen::antwort(code));
            Some(Ende::ablehnung(code, m))
        };
        let Some(k) = kopf_einsetzen::zerlege(&kopf) else {
            return ablehnen(tls, "mw.anfrage_unlesbar", Meldung::default());
        };
        let g = match kopf_einsetzen::pruefe(ziel, &k, self.pfad_voll) {
            Ok(g) => g,
            Err((code, m)) => return ablehnen(tls, code, m),
        };
        let mut meldung = g.meldung.clone();
        // A bound target takes its body as `koerper_b64` in one piece: the holder sets the framing, so a chunked body
        // is unchunked here. An unbound target gets the framing byte for byte as it came.
        let koerper = match lies_koerper(tls, rest, g.rahmen, ziel.gebunden()) {
            Ok(k) => k,
            Err(code) => return ablehnen(tls, code, meldung),
        };
        meldung.kopf = Some(if ziel.gebunden() {
            kopf_einsetzen::KOPF_GEBUNDEN
        } else {
            "keiner"
        });
        if let Some(z) = &self.probelauf
            && z.zaehle(&koerper).is_err()
        {
            return ablehnen(tls, "mw.zaehlung_fehler", meldung);
        }
        if ziel.gebunden() {
            let Some((pfad, abfrage, kopf)) = kopf_einsetzen::an_halter(&k) else {
                return ablehnen(tls, "mw.anfrage_unlesbar", meldung);
            };
            let auftrag = Halterauftrag {
                ziel,
                methode: &k.methode,
                pfad: &pfad,
                abfrage: abfrage.as_deref(),
                kopf: &kopf,
                koerper: &koerper,
                umstellen: g.umstellen,
            };
            return Some(self.ueber_halter(tls, &auftrag, meldung));
        }
        let mut anfrage = an_ziel(&k);
        anfrage.extend_from_slice(&koerper);
        let ende = self.weiterreichen(tls, nummer, g.umstellen, &mut anfrage, abbruch, meldung);
        anfrage.fill(0);
        Some(ende)
    }

    /// The way to an **unbound** target and its answer to the harness. It never carries a key, so the leak lock runs
    /// over an empty list of keys; a bound target does not come here (it goes to the holder).
    fn weiterreichen(
        &self,
        tls: &mut SslStream<Huelle>,
        nummer: usize,
        umstellen: bool,
        anfrage: &mut [u8],
        abbruch: &Abbruch,
        meldung: Meldung,
    ) -> Ende {
        let ziel = &self.zielliste.ziele[nummer];
        let ablehnen = |tls: &mut SslStream<Huelle>, code: &'static str, m: Meldung| {
            let _ = tls.write_all(&kopf_einsetzen::antwort(code));
            Ende::ablehnung(code, m)
        };
        let Some(verbindung) = self.verbinder[nummer].as_ref().and_then(|v| v.verbinde()) else {
            return ablehnen(tls, "mw.ziel_nicht_erreichbar", meldung);
        };
        abbruch.setze_ziel(&self.betrieb, verbindung.abreissen);
        let lesefrist = verbindung.lesefrist;
        let mut strom = verbindung.strom;
        if strom
            .write_all(anfrage)
            .and_then(|()| strom.flush())
            .is_err()
        {
            return ablehnen(tls, "mw.ziel_nicht_erreichbar", meldung);
        }
        let (kopf, rest) = match http::lies_antwortkopf(&mut strom) {
            Ok(x) => x,
            Err(Lesefehler::Frist) => return ablehnen(tls, "mw.ziel_frist", meldung),
            Err(Lesefehler::Sonst) => return ablehnen(tls, "mw.ziel_nicht_erreichbar", meldung),
        };
        let umgestellt = kopf.status == 101 && umstellen && ziel.upgrade_durchreichen;
        let schluessel: Vec<&[u8]> = Vec::new();
        // W2: a negotiated extension would blind the lock; the 101 with one is replaced by 502. Only a bound target may
        // hand an upgrade through (section 12, `zl.upgrade_wert`), so every switch here is one of a bound target.
        if umgestellt
            && kopf
                .felder
                .iter()
                .any(|(n, _)| n == "sec-websocket-extensions")
        {
            return ablehnen(tls, "mw.antwort_komprimiert", meldung);
        }
        let felder = behaltene_felder(&kopf, umgestellt);
        if felder
            .iter()
            .any(|(_, w)| schluessel.iter().any(|s| enthaelt(w, s)))
        {
            return ablehnen(tls, "mw.leck_gesperrt", meldung);
        }
        let laenge = kopf.rahmung();
        let mut antwortkopf = format!("HTTP/1.1 {} \r\n", kopf.status).into_bytes();
        for (name, wert) in &felder {
            antwortkopf.extend_from_slice(name.as_bytes());
            antwortkopf.extend_from_slice(b": ");
            antwortkopf.extend_from_slice(wert);
            antwortkopf.extend_from_slice(b"\r\n");
        }
        if umgestellt {
            antwortkopf.extend_from_slice(b"\r\n");
            if tls.write_all(&antwortkopf).is_err() {
                return Ende::weiter(101, meldung, false);
            }
            lesefrist(Some(TAKT));
            let _ = tls.get_ref().set_read_timeout(Some(TAKT));
            // The lock on the payload of the frames (W3 to W5), not on the raw bytes. In the trial run the frames of the
            // harness are read and counted too (section 9, Z1 to Z3); without it that way stays unchecked (W6).
            let hin = self.probelauf.as_ref().map(Zaehlrahmen::neu);
            let ergebnis = bruecke_mit(tls, &mut strom, &rest, Rahmensperre::neu(&schluessel), hin);
            return match ergebnis {
                Ok(()) => Ende::weiter(101, meldung, false),
                Err(a) => Ende::abriss(a, meldung),
            };
        }
        let (rahmenkopf, mit_laenge) = rahmen_zum_harness(laenge);
        antwortkopf.extend_from_slice(&rahmenkopf);
        antwortkopf.extend_from_slice(b"\r\n");
        let Some(rahmung) = laenge else {
            return ablehnen(tls, "mw.ziel_nicht_erreichbar", meldung);
        };
        let komprimiert = kopf
            .felder
            .iter()
            .any(|(n, w)| n == "content-encoding" && !w.eq_ignore_ascii_case(b"identity"));
        if komprimiert && ziel.gebunden() {
            return ablehnen(tls, "mw.antwort_komprimiert", meldung);
        }
        let leser = KoerperLeser::neu(strom, rest, rahmung);
        let antwort = Antwortteil {
            kopf: antwortkopf,
            status: kopf.status,
            mit_laenge,
            komprimiert,
        };
        koerper_durch(tls, leser, &schluessel, &antwort, meldung)
    }

    /// The way over the holder for a bound target (section 5.1): one connection per request, no hello, the checked
    /// request as `modell.aufruf` or `modell.strom`, and the answer built out of `beginn`, `teil` and `ende`.
    /// Fail closed at every place: a failure of the holder or a tear-down ends the request, never with a fall back on
    /// a file. The model path holds no token for this target, so it runs no leak lock of its own here: that lock is
    /// the holder's (`halter.leck_gesperrt`, which becomes `mw.halter_fehler`).
    fn ueber_halter(
        &self,
        tls: &mut SslStream<Huelle>,
        a: &Halterauftrag<'_>,
        meldung: Meldung,
    ) -> Ende {
        let ablehnen = |tls: &mut SslStream<Huelle>, code: &'static str, m: Meldung| {
            let _ = tls.write_all(&kopf_einsetzen::antwort(code));
            Ende::ablehnung(code, m)
        };
        let name = a.ziel.halter_ziel.as_deref().unwrap_or_default();
        // In the session a holder that is gone is `mw.halter_fehler`; `mw.halter_fehlt` belongs to the start.
        let Ok(mut klient) = halter::Klient::verbinde(&self.halter_socket, self.eigene_uid) else {
            return ablehnen(tls, "mw.halter_fehler", meldung);
        };
        if klient.frist(Some(halter::FRIST_KOPF)).is_err() {
            return ablehnen(tls, "mw.halter_fehler", meldung);
        }
        let id = klient.naechste_id();
        let zeile = if a.umstellen {
            halter::modell_strom(id, name, a.pfad, a.abfrage, a.kopf)
        } else {
            halter::modell_aufruf(id, name, a.methode, a.pfad, a.abfrage, a.kopf, a.koerper)
        };
        let gesendet = zeile.and_then(|z| klient.senden(&z));
        if let Err(code) = gesendet {
            return ablehnen(tls, code, meldung);
        }
        let erste = match klient.naechste_zeile() {
            Ok(z) => z,
            Err(code) => return ablehnen(tls, code, meldung),
        };
        let (status, kopf) = match erste {
            halter::Zeile::Beginn { status, kopf } => (status, kopf),
            halter::Zeile::Ende(Some(code))
            | halter::Zeile::Antwort {
                code: Some(code), ..
            } => {
                return ablehnen(tls, halter::auf_mw(&code), meldung);
            }
            _ => return ablehnen(tls, "mw.halter_fehler", meldung),
        };
        let umgestellt = status == 101 && a.umstellen && a.ziel.upgrade_durchreichen;
        // The head of the answer: the headers of the holder as they came (his block list is the longer one and
        // already ran, section 13.3). He never sends `content-length` or `transfer-encoding`, so the body goes
        // chunked to the harness; a switched answer gets `upgrade` and `connection` from the model path itself.
        let mut antwortkopf = format!("HTTP/1.1 {status} \r\n").into_bytes();
        for (n, w) in &kopf {
            antwortkopf.extend_from_slice(format!("{n}: {w}\r\n").as_bytes());
        }
        if umgestellt {
            antwortkopf.extend_from_slice(b"upgrade: websocket\r\nconnection: Upgrade\r\n\r\n");
            if tls.write_all(&antwortkopf).is_err() {
                return Ende::weiter(101, meldung, false);
            }
            let _ = tls.get_ref().set_read_timeout(Some(TAKT));
            if klient.frist(Some(TAKT)).is_err() {
                return Ende::spaet("mw.halter_fehler", 101, meldung);
            }
            // Only the model path sees the bytes of the harness, so only it counts them in a trial run (Z1 to Z3).
            // The frames of the target are the holder's business (section 13.3).
            let hin = self.probelauf.as_ref().map(Zaehlrahmen::neu);
            return match bruecke_halter(tls, &mut klient, id, hin) {
                Ok(()) => Ende::weiter(101, meldung, false),
                Err(code) => Ende::spaet(code, 101, meldung),
            };
        }
        antwortkopf.extend_from_slice(b"transfer-encoding: chunked\r\n\r\n");
        koerper_vom_halter(tls, &mut klient, &antwortkopf, status, meldung)
    }

    /// Appends the line of a tunnel or a request; a failure blocks every further tunnel (`mw.protokoll_fehlt`).
    fn zeile(&self, ziel: Option<&Ziel>, e: &Ende) {
        let z = KopfZeile {
            zeit_ms: (self.betrieb.uhr)(),
            betriebsart: crate::Betriebsart::KopfEinsetzen.name().to_owned(),
            route: self.route.clone(),
            sitzung: self.sitzung.clone(),
            ziel: ziel.map(|z| z.name.clone()),
            methode: e.meldung.methode.map(str::to_owned),
            pfadregel: e.meldung.pfadregel.clone(),
            kopf: e.meldung.kopf.map(str::to_owned),
            ergebnis: e.ergebnis.to_owned(),
            grund: e.grund.map(str::to_owned),
            status: e.status.map(u32::from),
            // The path stands in the line only at a rejection and at a `GET` (section 8).
            pfad: e
                .meldung
                .pfad
                .clone()
                .filter(|_| e.ergebnis == "abgelehnt" || e.meldung.methode == Some("GET")),
            host: ziel.map(|z| z.host.clone()),
        };
        if protokoll::anhaengen_kopf(&self.protokoll, &z).is_err() {
            self.betrieb.protokoll_kaputt.store(true, Ordering::SeqCst);
        }
    }
}

/// The body of a plain answer of the holder: the lines `teil` as chunks to the harness, the line `ende` closes it.
/// A failure after the head went out keeps the status that went out and names the own code (section 13.4).
fn koerper_vom_halter(
    tls: &mut SslStream<Huelle>,
    klient: &mut halter::Klient,
    antwortkopf: &[u8],
    status: u16,
    meldung: Meldung,
) -> Ende {
    let mut kopf_gesendet = false;
    let schluss = |tls: &mut SslStream<Huelle>, code: &'static str, m: Meldung, gesendet: bool| {
        if gesendet {
            return Ende::spaet(code, status, m);
        }
        let _ = tls.write_all(&kopf_einsetzen::antwort(code));
        Ende::ablehnung(code, m)
    };
    loop {
        let z = match klient.naechste_zeile() {
            Ok(z) => z,
            Err(code) => return schluss(tls, code, meldung, kopf_gesendet),
        };
        match z {
            halter::Zeile::Teil(daten) => {
                if !kopf_gesendet {
                    kopf_gesendet = true;
                    if tls.write_all(antwortkopf).is_err() {
                        return Ende::weiter(status, meldung, false);
                    }
                }
                if stueck_senden(tls, &daten).is_err() {
                    return Ende::weiter(status, meldung, false);
                }
            }
            halter::Zeile::Ende(None) => {
                let abschluss = (|| -> io::Result<()> {
                    if !kopf_gesendet {
                        tls.write_all(antwortkopf)?;
                    }
                    tls.write_all(b"0\r\n\r\n")?;
                    tls.flush()
                })();
                return Ende::weiter(status, meldung, abschluss.is_ok());
            }
            halter::Zeile::Ende(Some(code)) => {
                return schluss(tls, halter::auf_mw(&code), meldung, kopf_gesendet);
            }
            halter::Zeile::Beginn { .. } | halter::Zeile::Antwort { .. } => {
                return schluss(tls, "mw.halter_fehler", meldung, kopf_gesendet);
            }
        }
    }
}

/// One chunk of a chunked answer to the harness; an empty part writes nothing.
fn stueck_senden(tls: &mut SslStream<Huelle>, bytes: &[u8]) -> io::Result<()> {
    if bytes.is_empty() {
        return Ok(());
    }
    tls.write_all(format!("{:x}\r\n", bytes.len()).as_bytes())?;
    tls.write_all(bytes)?;
    tls.write_all(b"\r\n")
}

/// The bridge of a switched connection over the holder (section 5.1): the bytes of the harness go as lines
/// `strom.teil` with the `id` of the stream, at most `TEIL_MAX` per line, the lines `teil` of the holder go as bytes
/// to the harness. The way from the harness is not checked (W6), only counted in a trial run. `Err` with the own code
/// when the stream tore down; nothing more then goes either way. A way of the harness that fails (a read that is no
/// passed deadline, a write, a tear-down of the service) ends the stream with `strom.ende` at once
/// ([`weg_gescheitert`]); a passed deadline of one round is no failure.
fn bruecke_halter(
    tls: &mut SslStream<Huelle>,
    klient: &mut halter::Klient,
    id: u64,
    mut hin: Option<Zaehlrahmen<'_>>,
) -> Result<(), &'static str> {
    let mut block = vec![0u8; BLOCK];
    let mut harness_zu = false;
    loop {
        // The harness closed: `strom.ende` went out, and the holder answers `ende`. From here the model path waits for
        // that line with the deadline of a head, so a silent holder ends the request instead of holding the thread.
        if harness_zu {
            return match klient.naechste_zeile()? {
                halter::Zeile::Teil(daten) => {
                    if tls.write_all(&daten).is_err() {
                        return Ok(());
                    }
                    continue;
                }
                halter::Zeile::Ende(None) => {
                    let _ = tls.flush();
                    Ok(())
                }
                halter::Zeile::Ende(Some(code)) => Err(halter::auf_mw(&code)),
                _ => Err("mw.halter_fehler"),
            };
        }
        {
            match tls.read(&mut block) {
                Ok(0) => {
                    harness_zu = true;
                    klient.senden(&halter::strom_ende(id)?)?;
                    klient.frist(Some(halter::FRIST_KOPF)).ok();
                }
                Ok(n) => {
                    let (geprueft, abriss) = match hin.as_mut() {
                        None => (None, None),
                        Some(z) => {
                            let (raus, abriss) = z.teil(&block[..n]);
                            (Some(raus), abriss)
                        }
                    };
                    let raus = geprueft.as_deref().unwrap_or(&block[..n]);
                    for stueck in raus.chunks(halter::TEIL_MAX) {
                        klient.senden(&halter::strom_teil(id, stueck)?)?;
                    }
                    if let Some(a) = abriss {
                        return Err(a.code());
                    }
                }
                Err(e) if frist_vorbei(&e) => {}
                Err(_) => return weg_gescheitert(klient, id),
            }
        }
        match klient.zeile_oder_nichts()? {
            None => {}
            Some(halter::Zeile::Teil(daten)) => {
                if tls.write_all(&daten).is_err() {
                    return weg_gescheitert(klient, id);
                }
            }
            Some(halter::Zeile::Ende(None)) => {
                let _ = tls.flush();
                return Ok(());
            }
            Some(halter::Zeile::Ende(Some(code))) => return Err(halter::auf_mw(&code)),
            Some(_) => return Err("mw.halter_fehler"),
        }
    }
}

/// The way of the harness failed while the stream is open (section 5.1): `strom.ende` goes to the holder before the
/// model path closes its connection to him, so that he sees the end of the stream as an end and not as a tear-down.
/// Nothing can reach the harness any more, so the model path does not wait for the `ende` of the holder; the stream
/// ends in every case.
fn weg_gescheitert(klient: &mut halter::Klient, id: u64) -> Result<(), &'static str> {
    klient.senden(&halter::strom_ende(id)?)
}

/// Whether a read ended because its deadline passed.
fn frist_vorbei(e: &io::Error) -> bool {
    matches!(
        e.kind(),
        io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
    )
}

/// The framing towards the harness: the length of the target if it had one, else chunked. Returns the header line
/// and whether it is a length.
fn rahmen_zum_harness(rahmung: Option<http::Rahmung>) -> (Vec<u8>, bool) {
    match rahmung {
        Some(http::Rahmung::Laenge(n)) => (format!("content-length: {n}\r\n").into_bytes(), true),
        _ => (b"transfer-encoding: chunked\r\n".to_vec(), false),
    }
}

/// One checked request to a bound target, as it goes to the holder (section 5.1).
struct Halterauftrag<'a> {
    ziel: &'a Ziel,
    methode: &'a str,
    pfad: &'a str,
    abfrage: Option<&'a str>,
    kopf: &'a [halter::Kopf],
    koerper: &'a [u8],
    umstellen: bool,
}

/// The head of an answer to the harness and how its body goes.
struct Antwortteil {
    kopf: Vec<u8>,
    status: u16,
    mit_laenge: bool,
    komprimiert: bool,
}

/// Hands the body through the leak lock: the head with the first free bytes, chunks when the target had no length.
fn koerper_durch<R: Read>(
    tls: &mut SslStream<Huelle>,
    mut leser: KoerperLeser<R>,
    schluessel: &[&[u8]],
    a: &Antwortteil,
    meldung: Meldung,
) -> Ende {
    let mut sperre = Sperre::neu(schluessel);
    let mut kopf_gesendet = false;
    let senden = |tls: &mut SslStream<Huelle>, bytes: &[u8]| -> io::Result<()> {
        if bytes.is_empty() {
            return Ok(());
        }
        if a.mit_laenge {
            tls.write_all(bytes)
        } else {
            tls.write_all(format!("{:x}\r\n", bytes.len()).as_bytes())?;
            tls.write_all(bytes)?;
            tls.write_all(b"\r\n")
        }
    };
    loop {
        match leser.teil() {
            Ok(Some(teil)) => {
                let frei = if a.komprimiert {
                    Some(teil)
                } else {
                    sperre.teil(&teil)
                };
                let Some(frei) = frei else {
                    if kopf_gesendet {
                        return Ende::leck(a.status, meldung);
                    }
                    let _ = tls.write_all(&kopf_einsetzen::antwort("mw.leck_gesperrt"));
                    return Ende::ablehnung("mw.leck_gesperrt", meldung);
                };
                if frei.is_empty() {
                    continue;
                }
                if !kopf_gesendet {
                    kopf_gesendet = true;
                    if tls.write_all(&a.kopf).is_err() {
                        return Ende::weiter(a.status, meldung, false);
                    }
                }
                if senden(tls, &frei).is_err() {
                    return Ende::weiter(a.status, meldung, false);
                }
            }
            Ok(None) => {
                let rest = sperre.ende();
                let abschluss = (|| -> io::Result<()> {
                    if !kopf_gesendet {
                        tls.write_all(&a.kopf)?;
                    }
                    senden(tls, &rest)?;
                    if !a.mit_laenge {
                        tls.write_all(b"0\r\n\r\n")?;
                    }
                    tls.flush()
                })();
                return Ende::weiter(a.status, meldung, abschluss.is_ok());
            }
            Err(_) => {
                if !kopf_gesendet {
                    let _ = tls.write_all(&a.kopf);
                }
                return Ende::weiter(a.status, meldung, false);
            }
        }
    }
}

/// What the head on the socket asks for (stages 2 and 4 of section 13.1).
#[derive(Debug, Clone, PartialEq, Eq)]
enum Connect {
    /// Not readable.
    Unlesbar,
    /// Readable, but not CONNECT.
    Anders,
    /// CONNECT to this request target.
    Ziel(String),
}

fn connect_ziel(kopf: &[u8]) -> Connect {
    connect_lesen(kopf).unwrap_or(Connect::Unlesbar)
}

fn connect_lesen(kopf: &[u8]) -> Option<Connect> {
    let text = std::str::from_utf8(kopf).ok()?;
    let mut zeilen = text.split("\r\n");
    let start = zeilen.next()?;
    let (methode, rest) = start.split_once(' ')?;
    let (ziel, version) = rest.split_once(' ')?;
    let gut = (1..=16).contains(&methode.len())
        && methode.bytes().all(|c| c.is_ascii_uppercase())
        && !ziel.is_empty()
        && ziel.bytes().all(|c| (0x21..=0x7e).contains(&c))
        && version == "HTTP/1.1";
    let felder_gut = zeilen.all(|z| {
        z.split_once(':').is_some_and(|(n, w)| {
            name_ok(n.as_bytes()) && w.bytes().all(|c| (0x20..=0x7e).contains(&c) || c == b'\t')
        })
    });
    if !(gut && felder_gut) {
        return None;
    }
    Some(if methode == "CONNECT" {
        Connect::Ziel(ziel.to_owned())
    } else {
        Connect::Anders
    })
}

/// The fields of the answer that go to the harness, with their names as sent.
fn behaltene_felder(kopf: &Antwortkopf, umgestellt: bool) -> Vec<(String, Vec<u8>)> {
    kopf.namen
        .iter()
        .zip(&kopf.felder)
        .filter(|(name, _)| antwortkopf_bleibt(name, umgestellt))
        .map(|(name, (_, wert))| (name.clone(), wert.clone()))
        .collect()
}

/// The body of a request: exactly `Content-Length` bytes (else `mw.anfrage_unlesbar`) or a chunked body. For an
/// unbound target the chunked framing goes out as sent; for a bound one only the data go out, because the body goes
/// to the holder as `koerper_b64` in one piece and he sets the framing. The limit of the data is the one of the
/// target (stage 7): 23 MiB bound, 32 MiB unbound.
fn lies_koerper<R: Read>(
    r: &mut R,
    rest: Vec<u8>,
    rahmen: Rahmen,
    gebunden: bool,
) -> Result<Vec<u8>, &'static str> {
    match rahmen {
        Rahmen::Ohne => Ok(Vec::new()),
        Rahmen::Laenge(n) => {
            let n = usize::try_from(n).map_err(|_| "mw.koerper_zu_gross")?;
            let mut k = rest;
            let fehlt =
                u64::try_from(n.saturating_sub(k.len())).map_err(|_| "mw.anfrage_unlesbar")?;
            r.take(fehlt)
                .read_to_end(&mut k)
                .map_err(|_| "mw.anfrage_unlesbar")?;
            k.truncate(n);
            if k.len() == n {
                Ok(k)
            } else {
                Err("mw.anfrage_unlesbar")
            }
        }
        Rahmen::Stueckweise => {
            let grenze = if gebunden {
                kopf_einsetzen::KOERPER_MAX_GEBUNDEN
            } else {
                kopf_einsetzen::KOERPER_MAX
            };
            stueckweise(r, rest, grenze, gebunden)
        }
    }
}

/// Reads a chunked body and returns its bytes with the framing as sent, up to 32 MiB of data.
#[cfg(test)]
fn stueckweise_roh<R: Read>(r: &mut R, puf: Vec<u8>) -> Result<Vec<u8>, &'static str> {
    stueckweise(r, puf, kopf_einsetzen::KOERPER_MAX, false)
}

/// Reads a chunked body: with `nur_daten` only the data of the chunks, else the framing as sent. More than `grenze`
/// bytes of data is `mw.koerper_zu_gross`.
fn stueckweise<R: Read>(
    r: &mut R,
    mut puf: Vec<u8>,
    grenze: u64,
    nur_daten: bool,
) -> Result<Vec<u8>, &'static str> {
    let mut roh = Vec::new();
    let mut daten: u64 = 0;
    let mut block = vec![0u8; BLOCK];
    let mut mehr = |puf: &mut Vec<u8>| -> Result<(), &'static str> {
        let n = r.read(&mut block).map_err(|_| "mw.anfrage_unlesbar")?;
        let neu = &block[..n];
        if neu.is_empty() {
            return Err("mw.anfrage_unlesbar");
        }
        puf.extend_from_slice(neu);
        Ok(())
    };
    loop {
        let zeile = loop {
            if let Some(i) = http::finde(&puf, b"\r\n") {
                break i;
            }
            if puf.len() > 4096 {
                return Err("mw.anfrage_unlesbar");
            }
            mehr(&mut puf)?;
        };
        let groesse = {
            let z = &puf[..zeile];
            let ziffern = z.split(|c| *c == b';').next().unwrap_or(z).trim_ascii();
            if !(1..=16).contains(&ziffern.len()) {
                return Err("mw.anfrage_unlesbar");
            }
            std::str::from_utf8(ziffern)
                .ok()
                .and_then(|t| u64::from_str_radix(t, 16).ok())
                .ok_or("mw.anfrage_unlesbar")?
        };
        daten = daten.saturating_add(groesse);
        if daten > grenze {
            return Err("mw.koerper_zu_gross");
        }
        let n = usize::try_from(groesse).map_err(|_| "mw.koerper_zu_gross")?;
        let bis = zeile + 2 + n + 2;
        if groesse == 0 {
            // The last chunk: trailer lines up to the empty line.
            loop {
                if let Some(i) = http::finde(&puf[zeile + 2..], b"\r\n\r\n").map(|i| i + zeile + 2)
                {
                    if !nur_daten {
                        roh.extend_from_slice(&puf[..i + 4]);
                    }
                    return Ok(roh);
                }
                if puf[zeile + 2..].starts_with(b"\r\n") {
                    if !nur_daten {
                        roh.extend_from_slice(&puf[..zeile + 4]);
                    }
                    return Ok(roh);
                }
                if puf.len() > zeile + 4096 {
                    return Err("mw.anfrage_unlesbar");
                }
                mehr(&mut puf)?;
            }
        }
        while puf.get(..bis).is_none() {
            mehr(&mut puf)?;
        }
        if &puf[bis - 2..bis] != b"\r\n" {
            return Err("mw.anfrage_unlesbar");
        }
        if nur_daten {
            roh.extend_from_slice(&puf[zeile + 2..bis - 2]);
        } else {
            roh.extend_from_slice(&puf[..bis]);
        }
        puf.drain(..bis);
    }
}

/// The polling of [`bruecke_mit`] with the plain leak lock on the raw bytes ([`Sperre`]), for the tests of both ways.
/// `Err` when a key was found (the connection to the harness is then torn down).
#[cfg(test)]
fn bruecke<H: Read + Write, Z: Read + Write>(
    tls: &mut H,
    ziel: &mut Z,
    rest: &[u8],
    schluessel: &[&[u8]],
) -> Result<(), ()> {
    bruecke_mit(tls, ziel, rest, Sperre::neu(schluessel), None).map_err(|_| ())
}

/// A switched connection (101): the way from the harness to the target byte for byte (W6) or, in the trial run, through
/// `hin` (whole frames only, counted first; section 9, Z1 to Z3), the way from the target to the harness through
/// `sperre`. `Err` with the reason when either tears the connection down (nothing more then goes either way). Both
/// streams read with a short deadline (`TAKT`), so one round polls each way once.
fn bruecke_mit<H: Read + Write, Z: Read + Write, F: Filter>(
    tls: &mut H,
    ziel: &mut Z,
    rest: &[u8],
    mut sperre: F,
    mut hin: Option<Zaehlrahmen<'_>>,
) -> Result<(), Abriss> {
    let mut block = vec![0u8; BLOCK];
    let erst = sperre.teil(rest)?;
    if tls.write_all(&erst).is_err() {
        return Ok(());
    }
    loop {
        match tls.read(&mut block) {
            Ok(0) => break,
            Ok(n) => {
                let (geprueft, abriss) = match hin.as_mut() {
                    None => (None, None),
                    Some(z) => {
                        let (raus, abriss) = z.teil(&block[..n]);
                        (Some(raus), abriss)
                    }
                };
                let raus = geprueft.as_deref().unwrap_or(&block[..n]);
                let gesendet = ziel.write_all(raus).is_ok();
                if let Some(a) = abriss {
                    return Err(a);
                }
                if !gesendet {
                    break;
                }
            }
            Err(e)
                if matches!(
                    e.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                ) => {}
            Err(_) => break,
        }
        match ziel.read(&mut block) {
            Ok(0) => break,
            Ok(n) => {
                let frei = sperre.teil(&block[..n])?;
                if tls.write_all(&frei).is_err() {
                    break;
                }
            }
            Err(e)
                if matches!(
                    e.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                ) => {}
            Err(_) => break,
        }
    }
    let _ = tls.write_all(&sperre.ende()?);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{Connect, connect_ziel, stueckweise_roh};

    #[test]
    fn the_connect_head() {
        assert_eq!(
            connect_ziel(b"CONNECT a.b:443 HTTP/1.1\r\nHost: a.b:443"),
            Connect::Ziel("a.b:443".to_owned())
        );
        assert_eq!(
            connect_ziel(b"GET http://a.b/ HTTP/1.1\r\nHost: a.b"),
            Connect::Anders
        );
        assert_eq!(connect_ziel(b"hallo"), Connect::Unlesbar);
        assert_eq!(
            connect_ziel(b"CONNECT a.b:443 HTTP/1.1\r\nA:\tb"),
            Connect::Ziel("a.b:443".to_owned())
        );
        assert_eq!(
            connect_ziel(b"CONNECT a.b:443 HTTP/1.1\r\nA: \x7f"),
            Connect::Unlesbar
        );
        assert_eq!(
            connect_ziel(b"CONNECT a.b:443 HTTP/1.1\r\nA b: c"),
            Connect::Unlesbar
        );
        assert_eq!(connect_ziel(b"CONNECT a.b:443 HTTP/1.0"), Connect::Unlesbar);
        assert_eq!(connect_ziel(b"CONNECT  HTTP/1.1"), Connect::Unlesbar);
        assert_eq!(
            connect_ziel(b"CONNECT a.b:443 HTTP/1.1\r\nkein feld"),
            Connect::Unlesbar
        );
        assert_eq!(connect_ziel(b"connect a.b:443 HTTP/1.1"), Connect::Unlesbar);
    }

    #[test]
    fn a_chunked_body_keeps_its_framing() {
        let roh = b"2\r\n{}\r\n0\r\n\r\nNEXT";
        assert_eq!(
            stueckweise_roh(&mut &b""[..], roh.to_vec()),
            Ok(b"2\r\n{}\r\n0\r\n\r\n".to_vec())
        );
        assert_eq!(
            stueckweise_roh(&mut &b"\r\n0\r\nT: 1\r\n\r\n"[..], b"2\r\n{}".to_vec()),
            Ok(b"2\r\n{}\r\n0\r\nT: 1\r\n\r\n".to_vec())
        );
        assert_eq!(
            stueckweise_roh(&mut &b""[..], b"2\r\n{}XX".to_vec()),
            Err("mw.anfrage_unlesbar")
        );
        assert_eq!(
            stueckweise_roh(&mut &b""[..], b"2\r\n{".to_vec()),
            Err("mw.anfrage_unlesbar")
        );
        assert_eq!(
            stueckweise_roh(&mut &b""[..], b"g\r\n".to_vec()),
            Err("mw.anfrage_unlesbar")
        );
        assert_eq!(
            stueckweise_roh(&mut &b""[..], b"2000001\r\n".to_vec()),
            Err("mw.koerper_zu_gross")
        );
    }
}

#[cfg(test)]
mod pruefungen {
    use super::{bruecke, lies_koerper, rahmen_zum_harness, stueckweise_roh};
    use crate::betriebsart::kopf_einsetzen::{KOERPER_MAX, Rahmen};
    use crate::http::Rahmung;
    use std::collections::VecDeque;
    use std::io::{self, Read, Write};

    /// A stream from a script: each read hands out one prepared result; writes are recorded.
    #[derive(Default)]
    struct Strom {
        lesen: VecDeque<io::Result<Vec<u8>>>,
        geschrieben: Vec<u8>,
    }

    impl Strom {
        fn neu(teile: Vec<io::Result<Vec<u8>>>) -> Self {
            Self {
                lesen: teile.into(),
                geschrieben: Vec::new(),
            }
        }
    }

    impl Read for Strom {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            match self.lesen.pop_front() {
                None => Ok(0),
                Some(Err(e)) => Err(e),
                Some(Ok(t)) => {
                    buf[..t.len()].copy_from_slice(&t);
                    Ok(t.len())
                }
            }
        }
    }

    impl Write for Strom {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.geschrieben.extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    fn warte() -> io::Result<Vec<u8>> {
        Err(io::ErrorKind::WouldBlock.into())
    }

    fn frist() -> io::Result<Vec<u8>> {
        Err(io::ErrorKind::TimedOut.into())
    }

    fn bruch() -> io::Result<Vec<u8>> {
        Err(io::ErrorKind::ConnectionReset.into())
    }

    #[test]
    fn the_bridge_polls_both_ways() {
        let mut h = Strom::neu(vec![warte(), Ok(b"zu".to_vec()), frist(), Ok(Vec::new())]);
        let mut z = Strom::neu(vec![
            Ok(b"ab".to_vec()),
            warte(),
            Ok(b"cd".to_vec()),
            frist(),
            Ok(b"ef".to_vec()),
        ]);
        assert_eq!(bruecke(&mut h, &mut z, b"r", &[b"KEY"]), Ok(()));
        assert_eq!(z.geschrieben, b"zu");
        assert_eq!(h.geschrieben, b"rabcd");
    }

    #[test]
    fn a_broken_way_ends_the_bridge() {
        let mut h = Strom::neu(vec![bruch(), Ok(b"nie".to_vec())]);
        let mut z = Strom::neu(vec![Ok(b"ab".to_vec())]);
        assert_eq!(bruecke(&mut h, &mut z, b"", &[b"K"]), Ok(()));
        assert_eq!(z.geschrieben, Vec::<u8>::new());
        assert_eq!(h.geschrieben, Vec::<u8>::new());
        let mut h = Strom::neu(vec![warte(), warte(), Ok(b"nie".to_vec())]);
        let mut z = Strom::neu(vec![bruch(), Ok(b"nie".to_vec())]);
        assert_eq!(bruecke(&mut h, &mut z, b"", &[b"K"]), Ok(()));
        assert_eq!(z.geschrieben, Vec::<u8>::new());
        assert_eq!(h.geschrieben, Vec::<u8>::new());
        let mut h = Strom::neu(vec![warte(), warte()]);
        let mut z = Strom::neu(vec![Ok(b"xKy".to_vec())]);
        assert_eq!(bruecke(&mut h, &mut z, b"", &[b"K"]), Err(()));
        assert_eq!(
            bruecke(&mut Strom::default(), &mut Strom::default(), b"K", &[b"K"]),
            Err(())
        );
    }

    #[test]
    fn the_framing_towards_the_harness() {
        assert_eq!(
            rahmen_zum_harness(Some(Rahmung::Laenge(7))),
            (b"content-length: 7\r\n".to_vec(), true)
        );
        assert_eq!(
            rahmen_zum_harness(Some(Rahmung::BisEnde)),
            (b"transfer-encoding: chunked\r\n".to_vec(), false)
        );
        assert_eq!(
            rahmen_zum_harness(None),
            (b"transfer-encoding: chunked\r\n".to_vec(), false)
        );
    }

    #[test]
    fn a_body_by_its_length() {
        assert_eq!(
            lies_koerper(&mut &b""[..], b"abcX".to_vec(), Rahmen::Laenge(3), false),
            Ok(b"abc".to_vec())
        );
        assert_eq!(
            lies_koerper(
                &mut &b"cdEXTRA"[..],
                b"ab".to_vec(),
                Rahmen::Laenge(4),
                false
            ),
            Ok(b"abcd".to_vec())
        );
        assert_eq!(
            lies_koerper(&mut &b"bcdefg"[..], b"a".to_vec(), Rahmen::Laenge(4), false),
            Ok(b"abcd".to_vec())
        );
        assert_eq!(
            lies_koerper(&mut &b""[..], b"ab".to_vec(), Rahmen::Laenge(2), false),
            Ok(b"ab".to_vec())
        );
        assert_eq!(
            lies_koerper(&mut &b"c"[..], b"ab".to_vec(), Rahmen::Laenge(4), false),
            Err("mw.anfrage_unlesbar")
        );
        assert_eq!(
            lies_koerper(&mut &b"x"[..], Vec::new(), Rahmen::Ohne, false),
            Ok(Vec::new())
        );
    }

    #[test]
    fn a_chunked_body_over_several_reads() {
        let mut r = &b"bc\r\n3\r\ndef\r\n0\r\nT: 1\r\n\r\nNEXT"[..];
        assert_eq!(
            stueckweise_roh(&mut r, b"3\r\na".to_vec()),
            Ok(b"3\r\nabc\r\n3\r\ndef\r\n0\r\nT: 1\r\n\r\n".to_vec())
        );
        let teile: Vec<io::Result<Vec<u8>>> =
            [&b"1\r"[..], b"\nx", b"\r\n0\r", b"\n", b"T: 1\r\n", b"\r\n"]
                .iter()
                .map(|t| Ok(t.to_vec()))
                .collect();
        let mut s = Strom::neu(teile);
        assert_eq!(
            stueckweise_roh(&mut s, Vec::new()),
            Ok(b"1\r\nx\r\n0\r\nT: 1\r\n\r\n".to_vec())
        );
        let mut s = Strom::neu(vec![Ok(b"0\r".to_vec()), Ok(b"\n\r\n".to_vec())]);
        assert_eq!(
            stueckweise_roh(&mut s, Vec::new()),
            Ok(b"0\r\n\r\n".to_vec())
        );
    }

    #[test]
    fn a_size_line_and_the_trailer_may_wait_up_to_their_limit() {
        let schluss = b"\r\nx\r\n0\r\n\r\n";
        let mut zeile = b"1;".to_vec();
        zeile.resize(4096, b'x');
        let mut ganz = zeile.clone();
        ganz.extend_from_slice(schluss);
        assert_eq!(stueckweise_roh(&mut &schluss[..], zeile.clone()), Ok(ganz));
        zeile.push(b'x');
        assert_eq!(
            stueckweise_roh(&mut &schluss[..], zeile),
            Err("mw.anfrage_unlesbar")
        );
        let mut nachspann = b"0\r\n".to_vec();
        nachspann.resize(4097, b'T');
        let mut ganz = nachspann.clone();
        ganz.extend_from_slice(b"\r\n\r\n");
        assert_eq!(
            stueckweise_roh(&mut &b"\r\n\r\n"[..], nachspann.clone()),
            Ok(ganz)
        );
        nachspann.push(b'T');
        assert_eq!(
            stueckweise_roh(&mut &b"\r\n\r\n"[..], nachspann),
            Err("mw.anfrage_unlesbar")
        );
    }

    #[test]
    fn a_chunked_body_has_limits() {
        let mut zeile = b"1;".to_vec();
        zeile.resize(4097, b'x');
        assert_eq!(
            stueckweise_roh(&mut &b""[..], zeile.clone()),
            Err("mw.anfrage_unlesbar")
        );
        zeile.truncate(4095);
        zeile.extend_from_slice(b"\r\nx\r\n0\r\n\r\n");
        assert!(stueckweise_roh(&mut &b""[..], zeile).is_ok());
        let grenze = format!("{KOERPER_MAX:x}\r\n").into_bytes();
        assert_eq!(
            stueckweise_roh(&mut &b""[..], grenze),
            Err("mw.anfrage_unlesbar")
        );
        let ueber = format!("{:x}\r\n", KOERPER_MAX + 1).into_bytes();
        assert_eq!(
            stueckweise_roh(&mut &b""[..], ueber),
            Err("mw.koerper_zu_gross")
        );
        let mut anhang = b"0\r\nT: ".to_vec();
        anhang.resize(4099, b'x');
        assert_eq!(
            stueckweise_roh(&mut &b""[..], anhang),
            Err("mw.anfrage_unlesbar")
        );
        let mut anhang = b"0\r\nT: ".to_vec();
        anhang.resize(4096, b'x');
        anhang.extend_from_slice(b"\r\n\r\n");
        assert!(stueckweise_roh(&mut &b""[..], anhang).is_ok());
        assert_eq!(
            stueckweise_roh(&mut &b"\r\n"[..], b"2\r\nab".to_vec()),
            Ok(b"2\r\nab\r\n".to_vec()).and(Err("mw.anfrage_unlesbar"))
        );
    }
}
