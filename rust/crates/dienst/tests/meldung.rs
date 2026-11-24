//! Tests of the report of `lauf` (contract `dienst`, section 18; Linux only): the program `wb-dienst` makes the socket `meldung.sock` in the
//! folder of every session of `Starten`, starts `lauf` with `--meldung <pfad>` before the start file, accepts one connection of the child
//! and reads one line `<code> <grund>\n` from it.
//!
//! The program of a session is a Python script (`lauf.programm`) that plays `lauf`. What it does is named by files the test writes
//! into the throw-away folder before `Starten`: `modus`, `hex` (the bytes to send), `exit` (its own exit code).

#![cfg(target_os = "linux")]

mod gemeinsam;

use std::io::{ErrorKind, Read, Write};
use std::os::unix::fs::{FileTypeExt, PermissionsExt};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use gemeinsam::{DIENST, Probe, Prozess, Wegwerf, startdatei};
use serde_json::{Value, json};

/// The play of `lauf`. `nie`: no connection. `zeile`: connects, sends `hex`. `exec`: connects and then replaces itself (the socket is
/// closed on `exec`, like the trampoline of `lauf` does). `halten`: connects, sends `hex`, says `verbunden` and waits for `weiter`.
/// `fremd`: waits for `fremd.ok`, then as `zeile`. `still`: no connection, waits for `weiter`. `kill`: connects, then SIGKILL to itself.
const LAUF: &str = r#"#!/usr/bin/python3 -I
import os, socket, sys, time
d = os.environ["M_ORDNER"]
def warte(n):
    ende = time.time() + 30
    while not os.path.exists(d + "/" + n) and time.time() < ende:
        time.sleep(0.01)
def datei(n):
    return open(d + "/" + n).read().strip()
tmp = d + "/argv.tmp"
open(tmp, "w").write("\n".join(sys.argv))
os.rename(tmp, d + "/argv.txt")
modus = datei("modus")
code = int(datei("exit"))
if modus == "nie":
    sys.exit(code)
if modus == "still":
    warte("weiter")
    sys.exit(code)
if modus == "fremd":
    warte("fremd.ok")
strom = socket.socket(socket.AF_UNIX)
strom.connect(sys.argv[2])
if modus == "exec":
    os.execv("/bin/sh", ["sh", "-c", "exit %d" % code])
if modus == "kill":
    os.kill(os.getpid(), 9)
strom.sendall(bytes.fromhex(datei("hex")))
if modus == "halten":
    open(d + "/verbunden", "w").close()
    warte("weiter")
sys.exit(code)
"#;

/// A start file with `lauf` and no first session; the program of the sessions plays `lauf`.
fn lauf_start(ort: &Wegwerf) -> Value {
    let programm = ort.0.join("lauf.py");
    std::fs::write(&programm, LAUF).expect("script");
    std::fs::set_permissions(&programm, std::fs::Permissions::from_mode(0o700)).expect("mode");
    let einst = ort.0.join("einst.json");
    std::fs::write(
        &einst,
        include_str!("werkzeug/einstellungen/vorfuehrung.json"),
    )
    .expect("settings");
    let mut start = startdatei(ort, &[DIENST], &json!({}));
    let o = start.as_object_mut().expect("object");
    o.remove("kind");
    o.remove("groesse");
    start["anfang"]["sitzungen"] = json!([]);
    start["lauf"] = json!({
        "programm": programm.to_string_lossy(),
        "env": {"PATH": "/usr/bin:/bin", "LC_ALL": "C.UTF-8", "M_ORDNER": ort.0.to_string_lossy()},
        "groesse": [40, 10],
        "einstellungen": einst.to_string_lossy(),
    });
    start
}

fn gut() -> Value {
    json!({"fassung": 1, "harness": "claude", "modell": "m", "route": "cloud-beispiel", "kunde": "k-beispiel", "arbeitsordner": "/tmp"})
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    bytes.iter().fold(String::new(), |mut strom, puf| {
        let _ = write!(strom, "{puf:02x}");
        strom
    })
}

