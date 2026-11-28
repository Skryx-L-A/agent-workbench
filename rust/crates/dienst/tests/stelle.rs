//! The channel to the decision point in the run (addendum "Kanal zur Stelle" of the contract): a stand-in of the point on
//! `stelle-mensch` with the check of the other side, the service with `lauf.stelle`. `mcp.offene` once a second while a viewer
//! is connected; the open request reaches the core. A `Freigabe` over `dienst.sock` never reaches the point
//! (`freigabe_nur_lokal`, in both modes), a field that would name who decides is the error `feld_verboten`. On the local way
//! (DI-L11) the service takes the measured account, builds the proof `h1` and asks `mcp.entscheiden`; the decision of the point
//! reaches the core.
#![cfg(unix)] // the crate is Unix only (see `src/lib.rs`)
#![cfg(feature = "pruefbau")] // the fixed child programs `--kind` exist in the test build only

mod gemeinsam;

use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use ::stelle::Nachweispruefer;
use gemeinsam::{DIENST, Probe, Prozess, Wegwerf, startdatei};
use peerkanal::{Annahme, Erlaubnis, Kennung, Server, ServerVorgabe, SocketModus, UnixOrt};
use serde_json::{Value, json};

/// Every frame the stand-in got, with the time it came.
type Gesehen = Arc<Mutex<Vec<(Instant, Value)>>>;

const MF: &str = "mf-1795179540000-1";
/// A second open request of `s-1` (the local way refuses it with a reason).
const MF2: &str = "mf-1795179540000-3";
/// The pseudonym that `lauf.stelle.nachweis` gives the own account.
const PS: &str = "ps_0123456789abcdef0123456789abcdef";
/// The key of the proof in the file `lauf.stelle.nachweis.schluessel`.
const SCHLUESSEL: [u8; 32] = [5; 32];

/// The program of the started sessions: a script that runs the fixed child `lauf` of `wb-dienst` with its two arguments.
fn programm(w: &Wegwerf) -> String {
    let pfad = w.0.join("laufkind.sh");
    std::fs::write(
        &pfad,
        format!("#!/bin/sh\nexec {DIENST} --kind lauf \"$0\" \"$@\"\n"),
    )
    .expect("script");
    std::fs::set_permissions(&pfad, std::fs::Permissions::from_mode(0o700)).expect("mode");
    pfad.to_string_lossy().into_owned()
}

/// A start file with `lauf` (the demonstration settings of contract 2) and no first session.
fn lauf_start(w: &Wegwerf, stelle: &Value) -> Value {
    let einst = w.0.join("einst.json");
    std::fs::write(
        &einst,
        include_str!("werkzeug/einstellungen/vorfuehrung.json"),
    )
    .expect("settings");
    let mut start = startdatei(w, &[DIENST], &json!({}));
    let o = start.as_object_mut().expect("object");
    o.remove("kind");
    o.remove("groesse");
    start["anfang"]["sitzungen"] = json!([]);
    start["lauf"] = json!({
        "programm": programm(w),
        "env": {"KIND_PLAN": json!({"*": {"modus": "marke", "sek": 40, "text": "EINS"}}).to_string(), "LC_ALL": "C.UTF-8"},
        "groesse": [40, 10],
        "einstellungen": einst.to_string_lossy(),
        "stelle": stelle,
    });
    start
}

fn wanduhr_ms() -> i64 {
    let ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_millis();
    i64::try_from(ms).expect("ms")
}

