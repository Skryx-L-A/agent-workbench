//! One connection through a real Unix socket under Linux, with one uid (contract 15.12.2): `SO_PEERCRED` of the own
//! process, one request, one answer with a line end, the connection closed. Two uids need a throwaway VM (AL-20).

#![cfg(target_os = "linux")]

mod gemeinsam;

use std::io::{Read, Write};
use std::net::Shutdown;
use std::os::unix::net::{UnixListener, UnixStream};
use std::time::{Duration, Instant};

use gemeinsam::{Gestoert, HELFER_UID, Ort, modus};
use serde_json::{Value, json};
use vorgabe_helfer::bedienen::bedienen;
use vorgabe_helfer::helfer::Orte;
use vorgabe_helfer::json::bytes_der;
use vorgabe_helfer::konten::Tabelle;
use vorgabe_helfer::partner;

const NAME: &str = "firmenvorgabe.json";

fn selbst_eingeschlossen() -> bool {
    let c = std::fs::read_to_string("/proc/self/cgroup").ok();
    let s = std::fs::read_to_string("/proc/self/status").ok();
    partner::eingeschlossen(c.as_deref(), s.as_deref())
}

struct Lage {
    _ort: Ort,
    orte: Orte,
    d: Gestoert,
    hoerer: UnixListener,
    sock: std::path::PathBuf,
}

fn lage(name: &str, admins: &[&str]) -> Lage {
    let ort = Ort::neu(&format!("sock-{name}"));
    let e = ort.0.join("e");
    let v = e.join("vorgabe");
    std::fs::create_dir_all(&v).expect("folders");
    let inhalt = json!({"admins": admins, "leseklassen": []});
    std::fs::write(v.join(NAME), bytes_der(&inhalt)).expect("file");
    modus(&v.join(NAME), 0o644);
    modus(&v, 0o755);
    modus(&e, 0o755);
    let mut d = Gestoert::neu(&[]);
    d.eigner_von(&e, 0);
    let sock = ort.0.join("s.sock");
    let hoerer = UnixListener::bind(&sock).expect("bind");
    Lage {
        orte: Orte {
            ordner: v,
            name: NAME.into(),
            eltern: vec![e],
            helfer_uid: HELFER_UID,
        },
        d,
        hoerer,
        sock,
        _ort: ort,
    }
}

fn konten() -> Tabelle {
    Tabelle(vec![
        ("konto-ich".into(), rustix::process::getuid().as_raw()),
        ("konto-anderes".into(), 4_000_000),
    ])
}

/// Sends `anfrage` (then shuts the write side unless `offen`), returns the answer bytes.
fn senden(
    sock: &std::path::Path,
    anfrage: Vec<u8>,
    offen: bool,
) -> std::thread::JoinHandle<Vec<u8>> {
    let sock = sock.to_path_buf();
    std::thread::spawn(move || {
        let mut c = UnixStream::connect(&sock).expect("connect");
        let _ = c.write_all(&anfrage);
        if !offen {
            let _ = c.shutdown(Shutdown::Write);
        }
        let mut aus = Vec::new();
        let _ = c.read_to_end(&mut aus);
        aus
    })
}

fn antwort(b: &[u8]) -> Value {
    assert!(b.ends_with(b"\n"), "one line end");
    assert_eq!(b.split(|x| *x == b'\n').count() - 1, 1);
    serde_json::from_slice(b).expect("json")
}

#[test]
fn admin_ersetzt_ueber_den_socket() {
    let l = lage("admin", &["konto-ich"]);
    let alt = std::fs::read(l.orte.ordner.join(NAME)).expect("file");
    let neu = json!({"admins": ["konto-ich"], "leseklassen": [{"ebene": "gruppe", "wer": "g", "klasse": "intern"}]});
    let a = json!({"art": "Ersetzen", "fassung": 1, "basis": formate::kette::sha256_hex(&alt), "inhalt": neu});
    let t = senden(&l.sock, serde_json::to_vec(&a).expect("json"), false);
    let (strom, _) = l.hoerer.accept().expect("accept");
    let erg = bedienen(strom, &l.orte, &l.d, &konten());
    let b = t.join().expect("client");
    if selbst_eingeschlossen() {
        assert_eq!(
            antwort(&b),
            json!({"code": "firmenvorgabe.partner_eingeschlossen", "sha256": null})
        );
        assert_eq!(
            erg.zeile(),
            "werkbank-vorgabe: abgelehnt: firmenvorgabe.partner_eingeschlossen"
        );
        return;
    }
    let sha = formate::kette::sha256_hex(&bytes_der(&neu));
    assert_eq!(antwort(&b), json!({"code": null, "sha256": sha}));
    assert_eq!(erg.zeile(), "werkbank-vorgabe: ersetzt");
    assert_eq!(
        std::fs::read(l.orte.ordner.join(NAME)).expect("file"),
        bytes_der(&neu)
    );
}

