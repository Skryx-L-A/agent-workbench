//! Serving the harness (contract 8, sections 7 to 10): one request per connection, the ten stages in their order, the way
//! to the target, the answer with the leak lock, and one line per request.

use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;

use crate::Betriebsart;
use crate::betrieb::{Abbruch, Betrieb, Faden, Huelle, Platz, Server};
pub use crate::betrieb::{FAEDEN_MAX, PARALLEL_MAX, Uhr};
use crate::betriebsart::Regeln;
use crate::betriebsart::openai_lokal::{OpenaiLokal, Optionen};
use crate::codes::Code;
use crate::http::{self, KoerperLeser, Lesefehler};
use crate::pfad::pfad_feld;
use crate::probelauf::Zaehlung;
use crate::protokoll::{self, Modellweg};
use crate::schluessel::Schluessel;
use crate::sperre::{Sperre, enthaelt};
use crate::ziel::{Verbinder, Ziel};

/// Largest `Content-Length` of a request (stage 8).
pub const KOERPER_MAX: u64 = 33_554_432;
/// Deadline of each read from the harness (head and body).
const HARNESS_FRIST: Duration = Duration::from_secs(60);

/// What one process of mode 1 serves: the call, the key and the target.
pub struct Lage {
    /// The options of `openai-lokal`.
    pub art: Optionen,
    /// Session of the call.
    pub sitzung: String,
    /// Route of the call.
    pub route: String,
    /// Folder of the chain.
    pub protokoll: PathBuf,
    /// The key from the key file.
    pub schluessel: Schluessel,
    /// The marker count of a trial run (section 9), else `None`.
    pub probelauf: Option<Zaehlung>,
    /// Whether the whole path stands in the field `pfad` of the line (`--protokoll-pfad-voll`, section 8).
    pub pfad_voll: bool,
}

/// The result of one request for its line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Ausgang {
    ergebnis: &'static str,
    grund: Option<Code>,
    status: u16,
    tokens: Option<(u64, u64)>,
}

impl Ausgang {
    fn eigen(code: Code) -> Self {
        Self {
            ergebnis: code.ergebnis(),
            grund: Some(code),
            status: code.status(),
            tokens: None,
        }
    }
}

/// The service of one process of mode 1.
pub struct Dienst {
    lage: Lage,
    betriebsart: Betriebsart,
    modell: String,
    ziel: Ziel,
    regeln: Box<dyn Regeln>,
    verbinder: Box<dyn Verbinder>,
    betrieb: Betrieb,
}

impl Server for Dienst {
    fn bediene(&self, harness: UnixStream) {
        Dienst::bediene(self, harness);
    }

    fn betrieb(&self) -> &Betrieb {
        &self.betrieb
    }
}

impl Dienst {
    /// The service for `lage`; `verbinder` opens connections to the target, `uhr` gives the time of each line.
    #[must_use]
    pub fn neu(lage: Lage, verbinder: Box<dyn Verbinder>, uhr: Uhr) -> Self {
        Self {
            betriebsart: Betriebsart::OpenaiLokal,
            modell: lage.art.modell.clone(),
            ziel: lage.art.ziel.clone(),
            regeln: Box::new(OpenaiLokal::neu(lage.art.modell.clone())),
            lage,
            verbinder,
            betrieb: Betrieb::neu(uhr),
        }
    }

    /// Waits until fewer than [`FAEDEN_MAX`] connections are served and takes a place.
    #[must_use]
    pub fn faden(self: &Arc<Self>) -> Faden {
        let server: Arc<dyn Server> = Arc::clone(self) as Arc<dyn Server>;
        Faden::nimm(server)
    }

    /// Waits at most `frist` until no connection is served any more; true if none is left.
    #[must_use]
    pub fn warte_leer(&self, frist: Duration) -> bool {
        self.betrieb.warte_leer(frist)
    }

    /// End of the process: every running request is torn down (each still writes its line), new ones too.
    pub fn abreissen_alle(&self) {
        self.betrieb.abreissen_alle();
    }