/// The stand-in of the point: `mcp.offene` names two open requests of `s-1` with ten minutes left; `mcp.entscheiden` is answered
/// with `erteilt` for `erteilen` and `abgelehnt` for `ablehnen`. Records every frame.
fn attrappe(ordner: &Path) -> Gesehen {
    let uid = std::fs::metadata(ordner).expect("folder").uid();
    let mut v = ServerVorgabe::neu(
        Kennung::neu("stelle-mensch").expect("name"),
        vec![Erlaubnis::Unix { uid, gid: None }],
        4,
    );
    v.unix = Some(UnixOrt {
        ordner: ordner.to_path_buf(),
        modus: SocketModus::M0600,
    });
    let server = Server::anlegen(v).expect("server");
    let gesehen = Arc::new(Mutex::new(Vec::new()));
    let g = Arc::clone(&gesehen);
    // the thread ends with the test program
    std::thread::spawn(move || {
        loop {
            let Ok(Annahme::Vorlaeufig(vl)) = server.annehmen() else {
                std::thread::sleep(Duration::from_millis(10));
                continue;
            };
            let Ok(v) = vl.bestaetigen() else { continue };
            let mut leser = BufReader::new(v);
            let mut zeile = Vec::new();
            if leser.read_until(b'\n', &mut zeile).unwrap_or(0) == 0 {
                continue;
            }
            let rahmen: Value = serde_json::from_slice(&zeile).expect("frame");
            g.lock()
                .expect("lock")
                .push((Instant::now(), rahmen.clone()));
            let offen = |anfrage: &str| {
                json!({"anfrage": anfrage, "server": "tickets", "sitzung": "s-1", "werkzeug": "update", "vorschau": "v",
                       "vorschau_sha256": null, "digest": "d0".repeat(32), "frist_bis_ms": wanduhr_ms() + 600_000})
            };
            let eingabe = &rahmen["eingabe"];
            let antwort = match rahmen["vorgang"].as_str() {
                Some("mcp.offene") => {
                    json!({"ok": true, "ergebnis": "offene", "offene": [offen(MF), offen(MF2)]})
                }
                Some("mcp.entscheiden") if eingabe["entscheidung"] == "ablehnen" => {
                    json!({"ok": true, "ergebnis": "abgelehnt", "anfrage": eingabe["anfrage"]})
                }
                Some("mcp.entscheiden") => {
                    json!({"ok": true, "ergebnis": "erteilt", "anfrage": eingabe["anfrage"], "einloesen_bis_ms": 7})
                }
                _ => continue,
            };
            let _ = leser.get_mut().write_all(format!("{antwort}\n").as_bytes());
        }
    });
    gesehen
}

fn vorgaenge(g: &Gesehen, vorgang: &str) -> Vec<(Instant, Value)> {
    g.lock()
        .expect("lock")
        .iter()
        .filter(|(_, r)| r["vorgang"] == vorgang)
        .cloned()
        .collect()
}

/// Sends `e` and waits for one more refusal; returns its reason.
fn abgelehnt(p: &mut Probe, e: &Value) -> String {
    let n = p.ablehnungen.len();
    p.ereignis(e);
    assert!(
        p.warte(Duration::from_secs(10), |p| p.ablehnungen.len() > n),
        "no refusal for {e}"
    );
    p.ablehnungen[n].clone()
}

/// The service with `lauf.stelle`, the local way and the stand-in, one viewer of `dienst.sock`, the session `s-1` started; waits
/// until the open request of the point has come to the core of `s-1` and checks its time left (the deadline of the point on the
/// clock of the service). `eigenes_konto`: `lauf.stelle.nachweis.konten` names the own account, otherwise only another one.
fn aufbau(w: &Wegwerf, eigenes_konto: bool) -> (Prozess, Probe, Gesehen) {
    let ordner = w.0.join("st");
    std::fs::create_dir(&ordner).expect("folder");
    std::fs::set_permissions(&ordner, std::fs::Permissions::from_mode(0o700)).expect("mode");
    let gesehen = attrappe(&ordner);
    let uid = std::fs::metadata(&ordner).expect("folder").uid();
    // the proof is configured for the own account: the switch alone keeps a viewer of dienst.sock from releasing
    let schluessel = w.0.join("h1.key");
    std::fs::write(&schluessel, SCHLUESSEL).expect("key");
    std::fs::set_permissions(&schluessel, std::fs::Permissions::from_mode(0o600)).expect("mode");
    let konto = if eigenes_konto {
        format!("uid-{uid}")
    } else {
        format!("uid-{}", uid.wrapping_add(1))
    };
    let mut start = lauf_start(
        w,
        &json!({"ordner": ordner.to_string_lossy(), "erwartet": {"uid": uid},
                "nachweis": {"schluessel": schluessel.to_string_lossy(), "konten": {konto: PS}}}),
    );
    start["lokal"] = json!({"kanal": "fenster"});
    let prozess = Prozess::starten(w, &start);
    let mut p = Probe::neu(&w.socket(), false);
    let n = p.nachrichten.len();
    p.ereignis(
        &json!({"art": "Starten", "start": {"fassung": 1, "harness": "claude", "modell": "m",
        "route": "cloud-beispiel", "kunde": "k-beispiel", "arbeitsordner": "/tmp"}}),
    );
    assert!(
        p.warte(Duration::from_secs(10), |p| p.nachrichten[n..]
            .iter()
            .any(|m| m["art"] == "Gestartet" && m["sitzung"] == "s-1")),
        "s-1 starts: {:?}",
        p.ablehnungen
    );
    // the open request of the point comes to the core of s-1, with the time left of the point on the clock of the service
    assert!(
        p.warte(Duration::from_secs(10), |p| p
            .nachrichten
            .iter()
            .any(|m| m["art"] == "Anfrage" && m["id"] == MF)),
        "the request comes: {:?}",
        p.arten()
    );
    let anfrage = p
        .nachrichten
        .iter()
        .find(|m| m["art"] == "Anfrage" && m["id"] == MF)
        .cloned()
        .expect("request");
    let rest = anfrage["frist"].as_i64().expect("frist") - anfrage["t"].as_i64().expect("t");
    assert!((590_000..=600_000).contains(&rest), "{anfrage}");
    assert_eq!(anfrage["zustand"], "offen");
    (prozess, p, gesehen)
}

