//! Tests of the report of `lauf` (contract `dienst`, section 18; Linux and macOS): the program `wb-dienst` makes the socket `meldung.sock` in the
//! folder of every session of `Starten`, starts `lauf` with `--meldung <pfad>` before the start file, accepts one connection of the child
//! and reads one line `<code> <grund>\n` from it.
//!
//! The program of a session is a Python script (`lauf.programm`) that plays `lauf`. What it does is named by files the test writes
//! into the throw-away folder before `Starten`: `modus`, `hex` (the bytes to send), `exit` (its own exit code).

#![cfg(any(target_os = "linux", target_os = "macos"))]

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
/// `anfordern` (Nachtrag 28.11.2026): connects, sends `hex`, then for each line `<hex> <n>` of the file `schritte` sends the bytes and
/// reads `n` answer lines; it writes all answers to `antworten` and ends.
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
if modus == "anfordern":
    strom.settimeout(20)
    leser = strom.makefile("rb")
    antworten = b""
    for schritt in datei("schritte").splitlines():
        teil, n = schritt.split(" ")
        strom.sendall(bytes.fromhex(teil))
        for _ in range(int(n)):
            antworten += leser.readline()
    tmp = d + "/antworten.tmp"
    open(tmp, "wb").write(antworten)
    os.rename(tmp, d + "/antworten")
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
    for n in [
        "argv.txt",
        "verbunden",
        "weiter",
        "fremd.ok",
        "schritte",
        "antworten",
    ] {
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
    gestartet(proz, &json!({"art": "Starten", "start": gut()}))
}