    /// Serves one connection of the harness: one request, one answer, one line, then the connection is closed.
    pub fn bediene(&self, harness: UnixStream) {
        let _ = harness.set_read_timeout(Some(HARNESS_FRIST));
        let Some(anmeldung) = self.betrieb.anmelden(&harness) else {
            return;
        };
        let mut huelle = anmeldung.abbruch.huelle(harness);
        let mut pfad = None;
        if let Some(ausgang) = self.verarbeite(&mut huelle, &anmeldung.abbruch, &mut pfad) {
            self.zeile(ausgang, pfad);
        }
        let _ = huelle.shutdown(std::net::Shutdown::Both);
        drop(anmeldung);
    }

    /// The stages 1 to 10 and the way to the target. `None` only for `mw.protokoll_fehlt` (no line).
    /// `pfad` takes the limited path of the request as soon as the head is read, for the line (section 8).
    fn verarbeite(
        &self,
        h: &mut Huelle,
        abbruch: &Abbruch,
        pfad: &mut Option<String>,
    ) -> Option<Ausgang> {
        if self.betrieb.protokoll_kaputt.load(Ordering::SeqCst) {
            let _ = http::lies_anfragekopf(h);
            let _ = h.write_all(&Code::ProtokollFehlt.antwort());
            return None;
        }
        let Some((kopf, rest)) = http::lies_anfragekopf(h) else {
            return Some(ablehnen(h, Code::AnfrageUnlesbar));
        };
        *pfad = Some(pfad_feld(&kopf.ziel, self.lage.pfad_voll));
        let Ok(laenge) = kopf.inhaltslaenge() else {
            return Some(ablehnen(h, Code::AnfrageUnlesbar));
        };
        let Some(_platz) = self.platz() else {
            return Some(ablehnen(h, Code::Ausgelastet));
        };
        if let Err(code) = self.regeln.pruefe_kopf(&kopf) {
            return Some(ablehnen(h, code));
        }
        if kopf.werte("transfer-encoding").next().is_some() {
            return Some(ablehnen(h, Code::ChunkedNichtUnterstuetzt));
        }
        let Some(laenge) = laenge else {
            return Some(ablehnen(h, Code::LaengeFehlt));
        };
        if laenge > KOERPER_MAX {
            return Some(ablehnen(h, Code::KoerperZuGross));
        }
        let Some(koerper) = lies_koerper(h, rest, laenge) else {
            return Some(ablehnen(h, Code::AnfrageUnlesbar));
        };
        if let Err(code) = self.regeln.pruefe_koerper(&koerper) {
            return Some(ablehnen(h, code));
        }
        if let Some(z) = &self.lage.probelauf
            && z.zaehle(&koerper).is_err()
        {
            return Some(ablehnen(h, Code::ZaehlungFehler));
        }
        let anfrage = self
            .regeln
            .an_ziel(&kopf, &koerper, &self.ziel, &self.lage.schluessel);
        Some(self.weiterreichen(h, anfrage, abbruch))
    }