/// Sends `anfrage` and returns the answer bytes and the time from `connect` to the end of the answer.
fn senden_mit_zeit(
    sock: &std::path::Path,
    anfrage: Vec<u8>,
    offen: bool,
) -> std::thread::JoinHandle<(Vec<u8>, Duration)> {
    let sock = sock.to_path_buf();
    std::thread::spawn(move || {
        let start = Instant::now();
        let mut c = UnixStream::connect(&sock).expect("connect");
        c.write_all(&anfrage).expect("write");
        if !offen {
            c.shutdown(Shutdown::Write).expect("shutdown");
        }
        let mut aus = Vec::new();
        c.read_to_end(&mut aus)
            .expect("the answer arrives without a reset");
        (aus, start.elapsed())
    })
}

fn soll_nichtadmin() -> &'static str {
    if selbst_eingeschlossen() {
        "firmenvorgabe.partner_eingeschlossen"
    } else {
        "firmenvorgabe.partner_nicht_admin"
    }
}

#[test]
fn nichtadmin_bekommt_den_code_ohne_dass_gelesen_wird() {
    let l = lage("nichtadmin", &["konto-anderes"]);
    // the client never shuts its write side: an answer before the deadline shows that nothing was read
    let t = senden_mit_zeit(&l.sock, b"{".to_vec(), true);
    let (strom, _) = l.hoerer.accept().expect("accept");
    let erg = bedienen(strom, &l.orte, &l.d, &konten());
    let (b, dauer) = t.join().expect("client");
    assert!(dauer < Duration::from_secs(1), "{dauer:?}");
    assert_eq!(
        antwort(&b),
        json!({"code": soll_nichtadmin(), "sha256": null})
    );
    assert_eq!(
        erg.code.map(vorgabe_helfer::code::Code::text),
        Some(soll_nichtadmin())
    );
}

/// A refusal before the first byte of a whole request that the client already sent: closing with unread bytes would reset
/// the connection and the answer would be lost (measured in the throwaway VM, 27.11.2026). The answer must arrive.
#[test]
fn ablehnung_vor_dem_lesen_kommt_trotz_gesendeter_anfrage_an() {
    for groesse in [100usize, 300_000] {
        let l = lage(&format!("ungelesen-{groesse}"), &["konto-anderes"]);
        let mut roh = serde_json::to_vec(&json!({"art": "Ersetzen", "fassung": 1, "basis": "0".repeat(64),
                                                   "inhalt": {"admins": ["konto-anderes"], "leseklassen": []}}))
            .expect("json");
        roh.resize(groesse.max(roh.len()), b' ');
        let t = senden_mit_zeit(&l.sock, roh, false);
        let (strom, _) = l.hoerer.accept().expect("accept");
        let erg = bedienen(strom, &l.orte, &l.d, &konten());
        let (b, _) = t.join().expect("client");
        assert_eq!(
            antwort(&b),
            json!({"code": soll_nichtadmin(), "sha256": null}),
            "{groesse}"
        );
        assert_eq!(
            erg.code.map(vorgabe_helfer::code::Code::text),
            Some(soll_nichtadmin())
        );
    }
}

#[test]
fn frist_der_anfrage() {
    if selbst_eingeschlossen() {
        return;
    }
    let l = lage("frist", &["konto-ich"]);
    let t = senden(&l.sock, b"{".to_vec(), true);
    let (strom, _) = l.hoerer.accept().expect("accept");
    let start = Instant::now();
    let erg = bedienen(strom, &l.orte, &l.d, &konten());
    let dauer = start.elapsed();
    assert!(
        dauer >= Duration::from_millis(1900) && dauer < Duration::from_secs(4),
        "{dauer:?}"
    );
    assert_eq!(
        antwort(&t.join().expect("client")),
        json!({"code": "firmenvorgabe.anfrage_frist", "sha256": null})
    );
    assert_eq!(
        erg.zeile(),
        "werkbank-vorgabe: abgelehnt: firmenvorgabe.anfrage_frist"
    );
}