/// What the next session plays: the files `modus`, `hex` and `exit`; the files of the one before are removed.
fn spielen(ort: &Wegwerf, modus: &str, zeile: &[u8], code: i32) {
    for n in ["argv.txt", "verbunden", "weiter", "fremd.ok"] {
        let _ = std::fs::remove_file(ort.0.join(n));
    }
    std::fs::write(ort.0.join("modus"), modus).expect("modus");
    std::fs::write(ort.0.join("hex"), hex(zeile)).expect("hex");
    std::fs::write(ort.0.join("exit"), code.to_string()).expect("exit");
}

fn anlegen(ort: &Wegwerf, n: &str) {
    std::fs::write(ort.0.join(n), "").expect("file");
}

/// Sends `Starten` and returns the identifier of `Gestartet`.
fn starten(proz: &mut Probe) -> String {
    let n = proz.nachrichten.len();
    proz.ereignis(&json!({"art": "Starten", "start": gut()}));
    assert!(
        proz.warte(Duration::from_secs(10), |proz| proz.nachrichten[n..]
            .iter()
            .any(|m| m["art"] == "Gestartet")),
        "no Gestartet: {:?}",
        proz.ablehnungen
    );
    proz.nachrichten[n..]
        .iter()
        .find(|m| m["art"] == "Gestartet")
        .and_then(|m| m["sitzung"].as_str())
        .expect("id")
        .to_owned()
}

/// The entry of the session `id` in the last list, after a barrier.
fn eintrag(proz: &mut Probe, id: &str) -> Option<Value> {
    let zust = proz.zustand();
    assert!(zust.is_object());
    proz.nachrichten
        .iter()
        .rev()
        .find(|m| m["art"] == "Sitzungsliste")
        .and_then(|m| m["sitzungen"].as_array().cloned())
        .and_then(|laenge_l| laenge_l.into_iter().find(|strom| strom["id"] == id))
}

/// Waits until the session `id` is `beendet` and returns its entry of the list.
fn warte_beendet(proz: &mut Probe, id: &str) -> Value {
    let ende = Instant::now() + Duration::from_secs(20);
    loop {
        if let Some(strom) = eintrag(proz, id)
            && strom["zustand"] == "beendet"
        {
            return strom;
        }
        assert!(
            Instant::now() < ende,
            "{id} did not end: {:?}",
            eintrag(proz, id)
        );
        proz.lesen(Duration::from_millis(20));
    }
}