    /// Stage 3: one of the 8 places, or `None`.
    fn platz(&self) -> Option<Platz<'_>> {
        self.betrieb.platz()
    }

    /// Sends the request to the target and hands the answer back through the leak lock.
    fn weiterreichen(&self, h: &mut Huelle, mut anfrage: Vec<u8>, abbruch: &Abbruch) -> Ausgang {
        let Some(verbindung) = self.verbinder.verbinde() else {
            anfrage.fill(0);
            return ablehnen(h, Code::ZielNichtErreichbar);
        };
        abbruch.setze_ziel(&self.betrieb, verbindung.abreissen);
        let mut strom = verbindung.strom;
        let gesendet = strom.write_all(&anfrage).and_then(|()| strom.flush());
        anfrage.fill(0);
        if gesendet.is_err() {
            return ablehnen(h, Code::ZielNichtErreichbar);
        }
        let (kopf, rest) = match http::lies_antwortkopf(&mut strom) {
            Ok(k) => k,
            Err(Lesefehler::Frist) => return ablehnen(h, Code::ZielFrist),
            Err(Lesefehler::Sonst) => return ablehnen(h, Code::ZielNichtErreichbar),
        };
        let Some(rahmung) = kopf.rahmung() else {
            return ablehnen(h, Code::ZielNichtErreichbar);
        };
        let schluessel = self.lage.schluessel.bytes();
        let inhaltstyp = kopf.inhaltstyp();
        if inhaltstyp.is_some_and(|t| enthaelt(t, schluessel)) {
            return ablehnen(h, Code::LeckGesperrt);
        }
        let antwortkopf = harness_kopf(kopf.status, inhaltstyp);
        let mut leser = KoerperLeser::neu(strom, rest, rahmung);
        let mut sperre = Sperre::neu(&[schluessel]);
        let mut zaehler = self.regeln.zaehler();
        let mut kopf_gesendet = false;
        let weiter = |tokens| Ausgang {
            ergebnis: "weitergereicht",
            grund: None,
            status: kopf.status,
            tokens,
        };
        loop {
            match leser.teil() {
                Ok(Some(teil)) => {
                    zaehler.teil(&teil);
                    let Some(frei) = sperre.teil(&teil) else {
                        if kopf_gesendet {
                            // The connection is torn down by `bediene` right after the line.
                            return Ausgang {
                                ergebnis: "leck_gesperrt",
                                grund: Some(Code::LeckGesperrt),
                                status: kopf.status,
                                tokens: None,
                            };
                        }
                        return ablehnen(h, Code::LeckGesperrt);
                    };
                    if frei.is_empty() {
                        continue;
                    }
                    if !kopf_gesendet {
                        kopf_gesendet = true;
                        if h.write_all(&antwortkopf).is_err() {
                            return weiter(zaehler.ende());
                        }
                    }
                    if h.write_all(&frei).is_err() {
                        return weiter(zaehler.ende());
                    }
                }
                Ok(None) => {
                    let rest = sperre.ende();
                    if !kopf_gesendet {
                        let _ = h.write_all(&antwortkopf);
                    }
                    let _ = h.write_all(&rest);
                    return weiter(zaehler.ende());
                }
                Err(_) => {
                    if !kopf_gesendet {
                        let _ = h.write_all(&antwortkopf);
                    }
                    return weiter(zaehler.ende());
                }
            }
        }
    }

    /// Appends the line of a request; a failure blocks every further request (`mw.protokoll_fehlt`).
    /// The path stands in the line only at a rejection (section 8); a `GET` never gets past stage 4 or 5 here.
    fn zeile(&self, a: Ausgang, pfad: Option<String>) {
        // Only `weitergereicht` carries tokens; every other result is built without them.
        let (token_ein, token_aus) = a
            .tokens
            .map_or((None, None), |(ein, aus)| (Some(ein), Some(aus)));
        let m = Modellweg {
            zeit_ms: (self.betrieb.uhr)(),
            betriebsart: self.betriebsart.name().to_owned(),
            sitzung: self.lage.sitzung.clone(),
            route: self.lage.route.clone(),
            modell: self.modell.clone(),
            ergebnis: a.ergebnis.to_owned(),
            grund: a.grund.map(|c| c.code().to_owned()),
            status: u32::from(a.status),
            token_ein,
            token_aus,
            pfad: pfad.filter(|_| a.ergebnis == "abgelehnt"),
            host: None,
        };
        if protokoll::anhaengen_modellweg(&self.lage.protokoll, &m).is_err() {
            self.betrieb.protokoll_kaputt.store(true, Ordering::SeqCst);
        }
    }
}

/// Sends an own answer and returns its result.
fn ablehnen(h: &mut Huelle, code: Code) -> Ausgang {
    let _ = h.write_all(&code.antwort());
    Ausgang::eigen(code)
}

/// The body of exactly `laenge` bytes: the bytes after the head and the rest from the stream. Too few: `None`.
/// Bytes beyond the length are not read.
fn lies_koerper<R: Read>(h: &mut R, mut koerper: Vec<u8>, laenge: u64) -> Option<Vec<u8>> {
    let n = usize::try_from(laenge).ok()?;
    if koerper.len() >= n {
        koerper.truncate(n);
        return Some(koerper);
    }
    let fehlt = u64::try_from(n - koerper.len()).ok()?;
    h.take(fehlt).read_to_end(&mut koerper).ok()?;
    (koerper.len() == n).then_some(koerper)
}

