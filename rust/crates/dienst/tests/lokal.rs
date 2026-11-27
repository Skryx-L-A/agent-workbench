//! The local way as a program, in both builds (contract `dienst`, DI-L11): the delivered build has no fixed child programs, so the
//! child here is `/bin/sleep`. The service creates `<ordner>/fenster.sock` with mode 0600 next to `dienst.sock`; a client with the
//! hello of `peerkanal` is a viewer whose `wer` (only in the answer to `Zustand`) is the account of this test process, measured by
//! the kernel; a viewer of `dienst.sock` has no `wer`. `konto` and `person` in an event are `feld_verboten` on both ways, a wrong
//! hello gets no answer and one line on stderr, and the end removes both sockets.
#![cfg(unix)] // the crate is Unix only (see `src/lib.rs`)

mod gemeinsam;

use std::io::{Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};

use gemeinsam::{Probe, Prozess, Wegwerf, startdatei};
use serde_json::{Value, json};

/// The output of `id <arg>` (the account of this process as the system says it, not as the service measures it).
fn id(arg: &str) -> String {
    let aus = Command::new("id").arg(arg).output().expect("id");
    String::from_utf8(aus.stdout)
        .expect("UTF-8")
        .trim()
        .to_owned()
}

/// A client of the local way: the hello, then the frames of the protocol.
struct Lokal {
    strom: UnixStream,
    puffer: Vec<u8>,
}

impl Lokal {
    /// Connects and sends `hallo`; returns the client and the answer of the server (empty: no answer, the connection ended).
    fn verbinden(pfad: &Path, hallo: &[u8]) -> (Self, Vec<u8>) {
        let mut strom = UnixStream::connect(pfad).expect("connect");
        strom
            .set_read_timeout(Some(Duration::from_secs(5)))
            .expect("timeout");
        strom.write_all(hallo).expect("hello");
        let mut antwort = Vec::new();
        let mut p = [0_u8; 4];
        while antwort.len() < 4 {
            match strom.read(&mut p[..4 - antwort.len()]) {
                Ok(0) | Err(_) => break,
                Ok(n) => antwort.extend_from_slice(&p[..n]),
            }
        }
        (
            Self {
                strom,
                puffer: Vec::new(),
            },
            antwort,
        )
    }

    fn rahmen(&mut self, art: u8, nutzlast: &[u8]) {
        let n = u32::try_from(nutzlast.len() + 1).expect("size");
        let mut r = n.to_be_bytes().to_vec();
        r.push(art);
        r.extend_from_slice(nutzlast);
        self.strom.write_all(&r).expect("write");
    }

    /// Reads frames until one of the kind `art` comes; its JSON payload, or `None` at the end of the connection or after 5 s.
    fn warte(&mut self, art: u8) -> Option<Value> {
        self.warte_auf(art, |_| true)
    }

    /// Reads frames until one of the kind `art` comes whose payload passes `bis`.
    fn warte_auf(&mut self, art: u8, bis: impl Fn(&Value) -> bool) -> Option<Value> {
        let ende = Instant::now() + Duration::from_secs(5);
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
                if a == art
                    && let Ok(wert) = serde_json::from_slice::<Value>(&nutz)
                    && bis(&wert)
                {
                    return Some(wert);
                }
            }
            match self.strom.read(&mut p) {
                Ok(0) | Err(_) => return None,
                Ok(n) => self.puffer.extend_from_slice(&p[..n]),
            }
        }
        None
    }

    /// `true` when the server closed the connection.
    fn ende(&mut self) -> bool {
        let mut p = [0_u8; 4096];
        let ende = Instant::now() + Duration::from_secs(5);
        while Instant::now() < ende {
            match self.strom.read(&mut p) {
                Ok(0) => return true,
                Ok(_) => {}
                Err(_) => return false,
            }
        }
        false
    }
}