#[test]
fn die_freigabe_ueber_dienst_sock_erreicht_die_stelle_nie() {
    let w = Wegwerf::neu("stelle");
    let (_prozess, mut p, gesehen) = aufbau(&w, true);
    let freigabe = |zusatz: Value| {
        let mut e = json!({"art": "Freigabe", "anfragen": [MF]});
        for (k, v) in zusatz.as_object().expect("object") {
            e[k] = v.clone();
        }
        e
    };
    // the switch comes before every check of DI-L6 point 4: a good release, a bad form, a second request, an unknown request
    for zusatz in [
        json!({}),
        json!({"entscheidung": "ablehnen", "begruendung": "passt nicht"}),
        json!({"entscheidung": "vielleicht"}),
        json!({"anfragen": [MF, "mf-1795179540000-2"]}),
        json!({"anfragen": ["mf-1795179540000-2"]}),
    ] {
        assert_eq!(
            abgelehnt(&mut p, &freigabe(zusatz.clone())),
            "freigabe_nur_lokal",
            "{zusatz}"
        );
    }
    assert!(p.fehler.is_empty(), "{:?}", p.fehler);
    // `mcp.offene` once a second while the viewer is connected, not in every turn of the loop
    let ende = Instant::now() + Duration::from_secs(10);
    while vorgaenge(&gesehen, "mcp.offene").len() < 3 && Instant::now() < ende {
        p.lesen(Duration::from_millis(50));
    }
    let offene = vorgaenge(&gesehen, "mcp.offene");
    let dauer = offene
        .last()
        .expect("last")
        .0
        .duration_since(offene[0].0)
        .as_millis();
    let zahl = u128::try_from(offene.len()).expect("count");
    assert!(zahl >= 3, "{zahl}");
    assert!(zahl <= dauer / 1000 + 2, "{zahl} requests in {dauer} ms");
    assert!(p.fehler.is_empty(), "{:?}", p.fehler);
    // a proof of the view or a field that would name who decides: the error of DI3 before the refusals of the switch
    // (DI-E5 point 4), the connection ends, and again no request
    for feld in ["nachweis", "wer", "konto", "person", "uid", "sid"] {
        let mut q = Probe::neu(&w.socket(), false);
        q.ereignis(&freigabe(json!({feld: "h1:x"})));
        assert!(
            q.warte(Duration::from_secs(10), |q| !q.fehler.is_empty()),
            "{feld}"
        );
        assert_eq!(q.fehler, ["feld_verboten"], "{feld}");
        assert!(q.ablehnungen.is_empty(), "{feld}");
    }
    p.lesen(Duration::from_millis(200));
    assert_eq!(vorgaenge(&gesehen, "mcp.entscheiden").len(), 0);
}

#[test]
fn ohne_lauf_erreicht_die_freigabe_den_kern_nie() {
    let w = Wegwerf::neu("ohnelauf");
    let _prozess = Prozess::starten(&w, &startdatei(&w, &["/bin/sleep", "60"], &json!({})));
    let mut p = Probe::neu(&w.socket(), false);
    let e = json!({"art": "Freigabe", "anfragen": ["a-1"]});
    assert_eq!(abgelehnt(&mut p, &e), "freigabe_nur_lokal");
    for e in [
        json!({"art": "Freigabe", "anfragen": ["a-1"], "nachweis": "h1:x"}),
        json!({"art": "Freigabe", "anfragen": ["a-1"], "konto": "uid-0"}),
    ] {
        let mut q = Probe::neu(&w.socket(), false);
        q.ereignis(&e);
        assert!(
            q.warte(Duration::from_secs(10), |q| !q.fehler.is_empty()),
            "{e}"
        );
        assert_eq!(q.fehler, ["feld_verboten"], "{e}");
    }
}

/// A client of the local way (DI-L11): the hello of `peerkanal`, then the frames of the protocol.
struct Lokal {
    strom: UnixStream,
    puffer: Vec<u8>,
}