/// Sends `e` (`Starten` or `Fortsetzen`) and returns the identifier of `Gestartet`.
fn gestartet(proz: &mut Probe, e: &Value) -> String {
    let n = proz.nachrichten.len();
    proz.ereignis(e);
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

/// Nachtrag 27.11.2026 (Fortsetzen, contract `start`, step 5d): the codes of `lauf` for a continued session that cannot go on come
/// unchanged as the end of the new session, with code and reason. DI-L3 point 2 (Nachtrag 27.11.2026): `lauf` ended before any child,
/// so the old session loses `fortgesetzt_als` and can be continued again; the new one keeps `fortsetzung_von`.
#[test]
fn die_codes_des_fortsetzens_von_lauf_sind_das_ende_der_neuen_sitzung() {
    let ort = Wegwerf::neu("meldung-fortsetzen");
    let (proz, mut ans) = dienst(&ort);
    let f2 = json!({"fassung": 2, "harness": "claude", "modell": "m", "route": "cloud-beispiel", "kunde": "k-beispiel",
                    "arbeitsordner": "/tmp", "effort": null, "name": null});
    for (code, grund) in [
        (69, "lauf.fortsetzung_unmoeglich"),
        (66, "lauf.fortsetzung_fehlt"),
        (65, "lauf.fortsetzung_kaputt"),
        (65, "lauf.fortsetzung_fremd"),
        (73, "lauf.sitzung_belegt"),
    ] {
        spielen(&ort, "zeile", b"0 lauf.ende\n", 0);
        let alt = gestartet(&mut ans, &json!({"art": "Starten", "start": f2}));
        warte_beendet(&mut ans, &alt);
        spielen(&ort, "zeile", format!("{code} {grund}\n").as_bytes(), 1);
        let neu = gestartet(&mut ans, &json!({"art": "Fortsetzen", "sitzung": alt}));
        let res = warte_beendet(&mut ans, &neu);
        assert_eq!(
            (&res["code"], &res["grund"], &res["fortsetzung_von"]),
            (&json!(code), &json!(grund), &json!(alt)),
            "{res}"
        );
        assert_eq!(
            eintrag(&mut ans, &alt).map(|e| e.get("fortgesetzt_als").cloned()),
            Some(None)
        );
        // the old session can be continued again
        spielen(&ort, "zeile", format!("{code} {grund}\n").as_bytes(), 1);
        let zweite = gestartet(&mut ans, &json!({"art": "Fortsetzen", "sitzung": alt}));
        assert_ne!(zweite, neu);
        warte_beendet(&mut ans, &zweite);
    }
    beenden(proz);
}

/// DI-L3 point 2 (Nachtrag 27.11.2026), fail closed: without a valid line of `lauf` it is open whether a child ran, so the old session
/// keeps `fortgesetzt_als` (a line not of the form, and an end after the `exec`, where the connection ends without data).
#[test]
fn ohne_gueltige_zeile_bleibt_die_alte_sitzung_fortgesetzt() {
    let ort = Wegwerf::neu("meldung-fortgesetzt");
    let (proz, mut ans) = dienst(&ort);
    let f2 = json!({"fassung": 2, "harness": "claude", "modell": "m", "route": "cloud-beispiel", "kunde": "k-beispiel",
                    "arbeitsordner": "/tmp", "effort": null, "name": null});
    for (modus, zeile, code) in [
        ("zeile", &b"66 Lauf.fortsetzung_fehlt\n"[..], 66),
        ("exec", &b""[..], 3),
    ] {
        spielen(&ort, "zeile", b"0 lauf.ende\n", 0);
        let alt = gestartet(&mut ans, &json!({"art": "Starten", "start": f2}));
        warte_beendet(&mut ans, &alt);
        spielen(&ort, modus, zeile, code);
        let neu = gestartet(&mut ans, &json!({"art": "Fortsetzen", "sitzung": alt}));
        let res = warte_beendet(&mut ans, &neu);
        assert_eq!(res["fortsetzung_von"], json!(alt), "{res}");
        assert_eq!(
            eintrag(&mut ans, &alt).map(|e| e["fortgesetzt_als"].clone()),
            Some(json!(neu)),
            "{modus}"
        );
    }
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
    // EINTR (a read with a timeout after the process was stopped and continued, Linux) is no answer: read again
    let gelesen = loop {
        match fremd.read(&mut puffer) {
            Err(e) if e.kind() == ErrorKind::Interrupted => {}
            r => break r,
        }
    };
    match gelesen {
        Ok(0) => {}
        Err(res) if res.kind() != ErrorKind::WouldBlock && res.kind() != ErrorKind::TimedOut => {}
        r => panic!("the foreign connection was not closed: {r:?}"),
    }
    // the right child connects now and its line counts
    anlegen(&ort, "fremd.ok");
    let res = warte_beendet(&mut ans, &id);
    assert_eq!(res["code"], json!(69), "{res}");
    assert_eq!(res["grund"], json!("lauf.echt"), "{res}");

    // a connection after the one of the child: it holds the connection, and the service closes its listener (R-B3). Proof without a race and
    // without filling the backlog: one connection of this test. Made before the close, it waits in the backlog and ends when the listener
    // closes (`Ok(0)` or a reset); made after it, `connect` is refused. A listener that stays open leaves the connection waiting until the
    // deadline: red. After that a fresh connection must be refused as well (one waiting connection does not fill the backlog of 4).
    spielen(&ort, "halten", b"69 lauf.erste\n", 1);
    let id = starten(&mut ans);
    warte_datei(&ort, "verbunden");
    let sock = ort.ordner().join(&id).join("meldung.sock");
    let geschlossen =
        |e: &std::io::Error| matches!(e.kind(), ErrorKind::ConnectionRefused | ErrorKind::NotFound);
    match UnixStream::connect(&sock) {
        Err(e) => assert!(geschlossen(&e), "connect: {e:?}"),
        Ok(mut strom) => {
            strom
                .set_read_timeout(Some(Duration::from_secs(10)))
                .expect("timeout");
            let _ = strom.write_all(b"99 zweite.zeile\n");
            let mut puf = [0_u8; 8];
            // EINTR is no answer, as above: read again
            let gelesen = loop {
                match strom.read(&mut puf) {
                    Err(e) if e.kind() == ErrorKind::Interrupted => {}
                    r => break r,
                }
            };
            match gelesen {
                Ok(0) => {}
                Err(e) if !matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {}
                r => panic!(
                    "a second connection stayed open after the one of the child: the listener is open ({r:?})"
                ),
            }
        }
    }
    let e = UnixStream::connect(&sock).expect_err("the listener is closed: no second connection");
    assert!(geschlossen(&e), "connect: {e:?}");
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

/// Nachtrag 28.11.2026 (stage 1): after `laeuft` the script sends requests and reads the answers on the same connection. Every valid
/// request gets `abgelehnt stelle_nicht_erreichbar`, never `frei`; a second one before the answer to the first gets
/// `abgelehnt anfordern_offen`; every other line gets no answer, and only their number goes to the log. The child ends with 0 and the
/// session has code 0 without `grund`.
#[test]
fn nach_laeuft_bekommt_jede_gueltige_anforderung_eine_ablehnung_und_nie_frei() {
    let ort = Wegwerf::neu("meldung-anfordern");
    let (proz, mut ans) = dienst(&ort);
    let gut = b"anfordern worker pi qwen3:8b medium\n";
    // the form is right, but with its line end the line has 257 bytes
    let lang = format!(
        "anfordern {} {} {} e\n",
        "r".repeat(64),
        "h".repeat(64),
        "m".repeat(114)
    )
    .into_bytes();
    assert_eq!(lang.len(), 257);
    let ungueltig = [
        &b"69 lauf.harness_fehlt\n"[..],
        b"laeuft\n",
        b"anfordern Worker pi m e\n",
        &lang,
    ]
    .concat();
    let schritte = format!(
        "{} 1\n{} 1\n{} 2\n",
        hex(gut),
        hex(&[&ungueltig[..], gut].concat()),
        hex(&[&gut[..], gut].concat())
    );
    spielen(&ort, "anfordern", b"laeuft\n", 0);
    std::fs::write(ort.0.join("schritte"), schritte).expect("steps");
    let id = starten(&mut ans);
    let res = warte_beendet(&mut ans, &id);
    let antworten = std::fs::read_to_string(ort.0.join("antworten")).expect("answers");
    assert_eq!(
        antworten,
        "abgelehnt stelle_nicht_erreichbar\nabgelehnt stelle_nicht_erreichbar\n\
         abgelehnt stelle_nicht_erreichbar\nabgelehnt anfordern_offen\n"
    );
    assert_eq!(res["code"], json!(0), "{res}");
    assert!(res.get("grund").is_none(), "{res}");
    let log = proz.stderr_zeilen();
    assert!(
        log.iter()
            .any(|z| z == &format!("wb-dienst: meldung {id} verworfen: 4 Zeilen")),
        "{log:?}"
    );
    assert!(
        !log.iter().any(|z| z.contains("anfordern")
            || z.contains("harness_fehlt")
            || z.contains("Worker")),
        "no byte of a line in the log: {log:?}"
    );
    beenden(proz);
}

/// The length of `laeuft\n`.
const LAEUFT_LAENGE: usize = 7;

/// Nachtrag 28.11.2026: `laeuft` and the end of the child: the code of the child counts without `grund`, whether the connection ends
/// at once, a code line follows `laeuft` (dropped, the code does not change) or a second `laeuft`; a start without `laeuft` (end without
/// data) stays valid.
#[test]
fn nach_laeuft_gilt_der_code_des_kindes_ohne_grund() {
    let ort = Wegwerf::neu("meldung-laeuft");
    let (proz, mut ans) = dienst(&ort);
    for (zeile, ende) in [
        (&b"laeuft\n"[..], 0),
        (b"laeuft\n", 3),
        (b"laeuft\n69 lauf.harness_fehlt\n", 5),
        (b"laeuft\nlaeuft\n", 0),
        (b"laeuft\nanfordern a b", 0),
        (b"", 0),
    ] {
        spielen(&ort, "zeile", zeile, ende);
        let id = starten(&mut ans);
        let res = warte_beendet(&mut ans, &id);
        assert_eq!(res["code"], json!(ende), "{zeile:?}: {res}");
        assert!(res.get("grund").is_none(), "{zeile:?}: {res}");
        // a line in the log only when lines were dropped
        let verworfen = usize::from(zeile.len() > LAEUFT_LAENGE);
        let zeilen = proz.stderr_zeilen();
        let im_log = zeilen
            .iter()
            .filter(|z| z.starts_with(&format!("wb-dienst: meldung {id} ")))
            .count();
        assert_eq!(im_log, verworfen, "{zeile:?}: {zeilen:?}");
    }
    // `laeuft` is no code line: without its line end it is a line that has not ended
    spielen(&ort, "zeile", b"laeuft", 0);
    let id = starten(&mut ans);
    let res = warte_beendet(&mut ans, &id);
    assert_eq!(
        (&res["code"], &res["grund"]),
        (&json!(70), &json!("lauf.meldung_ungueltig")),
        "{res}"
    );
    beenden(proz);
}