#[test]
fn der_lokale_weg_mit_konto_aus_dem_kernel() {
    let w = Wegwerf::neu("lokal");
    let start = startdatei(
        &w,
        &["/bin/sleep", "30"],
        &json!({"lokal": {"kanal": "fenster"}}),
    );
    let mut p = Prozess::starten(&w, &start);
    let pfad = w.ordner().join("fenster.sock");
    let modus = std::fs::symlink_metadata(&pfad)
        .expect("local socket")
        .permissions()
        .mode();
    assert_eq!(modus & 0o7777, 0o600, "mode of the local socket");

    // a wrong hello: no answer, no viewer
    let (_falsch, antwort) = Lokal::verbinden(&pfad, b"pk2\n");
    assert_eq!(antwort, Vec::<u8>::new());

    // the hello of peerkanal: a viewer `b1` with the measured account
    let (mut c1, antwort) = Lokal::verbinden(&pfad, b"pk1\n");
    assert_eq!(antwort, b"pk1\n".to_vec());
    // the thread of the local way hands the connection to the loop through the wake socket: only the first message says that the
    // loop took it (and named it `b1`); a connection of `dienst.sock` before that would be `b1`
    assert!(
        c1.warte(0x06).is_some(),
        "first message to the local viewer"
    );
    let mut c2 = Probe::neu(&w.socket(), false);
    c1.rahmen(0x12, b"");
    let zustand = c1.warte(0x12).expect("answer to Zustand");
    let system = if cfg!(target_os = "macos") {
        "macos"
    } else {
        "linux"
    };
    let uid: u32 = id("-u").parse().expect("uid");
    assert_eq!(
        zustand["dienst"]["betrachter"]["b1"]["wer"],
        json!({"system": system, "uid": uid, "sid": null, "name": id("-un")})
    );
    assert!(
        zustand["dienst"]["betrachter"]["b2"].get("wer").is_none(),
        "a viewer of dienst.sock has no wer: {zustand}"
    );
    let von_c2 = c2.zustand();
    assert_eq!(
        von_c2["dienst"]["betrachter"]["b1"]["wer"]["uid"],
        json!(uid)
    );
    assert!(von_c2["dienst"]["betrachter"]["b2"].get("wer").is_none());

    // an account in an event is feld_verboten on both ways, and the connection ends
    c2.ereignis(&json!({"art": "Text", "sitzung": "s-alpha", "text": "x", "person": "anna"}));
    assert!(c2.warte(Duration::from_secs(5), |p| !p.fehler.is_empty()));
    assert_eq!(c2.fehler, vec!["feld_verboten".to_owned()]);
    c1.rahmen(
        0x01,
        json!({"art": "Text", "sitzung": "s-alpha", "text": "x", "konto": "admin"})
            .to_string()
            .as_bytes(),
    );
    let fehler = c1.warte(0x08).expect("error frame");
    assert_eq!(fehler["fehler"], json!("feld_verboten"));
    assert!(c1.ende(), "the connection ends after the error");

    p.signal("TERM");
    assert_eq!(p.warte_ende(Duration::from_secs(15)), Some(0));
    assert!(!pfad.exists(), "the local socket is removed");
    assert!(!w.socket().exists(), "the socket of the bridge is removed");
    let zeilen = p.stderr_zeilen();
    assert_eq!(
        zeilen
            .iter()
            .filter(|z| z.starts_with("wb-dienst: lokal abgewiesen: "))
            .collect::<Vec<_>>(),
        vec!["wb-dienst: lokal abgewiesen: pk.hallo_falsch"],
        "{zeilen:?}"
    );
    let konto = id("-un");
    assert!(
        zeilen.iter().all(|z| !z.contains(&konto)),
        "no line names the account: {zeilen:?}"
    );
}

/// A taken path of the local way: exit code 3, the entry stays, no socket of the bridge is left.
#[test]
fn ein_belegter_lokaler_pfad_ist_exitcode_3() {
    let w = Wegwerf::neu("belegt");
    let pfad = w.ordner().join("fenster.sock");
    std::fs::write(&pfad, b"").expect("entry");
    let start = startdatei(
        &w,
        &["/bin/sleep", "30"],
        &json!({"lokal": {"kanal": "fenster"}}),
    );
    let datei = gemeinsam::datei_schreiben(&w, &start);
    let aus = Command::new(gemeinsam::DIENST)
        .arg(&datei)
        .output()
        .expect("runs");
    assert_eq!(aus.status.code(), Some(3));
    let fehler = String::from_utf8_lossy(&aus.stderr);
    assert_eq!(fehler.lines().count(), 1, "{fehler}");
    assert!(
        fehler.starts_with("wb-dienst: lokal: pk.name_belegt"),
        "{fehler}"
    );
    assert!(pfad.exists(), "the entry stays");
    assert!(!w.socket().exists(), "no socket of the bridge");
}