impl Lokal {
    /// Connects to `<ordner>/fenster.sock` and waits for the answer to the hello and for the first message (the list of events).
    fn verbinden(w: &Wegwerf) -> (Self, Value) {
        let mut strom = UnixStream::connect(w.ordner().join("fenster.sock")).expect("connect");
        strom
            .set_read_timeout(Some(Duration::from_secs(5)))
            .expect("timeout");
        strom.write_all(b"pk1\n").expect("hello");
        let mut antwort = [0_u8; 4];
        strom.read_exact(&mut antwort).expect("answer to the hello");
        assert_eq!(&antwort, b"pk1\n");
        let mut l = Self {
            strom,
            puffer: Vec::new(),
        };
        let erste = l.warte(|a, _| a == 0x06).expect("first message").1;
        (l, erste)
    }

    fn ereignis(&mut self, e: &Value) {
        let nutzlast = e.to_string().into_bytes();
        let n = u32::try_from(nutzlast.len() + 1).expect("size");
        let mut r = n.to_be_bytes().to_vec();
        r.push(0x01);
        r.extend_from_slice(&nutzlast);
        self.strom.write_all(&r).expect("write");
    }

    /// Reads frames until one passes `bis` (kind and JSON payload); `None` at the end of the connection or after 10 s.
    fn warte(&mut self, bis: impl Fn(u8, &Value) -> bool) -> Option<(u8, Value)> {
        let ende = Instant::now() + Duration::from_secs(10);
        let mut p = vec![0_u8; 65536];
        while Instant::now() < ende {
            while self.puffer.len() >= 5 {
                let n = u32::from_be_bytes(self.puffer[..4].try_into().expect("4 bytes")) as usize;
                if self.puffer.len() < 4 + n {
                    break;
                }
                let a = self.puffer[4];
                let nutz = self.puffer[5..4 + n].to_vec();
                self.puffer.drain(..4 + n);
                if let Ok(wert) = serde_json::from_slice::<Value>(&nutz)
                    && bis(a, &wert)
                {
                    return Some((a, wert));
                }
            }
            match self.strom.read(&mut p) {
                Ok(0) => return None,
                Ok(n) => self.puffer.extend_from_slice(&p[..n]),
                Err(e)
                    if e.kind() == std::io::ErrorKind::WouldBlock
                        || e.kind() == std::io::ErrorKind::TimedOut => {}
                Err(_) => return None,
            }
        }
        None
    }

    /// Sends `e` and waits for the refusal or the error; the reason and whether it was the error (`0x08`).
    fn antwort(&mut self, e: &Value) -> (String, bool) {
        self.ereignis(e);
        let (a, wert) = self
            .warte(|a, _| a == 0x07 || a == 0x08)
            .unwrap_or_else(|| panic!("no answer to {e}"));
        let grund = if a == 0x08 {
            &wert["fehler"]
        } else {
            &wert["grund"]
        };
        (grund.as_str().expect("reason").to_owned(), a == 0x08)
    }
}

/// A refusal with its reason on the local way: the reason stays with the service and reaches the core with the decision of the
/// point; the point gets exactly the request, the decision and the proof.
fn ablehnung_mit_grund(l: &mut Lokal, p: &mut Probe, gesehen: &Gesehen) {
    assert!(
        p.warte(Duration::from_secs(10), |p| p
            .nachrichten
            .iter()
            .any(|m| m["art"] == "Anfrage" && m["id"] == MF2)),
        "the second request comes"
    );
    let n = p.nachrichten.len();
    l.ereignis(&json!({"art": "Freigabe", "anfragen": [MF2], "entscheidung": "ablehnen", "begruendung": "zu breit"}));
    assert!(
        p.warte(Duration::from_secs(10), |p| p.nachrichten[n..].iter().any(
            |m| m["art"] == "Anfrage"
                && m["id"] == MF2
                && m["zustand"] == "abgelehnt"
                && m["begruendung"] == "zu breit"
        )),
        "the refusal reaches the core: {:?}",
        p.ablehnungen
    );
    let gefragt = vorgaenge(gesehen, "mcp.entscheiden");
    assert_eq!(gefragt.len(), 2);
    let eingabe = &gefragt[1].1["eingabe"];
    assert_eq!(eingabe["anfrage"], MF2);
    assert_eq!(eingabe["entscheidung"], "ablehnen");
    assert!(eingabe.get("begruendung").is_none(), "{eingabe}");
}