/// The head to the harness: status, `content-type` of the target if it passed, own framing.
fn harness_kopf(status: u16, inhaltstyp: Option<&[u8]>) -> Vec<u8> {
    let mut v = format!("HTTP/1.1 {status} \r\n").into_bytes();
    if let Some(t) = inhaltstyp {
        v.extend_from_slice(b"content-type: ");
        v.extend_from_slice(t);
        v.extend_from_slice(b"\r\n");
    }
    v.extend_from_slice(b"Connection: close\r\n\r\n");
    v
}

#[cfg(test)]
mod tests {
    use super::{Dienst, FAEDEN_MAX, Lage, PARALLEL_MAX, harness_kopf, lies_koerper};
    use crate::betriebsart::openai_lokal::Optionen;
    use crate::schluessel::Schluessel;
    use crate::ziel::{Verbinder, Verbindung, Ziel};
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::sync::mpsc;
    use std::time::Duration;

    struct Nie;

    impl Verbinder for Nie {
        fn verbinde(&self) -> Option<Verbindung> {
            None
        }
    }

    fn dienst() -> Arc<Dienst> {
        let lage = Lage {
            art: Optionen {
                ziel: Ziel::lies("http://127.0.0.1:9/v1").expect("target"),
                modell: "m".to_owned(),
                schluessel_datei: PathBuf::from("/x"),
            },
            sitzung: "s".to_owned(),
            route: "r".to_owned(),
            protokoll: PathBuf::from("/nicht/da"),
            schluessel: Schluessel::fuer_test(b"k"),
            probelauf: None,
            pfad_voll: false,
        };
        Arc::new(Dienst::neu(lage, Box::new(Nie), Box::new(|| 0)))
    }

    #[test]
    fn eight_places_and_no_ninth() {
        let d = dienst();
        let plaetze: Vec<_> = (0..PARALLEL_MAX)
            .map(|_| d.platz().expect("a place"))
            .collect();
        assert!(d.platz().is_none());
        drop(plaetze);
        assert!(d.platz().is_some());
    }

    #[test]
    fn sixty_four_threads_and_no_more() {
        let d = dienst();
        let (tx, rx) = mpsc::channel();
        let d2 = Arc::clone(&d);
        std::thread::spawn(move || {
            let alle: Vec<_> = (0..FAEDEN_MAX).map(|_| d2.faden()).collect();
            tx.send(alle).expect("send");
        });
        let alle = rx.recv_timeout(Duration::from_secs(2)).expect("64 places");
        let (tx, rx) = mpsc::channel();
        let d3 = Arc::clone(&d);
        std::thread::spawn(move || {
            let f = d3.faden();
            let _ = tx.send(());
            drop(f);
        });
        assert!(
            rx.recv_timeout(Duration::from_millis(200)).is_err(),
            "a 65th place"
        );
        assert!(!d.warte_leer(Duration::from_millis(20)));
        drop(alle);
        rx.recv_timeout(Duration::from_secs(2))
            .expect("a freed place");
        assert!(d.warte_leer(Duration::from_secs(2)));
    }

    #[test]
    fn the_body_has_exactly_its_length() {
        assert_eq!(
            lies_koerper(&mut &b"cdef"[..], b"ab".to_vec(), 4),
            Some(b"abcd".to_vec())
        );
        assert_eq!(
            lies_koerper(&mut &b""[..], b"abc".to_vec(), 2),
            Some(b"ab".to_vec())
        );
        assert_eq!(
            lies_koerper(&mut &b""[..], b"ab".to_vec(), 2),
            Some(b"ab".to_vec())
        );
        assert_eq!(lies_koerper(&mut &b"c"[..], b"ab".to_vec(), 4), None);
        assert_eq!(lies_koerper(&mut &b""[..], Vec::new(), 0), Some(Vec::new()));
        assert_eq!(
            lies_koerper(&mut &b"bcdEXTRA"[..], b"a".to_vec(), 4),
            Some(b"abcd".to_vec())
        );
    }

    #[test]
    fn the_head_to_the_harness() {
        assert_eq!(
            harness_kopf(302, None),
            b"HTTP/1.1 302 \r\nConnection: close\r\n\r\n".to_vec()
        );
        assert_eq!(
            harness_kopf(200, Some(b"a/b")),
            b"HTTP/1.1 200 \r\ncontent-type: a/b\r\nConnection: close\r\n\r\n".to_vec()
        );
    }
}