/// The rebuild of the core (`Kernfehler`, test mode) sends `Verbinden` again for every viewer; the viewer of the local way keeps its `wer`.
#[test]
fn neubau_des_kerns_behaelt_den_lokalen_betrachter() {
    let w = Wegwerf::neu("neubau");
    let start = startdatei(
        &w,
        &["/bin/sleep", "30"],
        &json!({"lokal": {"kanal": "fenster"}}),
    );
    let _p = Prozess::starten(&w, &start);
    let (mut c1, antwort) = Lokal::verbinden(&w.ordner().join("fenster.sock"), b"pk1\n");
    assert_eq!(antwort, b"pk1\n".to_vec());
    c1.rahmen(0x12, b"");
    let vorher = c1.warte(0x12).expect("answer to Zustand");
    let bau = vorher["dienst"]["kernbau"].as_u64().expect("kernbau");
    c1.rahmen(0x13, b"");
    c1.rahmen(0x12, b"");
    let zustand = c1.warte(0x12).expect("answer to Zustand");
    assert_eq!(zustand["dienst"]["kernbau"], json!(bau + 1), "{zustand}");
    assert_eq!(
        zustand["kern"]["betrachter"]["b1"]["person"],
        json!("anna"),
        "{zustand}"
    );
    assert_eq!(
        zustand["dienst"]["betrachter"]["b1"]["wer"]["name"],
        json!(id("-un"))
    );
}

/// `Starten` over the local way (DI-L11 point 8) in the delivered build: `Gestartet` to the sender after the list, a refusal with `code`.
#[test]
fn starten_ueber_den_lokalen_weg() {
    let w = Wegwerf::neu("starten");
    let programm = w.0.join("lauf.sh");
    std::fs::write(&programm, "#!/bin/sh\nexec /bin/sleep 30\n").expect("script");
    std::fs::set_permissions(&programm, std::fs::Permissions::from_mode(0o700)).expect("mode");
    let einst = w.0.join("einst.json");
    std::fs::write(
        &einst,
        include_str!("werkzeug/einstellungen/vorfuehrung.json"),
    )
    .expect("settings");
    let mut start = startdatei(
        &w,
        &["/bin/sleep", "30"],
        &json!({"lokal": {"kanal": "fenster"}}),
    );
    let o = start.as_object_mut().expect("object");
    o.remove("kind");
    o.remove("groesse");
    start["anfang"]["sitzungen"] = json!([]);
    start["lauf"] = json!({
        "programm": programm.to_string_lossy(),
        "env": {"LC_ALL": "C.UTF-8"},
        "groesse": [40, 10],
        "einstellungen": einst.to_string_lossy(),
    });
    let _p = Prozess::starten(&w, &start);
    let (mut c1, antwort) = Lokal::verbinden(&w.ordner().join("fenster.sock"), b"pk1\n");
    assert_eq!(antwort, b"pk1\n".to_vec());
    let gut = json!({"fassung": 1, "harness": "claude", "modell": "m", "route": "cloud-beispiel", "kunde": "k-beispiel",
                     "arbeitsordner": "/tmp"});
    c1.rahmen(
        0x01,
        json!({"art": "Starten", "start": gut})
            .to_string()
            .as_bytes(),
    );
    let gestartet = c1
        .warte_auf(0x06, |m| m["art"] == "Gestartet")
        .expect("Gestartet");
    assert_eq!(gestartet["sitzung"], json!("s-1"));
    let mut falsch = gut.clone();
    falsch["harness"] = json!("unbekannt-x");
    c1.rahmen(
        0x01,
        json!({"art": "Starten", "start": falsch})
            .to_string()
            .as_bytes(),
    );
    let ablehnung = c1.warte(0x07).expect("refusal");
    assert!(ablehnung.get("code").is_some(), "{ablehnung}");
}