#[test]
fn die_freigabe_des_lokalen_wegs_traegt_den_nachweis_des_dienstes() {
    let w = Wegwerf::neu("stellelokal");
    let (_prozess, mut p, gesehen) = aufbau(&w, true);
    let (mut l, erste) = Lokal::verbinden(&w);
    assert_eq!(erste["art"], "Ereignisliste");
    assert!(
        erste["ereignisse"]
            .as_array()
            .expect("list")
            .contains(&json!("Freigabe")),
        "{erste}"
    );
    // the checks of DI-L6 point 4 come after the switch, on the local way with `lauf.stelle`
    let freigabe = |zusatz: Value| {
        let mut e = json!({"art": "Freigabe", "anfragen": [MF]});
        for (k, v) in zusatz.as_object().expect("object") {
            e[k] = v.clone();
        }
        e
    };
    for (zusatz, grund) in [
        (json!({"entscheidung": "vielleicht"}), "ereignis_ungueltig"),
        (
            json!({"begruendung": "passt"}),
            "begruendung_nur_bei_ablehnen",
        ),
        (
            json!({"anfragen": [MF, "mf-1795179540000-2"]}),
            "sammelfreigabe",
        ),
        (
            json!({"anfragen": ["mf-1795179540000-2"]}),
            "anfrage_unbekannt",
        ),
    ] {
        assert_eq!(
            l.antwort(&freigabe(zusatz.clone())),
            (grund.to_owned(), false),
            "{zusatz}"
        );
    }
    assert_eq!(vorgaenge(&gesehen, "mcp.entscheiden").len(), 0);
    // a good release: the proof of the service over the binding of the point
    let n = p.nachrichten.len();
    l.ereignis(&freigabe(json!({})));
    let ende = Instant::now() + Duration::from_secs(10);
    while vorgaenge(&gesehen, "mcp.entscheiden").is_empty() && Instant::now() < ende {
        p.lesen(Duration::from_millis(50));
    }
    let gefragt = vorgaenge(&gesehen, "mcp.entscheiden");
    assert_eq!(gefragt.len(), 1, "one question to the point");
    let eingabe = gefragt[0].1["eingabe"].clone();
    let schluessel: Vec<&String> = eingabe.as_object().expect("input").keys().collect();
    assert_eq!(
        schluessel,
        ["anfrage", "entscheidung", "nachweis"],
        "{eingabe}"
    );
    assert_eq!(eingabe["anfrage"], MF);
    assert_eq!(eingabe["entscheidung"], "erteilen");
    let nachweis = eingabe["nachweis"].as_str().expect("proof");
    let herausforderung = ::stelle::herausforderung(MF, &"d0".repeat(32), "erteilen", None);
    assert_eq!(
        ::stelle::PrueferH1::neu(SCHLUESSEL).pruefe(&herausforderung, nachweis),
        Ok(PS.to_owned())
    );
    // the stand-in answers `erteilt`: the core takes the decision of the point
    assert!(
        p.warte(Duration::from_secs(10), |p| p.nachrichten[n..].iter().any(
            |m| m["art"] == "Anfrage" && m["id"] == MF && m["zustand"] == "erteilt"
        )),
        "the decision reaches the core: {:?} {:?}",
        p.arten(),
        p.ablehnungen
    );
    // decided: a second release of the same request is no longer open, and no second question
    assert_eq!(
        l.antwort(&freigabe(json!({}))),
        ("nicht_offen".to_owned(), false)
    );
    assert_eq!(vorgaenge(&gesehen, "mcp.entscheiden").len(), 1);
    ablehnung_mit_grund(&mut l, &mut p, &gesehen);
    // a proof of the view is the error feld_verboten on the local way, the connection ends
    assert_eq!(
        l.antwort(&freigabe(json!({"nachweis": "h1:x"}))),
        ("feld_verboten".to_owned(), true)
    );
    assert!(l.warte(|_, _| false).is_none(), "the connection ends");
    assert_eq!(vorgaenge(&gesehen, "mcp.entscheiden").len(), 2);
}

#[test]
fn ein_konto_ohne_pseudonym_gibt_keinen_nachweis() {
    let w = Wegwerf::neu("stellekonto");
    let (_prozess, _p, gesehen) = aufbau(&w, false);
    let (mut l, _) = Lokal::verbinden(&w);
    assert_eq!(
        l.antwort(&json!({"art": "Freigabe", "anfragen": [MF]})),
        ("konto_ohne_person".to_owned(), false)
    );
    std::thread::sleep(Duration::from_millis(200));
    assert_eq!(vorgaenge(&gesehen, "mcp.entscheiden").len(), 0);
}