/// Waits at most 20 strom for a file of the throw-away folder.
fn warte_datei(ort: &Wegwerf, name: &str) {
    let ende = Instant::now() + Duration::from_secs(20);
    while !ort.0.join(name).exists() {
        assert!(Instant::now() < ende, "{name} did not appear");
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn beenden(mut proz: Prozess) {
    proz.signal("TERM");
    assert_eq!(proz.warte_ende(Duration::from_secs(15)), Some(0));
}

/// The dienst with a viewer.
fn dienst(ort: &Wegwerf) -> (Prozess, Probe) {
    let proz = Prozess::starten(ort, &lauf_start(ort));
    let ans = Probe::neu(&ort.socket(), false);
    (proz, ans)
}

#[test]
fn lauf_bekommt_die_option_meldung_vor_der_startdatei_und_der_socket_ist_0600() {
    let ort = Wegwerf::neu("meldung-aufruf");
    let (proz, mut ans) = dienst(&ort);
    spielen(&ort, "still", b"", 0);
    let id = starten(&mut ans);
    warte_datei(&ort, "argv.txt");
    let argv: Vec<String> = std::fs::read_to_string(ort.0.join("argv.txt"))
        .expect("argv")
        .lines()
        .map(str::to_owned)
        .collect();
    let ordner = ort.ordner().join(&id);
    let sock = ordner.join("meldung.sock");
    assert_eq!(
        argv[1..],
        [
            "--meldung".to_owned(),
            sock.to_string_lossy().into_owned(),
            ordner.join("start.json").to_string_lossy().into_owned()
        ],
        "argv {argv:?}"
    );
    assert_eq!(argv.len(), 4);
    assert!(sock.is_absolute());
    let meta = std::fs::symlink_metadata(&sock).expect("the socket exists while the session runs");
    assert!(meta.file_type().is_socket());
    assert_eq!(meta.permissions().mode() & 0o7777, 0o600);
    anlegen(&ort, "weiter");
    let res = warte_beendet(&mut ans, &id);
    assert_eq!(res["code"], json!(0));
    assert!(res.get("grund").is_none(), "{res}");
    assert!(
        !ordner.exists(),
        "the folder with the socket is removed at the end"
    );
    beenden(proz);
}

#[test]
fn eine_zeile_gibt_den_code_und_den_grund_der_sitzung() {
    let ort = Wegwerf::neu("meldung-zeile");
    let (proz, mut ans) = dienst(&ort);
    // `lauf` ends with 1 but reports 69
    spielen(&ort, "zeile", b"69 lauf.harness_fehlt\n", 1);
    let id = starten(&mut ans);
    let res = warte_beendet(&mut ans, &id);
    assert_eq!(res["code"], json!(69), "{res}");
    assert_eq!(res["grund"], json!("lauf.harness_fehlt"), "{res}");
    let zust = ans.zustand();
    assert_eq!(
        zust["dienst"]["kind"][&id]["code"],
        json!(69),
        "the core got the same code: {zust}"
    );
    // the report counts also when the program itself ends with 0, and the borders of the form
    spielen(&ort, "zeile", b"1 a\n", 0);
    let id = starten(&mut ans);
    let res = warte_beendet(&mut ans, &id);
    assert_eq!(
        (&res["code"], &res["grund"]),
        (&json!(1), &json!("a")),
        "{res}"
    );
    let lang = format!("255 {}\n", "z9_.".repeat(16));
    spielen(&ort, "zeile", lang.as_bytes(), 0);
    let id = starten(&mut ans);
    let res = warte_beendet(&mut ans, &id);
    assert_eq!(res["code"], json!(255), "{res}");
    assert_eq!(res["grund"], json!("z9_.".repeat(16)), "{res}");
    beenden(proz);
}

#[test]
fn eof_ohne_daten_laesst_den_code_beim_starter() {
    let ort = Wegwerf::neu("meldung-eof");
    let (proz, mut ans) = dienst(&ort);
    // `lauf` connects, then the `exec` closes the connection: the starter ends with 7
    spielen(&ort, "exec", b"", 7);
    let id = starten(&mut ans);
    let res = warte_beendet(&mut ans, &id);
    assert_eq!(res["code"], json!(7), "{res}");
    assert!(res.get("grund").is_none(), "{res}");
    beenden(proz);
}

#[test]
fn ohne_verbindung_gehoert_der_code_lauf() {
    let ort = Wegwerf::neu("meldung-nie");
    let (proz, mut ans) = dienst(&ort);
    spielen(&ort, "nie", b"", 77);
    let id = starten(&mut ans);
    let res = warte_beendet(&mut ans, &id);
    assert_eq!(res["code"], json!(77), "{res}");
    assert!(res.get("grund").is_none(), "{res}");
    beenden(proz);
}

#[test]
fn stirbt_lauf_nach_dem_connect_an_sigkill_sieht_der_dienst_eof_und_das_signal() {
    let ort = Wegwerf::neu("meldung-kill");
    let (proz, mut ans) = dienst(&ort);
    spielen(&ort, "kill", b"", 0);
    let id = starten(&mut ans);
    let res = warte_beendet(&mut ans, &id);
    assert!(res.get("grund").is_none(), "no line, no reason: {res}");
    assert_ne!(
        res["code"],
        json!(0),
        "the end of the child carries the signal: {res}"
    );
    beenden(proz);
}

#[test]
fn jede_andere_zeile_ist_lauf_meldung_ungueltig_mit_dem_festen_code_70() {
    let ort = Wegwerf::neu("meldung-ungueltig");
    let (proz, mut ans) = dienst(&ort);
    let ueber = vec![b'a'; 200];
    let zu_lang = format!("7 {}\n", "a".repeat(65));
    let faelle: Vec<(&str, &[u8])> = vec![
        ("ohne grund", b"69\n"),
        ("code kein zahl", b"abc d\n"),
        ("grosser buchstabe", b"69 Upper\n"),
        ("bindestrich", b"69 a-puf\n"),
        ("code 0", b"0 a\n"),
        ("code 256", b"256 a\n"),
        ("fuehrende null", b"007 a\n"),
        ("zwei leerzeichen", b"69  a\n"),
        ("leere zeile", b"\n"),
        ("crlf", b"69 a\r\n"),
        ("daten danach", b"69 a\nxx"),
        ("zwei zeilen", b"69 a\n69 a\n"),
        ("ohne zeilenende", b"69 a"),
        ("200 byte ohne zeilenende", &ueber),
        ("grund 65 zeichen", zu_lang.as_bytes()),
    ];
    // the code is 70 whatever the child ends with, also 0: a failed start never looks like a success
    for (name, bytes) in faelle {
        for ende in [5, 0] {
            spielen(&ort, "zeile", bytes, ende);
            let id = starten(&mut ans);
            let res = warte_beendet(&mut ans, &id);
            assert_eq!(
                res["code"],
                json!(70),
                "{name}, end {ende}: the fixed code of an invalid line: {res}"
            );
            assert_eq!(
                res["grund"],
                json!("lauf.meldung_ungueltig"),
                "{name}, end {ende}: {res}"
            );
        }
    }
    // 200 bytes with a line end after 10: the service reads 128 and sees data after the line
    let mut mit_ende = b"69 a\n".to_vec();
    mit_ende.extend_from_slice(&ueber);
    spielen(&ort, "zeile", &mit_ende, 5);
    let id = starten(&mut ans);
    let res = warte_beendet(&mut ans, &id);
    assert_eq!(res["grund"], json!("lauf.meldung_ungueltig"), "{res}");
    beenden(proz);
}

#[test]
fn ein_fremder_prozess_der_sich_zuerst_verbindet_wird_geschlossen_und_ein_zweiter_nach_lauf_auch() {
    let ort = Wegwerf::neu("meldung-fremd");
    let (proz, mut ans) = dienst(&ort);
    spielen(&ort, "fremd", b"69 lauf.echt\n", 1);
    let id = starten(&mut ans);
    warte_datei(&ort, "argv.txt");
    let sock: PathBuf = ort.ordner().join(&id).join("meldung.sock");
    // this test process is not the child: the service closes the connection at once and reads nothing from it
    let mut fremd = UnixStream::connect(&sock).expect("the socket listens");
    fremd
        .set_read_timeout(Some(Duration::from_secs(10)))
        .expect("timeout");
    let _ = fremd.write_all(b"99 fremd.zeile\n");
    let mut puffer = [0_u8; 8];
    match fremd.read(&mut puffer) {
        Ok(0) => {}
        Err(res) if res.kind() != ErrorKind::WouldBlock && res.kind() != ErrorKind::TimedOut => {}
        r => panic!("the foreign connection was not closed: {r:?}"),
    }
    // the right child connects now and its line counts
    anlegen(&ort, "fremd.ok");
    let res = warte_beendet(&mut ans, &id);
    assert_eq!(res["code"], json!(69), "{res}");
    assert_eq!(res["grund"], json!("lauf.echt"), "{res}");

    // a connection after the one of the child: it holds the connection, a second one finds the listener closed
    spielen(&ort, "halten", b"69 lauf.erste\n", 1);
    let id = starten(&mut ans);
    warte_datei(&ort, "verbunden");
    let sock = ort.ordner().join(&id).join("meldung.sock");
    let ende = Instant::now() + Duration::from_secs(10);
    loop {
        match UnixStream::connect(&sock) {
            Err(_) => break,
            Ok(mut strom) => {
                strom
                    .set_read_timeout(Some(Duration::from_millis(200)))
                    .expect("timeout");
                let _ = strom.write_all(b"99 zweite.zeile\n");
                let mut puf = [0_u8; 8];
                match strom.read(&mut puf) {
                    Ok(0) => break,
                    Err(res)
                        if !matches!(res.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) =>
                    {
                        break;
                    }
                    _ => {}
                }
            }
        }
        assert!(
            Instant::now() < ende,
            "a second connection stayed open after the one of the child"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    anlegen(&ort, "weiter");
    let res = warte_beendet(&mut ans, &id);
    assert_eq!(res["code"], json!(69), "{res}");
    assert_eq!(res["grund"], json!("lauf.erste"), "{res}");
    beenden(proz);
}

#[test]
fn ein_zu_langer_pfad_lehnt_den_start_ab_und_laesst_nichts_zurueck() {
    // the socket folder is so long that `dienst.sock` is within the limit of 100 bytes and `<kennung>/meldung.sock` is not
    let mut laenge = 1_usize;
    let ort = loop {
        let ort = Wegwerf::neu(&"n".repeat(laenge));
        let laenge_l = ort.ordner().as_os_str().len();
        if (84..=88).contains(&laenge_l) {
            break ort;
        }
        laenge = (laenge + 86).saturating_sub(laenge_l).max(1);
        assert!(laenge < 200, "no folder of the wanted length");
    };
    let ordner = ort.ordner();
    let laenge_l = ordner.as_os_str().len();
    assert!((84..=88).contains(&laenge_l), "{laenge_l}");
    let mut start = lauf_start(&ort);
    start["socket"]["ordner"] = json!(ordner.to_string_lossy());
    // `Prozess::starten` waits for `<ort>/o/dienst.sock`: start by hand when the folder is another
    let datei = gemeinsam::datei_schreiben(&ort, &start);
    let mut kind = std::process::Command::new(DIENST)
        .arg(&datei)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .expect("wb-dienst starts");
    let sock = ordner.join("dienst.sock");
    let frist = Instant::now() + Duration::from_secs(10);
    while !sock.exists() {
        assert!(Instant::now() < frist, "the socket did not appear");
        std::thread::sleep(Duration::from_millis(10));
    }
    spielen(&ort, "still", b"", 0);
    let mut ans = Probe::neu(&sock, false);
    let n = ans.ablehnungen.len();
    ans.ereignis(&json!({"art": "Starten", "start": gut()}));
    assert!(
        ans.warte(Duration::from_secs(10), |proz| proz.ablehnungen.len() > n),
        "no refusal"
    );
    assert_eq!(
        ans.ablehnungen.last().map(String::as_str),
        Some("meldung_kanal")
    );
    assert!(
        !ans.nachrichten.iter().any(|m| m["art"] == "Gestartet"),
        "no session was started"
    );
    assert!(
        !ort.0.join("argv.txt").exists(),
        "the program was not started"
    );
    assert!(
        !ordner.join("s-1").exists(),
        "the folder of the refused start is removed"
    );
    // SIGTERM ends the service in an orderly way
    let _ = std::process::Command::new("kill")
        .args(["-TERM", &kind.id().to_string()])
        .status();
    let ende = Instant::now() + Duration::from_secs(15);
    while kind.try_wait().expect("try_wait").is_none() {
        assert!(Instant::now() < ende, "the service did not end");
        std::thread::sleep(Duration::from_millis(10));
    }
}
