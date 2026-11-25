//! The server under Linux against stand-ins of a model target and an MCP server on 127.0.0.1 (plain TCP, allowed only
//! for a loopback host): the security promises of section 11 that hold without root and without foreign processes
//! (W3, W4, W7, W8 and L6 to L9 in the form of version 1), the role from the program (L3 in part) and the search.
//!
//! The peer is this test process; its program is made the role program of the test (root of the role programs: the
//! folder of the test binary, owner: the own uid). Throw-away folders lie below `CARGO_TARGET_TMPDIR`.
#![cfg(target_os = "linux")]
#![allow(
    clippy::many_single_char_names,
    reason = "a, c, d, z: stand-in, client, server, lines in every test"
)]

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use halter::ablage::Ablage;
use halter::b64;
use halter::dienst::Dienst;
use halter::konfig;
use halter::pruefung::Rolle;
use serde_json::{Value, json};
use std::collections::BTreeMap;

const MARKE: &str = "hgx1-geheimwert-0123";
const JETZT: i64 = 1_795_000_000_000;
const START: i64 = 1_794_996_400_000;
const PERSON: &str = "ps_a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1";

static ZAEHLER: AtomicUsize = AtomicUsize::new(0);

fn uhr() -> i64 {
    JETZT
}

fn ordner() -> PathBuf {
    let n = ZAEHLER.fetch_add(1, Ordering::Relaxed);
    let p = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("halter-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p).expect("folder");
    std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o700)).expect("mode");
    p
}

fn heim_mit(geheimnisse: &[(&str, &str)]) -> PathBuf {
    let heim = ordner();
    let mut g = heim.clone();
    for e in halter::ablage::EBENEN {
        g = g.join(e);
        std::fs::create_dir(&g).expect("level");
        std::fs::set_permissions(&g, std::fs::Permissions::from_mode(0o700)).expect("mode");
    }
    for (n, w) in geheimnisse {
        let p = g.join(n);
        std::fs::write(&p, format!("{w}\n")).expect("secret");
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o600)).expect("mode");
    }
    heim
}

fn uid() -> u32 {
    rustix::process::getuid().as_raw()
}

fn egid() -> u32 {
    rustix::process::getegid().as_raw()
}

/// The role groups of a server in which the effective gid of this process (the peer of every test connection) is the
/// group of `rolle`.
fn eigene_gid_als(rolle: &str) -> BTreeMap<u32, Rolle> {
    BTreeMap::from([(egid(), Rolle::aus_text(rolle).expect("role"))])
}

/// A server with the role `rolle` for this process, the model target and the MCP server at `port`.
fn dienst(rolle: &str, port: u16, heim: &Path, start_ms: i64) -> Arc<Dienst> {
    let kid = "qvy5SgFhefK2a9hsceqk+MJ5evYpEJTTdGpqO3O85VY=";
    let k = json!({
        "fassung": 1,
        "konten": {uid().to_string(): {"person": PERSON}},
        "rollen": {rolle: "wb-test"},
        "ziele": {"copilot": {"host": "127.0.0.1", "port": port, "tls": false, "praefix": "", "kopf": "authorization",
            "vorsatz": "Bearer ", "schluessel": "anmeldung.copilot",
            "pfade": ["GET /copilot_internal/*", "GET /responses", "POST /chat/completions"]}},
        "server": {"tickets": {"host": "127.0.0.1", "port": port, "tls": false, "pfad": "/mcp", "kopf": "authorization",
            "vorsatz": "Bearer ", "schluessel": "mcp.tickets", "freigabe": true,
            "werkzeuge": {"update": "schreibend", "get": "lesend"}, "lokal": false}},
        "kids": {"stelle-mcp-1": kid},
    });
    let konfig = konfig::lesen(k.to_string().as_bytes()).expect("konfig");
    let kette = heim.join("kette");
    std::fs::create_dir_all(&kette).expect("chain folder");
    Arc::new(Dienst::neu(
        konfig,
        Ablage::neu(heim, uid()),
        uid(),
        start_ms,
        eigene_gid_als(rolle),
        Some(kette),
        uhr,
    ))
}

/// One connection to the server.
struct Client {
    strom: UnixStream,
    vorrat: Vec<u8>,
    roh: Vec<u8>,
    /// The start of the connection: a connection that sends lines without end (a stream that never ends) fails after
    /// 12 s, well within the deadline of a mutant run.
    seit: std::time::Instant,
}

fn verbinden(d: &Arc<Dienst>) -> Client {
    let (a, b) = UnixStream::pair().expect("pair");
    let d = Arc::clone(d);
    std::thread::spawn(move || d.verbindung(b));
    // 5 s for every line: a line or an end that does not come fails the test (see `eine`) well within the deadline
    // of a mutant run
    a.set_read_timeout(Some(Duration::from_secs(5)))
        .expect("timeout");
    Client {
        strom: a,
        vorrat: Vec::new(),
        roh: Vec::new(),
        seit: std::time::Instant::now(),
    }
}

fn zeile(v: &Value) -> Vec<u8> {
    let mut b = halter::json::kanonisch(v).expect("canonical").into_bytes();
    b.push(b'\n');
    b
}

impl Client {
    fn senden(&mut self, v: &Value) {
        self.strom.write_all(&zeile(v)).expect("send");
    }

    fn eine(&mut self) -> Option<Value> {
        let mut buf = vec![0u8; 65_536];
        loop {
            assert!(
                self.seit.elapsed() < Duration::from_secs(12),
                "the connection runs longer than 12 s"
            );
            if let Some(p) = self.vorrat.iter().position(|b| *b == b'\n') {
                let rest = self.vorrat.split_off(p + 1);
                let z = std::mem::replace(&mut self.vorrat, rest);
                assert!(
                    halter::antwort::gueltig(&z[..z.len() - 1]),
                    "line outside the form"
                );
                return Some(serde_json::from_slice(&z).expect("json"));
            }
            match self.strom.read(&mut buf) {
                // a deadline is no end: the holder neither answered nor closed the connection
                Err(e)
                    if matches!(
                        e.kind(),
                        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                    ) =>
                {
                    panic!("no line and no end of the connection within the deadline")
                }
                Ok(0) | Err(_) => return None,
                Ok(n) => {
                    self.vorrat.extend_from_slice(&buf[..n]);
                    self.roh.extend_from_slice(&buf[..n]);
                }
            }
        }
    }

    /// Lines up to the first `antwort` or `ende`.
    fn bis_schluss(&mut self) -> Vec<Value> {
        let mut z = Vec::new();
        while let Some(v) = self.eine() {
            let art = v["art"].as_str().unwrap_or("").to_owned();
            z.push(v);
            if art == "antwort" || art == "ende" {
                break;
            }
        }
        z
    }

    fn marken(&self) -> usize {
        zaehlen(&self.roh, MARKE.as_bytes())
    }
}

fn zaehlen(heu: &[u8], nadel: &[u8]) -> usize {
    heu.windows(nadel.len()).filter(|f| *f == nadel).count()
}

fn teil_bytes(zeilen: &[Value]) -> Vec<u8> {
    zeilen
        .iter()
        .filter(|z| z["art"] == "teil")
        .flat_map(|z| b64::dekodieren(z["daten_b64"].as_str().expect("b64")).expect("b64"))
        .collect()
}

fn letzte_code(zeilen: &[Value]) -> Option<String> {
    zeilen
        .last()
        .and_then(|z| z["code"].as_str())
        .map(str::to_owned)
}

/// A stand-in target: every connection gets the next answer; it records each request (head and body) and, after the
/// answer, what else arrives until the close.
struct Attrappe {
    port: u16,
    anfragen: Arc<Mutex<Vec<Vec<u8>>>>,
    danach: Arc<Mutex<Vec<Vec<u8>>>>,
}

fn kopf_und_koerper(s: &mut TcpStream) -> Vec<u8> {
    let mut b = Vec::new();
    let mut buf = [0u8; 4096];
    loop {
        if let Some(p) = b.windows(4).position(|f| f == b"\r\n\r\n") {
            let kopf = String::from_utf8_lossy(&b[..p]).to_ascii_lowercase();
            let laenge: usize = kopf
                .lines()
                .find_map(|z| z.strip_prefix("content-length: "))
                .and_then(|n| n.trim().parse().ok())
                .unwrap_or(0);
            while b.len() < p + 4 + laenge {
                match s.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => b.extend_from_slice(&buf[..n]),
                }
            }
            return b;
        }
        match s.read(&mut buf) {
            Ok(0) | Err(_) => return b,
            Ok(n) => b.extend_from_slice(&buf[..n]),
        }
    }
}

fn attrappe(antworten: Vec<Vec<Vec<u8>>>) -> Attrappe {
    attrappe_mit(antworten, true)
}

/// `schliessen`: after its answer the stand-in closes its sending side; else it waits for the close of the holder.
fn attrappe_mit(antworten: Vec<Vec<Vec<u8>>>, schliessen: bool) -> Attrappe {
    let l = TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = l.local_addr().expect("addr").port();
    let anfragen = Arc::new(Mutex::new(Vec::new()));
    let danach = Arc::new(Mutex::new(Vec::new()));
    let (a2, d2) = (Arc::clone(&anfragen), Arc::clone(&danach));
    std::thread::spawn(move || {
        for teile in antworten {
            let Ok((mut s, _)) = l.accept() else { return };
            s.set_read_timeout(Some(Duration::from_secs(5)))
                .expect("timeout");
            let anfrage = kopf_und_koerper(&mut s);
            a2.lock().expect("lock").push(anfrage);
            for t in teile {
                if s.write_all(&t).is_err() {
                    break;
                }
                s.flush().expect("flush");
                std::thread::sleep(Duration::from_millis(30));
            }
            if schliessen {
                let _ = s.shutdown(std::net::Shutdown::Write);
            }
            s.set_read_timeout(Some(Duration::from_secs(20)))
                .expect("timeout");
            let mut rest = Vec::new();
            let _ = s.read_to_end(&mut rest);
            d2.lock().expect("lock").push(rest);
        }
    });
    Attrappe {
        port,
        anfragen,
        danach,
    }
}

fn aufruf(methode: &str, pfad: &str, koerper: &[u8]) -> Value {
    json!({"abfrage": null, "fassung": 1, "id": "m-1", "koerper_b64": b64::kodieren(koerper),
        "kopf": {"content-type": "application/json"}, "methode": methode, "op": "modell.aufruf",
        "pfad": pfad, "ziel": "copilot"})
}

fn strom_anfrage() -> Value {
    json!({"abfrage": null, "fassung": 1, "id": "m-2", "kopf": {"sec-websocket-key": "dGhlIHNhbXBsZSBub25jZQ==",
        "sec-websocket-version": "13"}, "op": "modell.strom", "pfad": "/responses", "ziel": "copilot"})
}

fn http(status: &str, kopf: &str, koerper: &[u8]) -> Vec<u8> {
    let mut b = format!(
        "HTTP/1.1 {status}\r\n{kopf}content-length: {}\r\n\r\n",
        koerper.len()
    )
    .into_bytes();
    b.extend_from_slice(koerper);
    b
}

fn rahmen(fin: bool, opcode: u8, nutz: &[u8]) -> Vec<u8> {
    let mut r = vec![if fin { 0x80 } else { 0 } | opcode];
    assert!(nutz.len() < 65_536);
    if nutz.len() < 126 {
        r.push(u8::try_from(nutz.len()).expect("len"));
    } else {
        r.push(126);
        r.extend_from_slice(&u16::try_from(nutz.len()).expect("len").to_be_bytes());
    }
    r.extend_from_slice(nutz);
    r
}

const ACHT_ZEHN_EINS: &[u8] = b"HTTP/1.1 101 Switching Protocols\r\nupgrade: websocket\r\nconnection: Upgrade\r\nsec-websocket-accept: s3pPLMBiTxaQ9kYGzzhZRbK+xOo=\r\n\r\n";

#[test]
fn vorhanden_nur_fuer_das_eigene_konto() {
    let heim = heim_mit(&[
        ("anmeldung.copilot", MARKE),
        (".anmeldung.copilot.tmp", "x"),
        ("modell.lokal", "hgx1-maschine"),
    ]);
    let d = dienst("bruecke", 9, &heim, START);
    let mut c = verbinden(&d);
    for (name, soll) in [
        ("anmeldung.copilot", true),
        ("mcp.tickets", false),
        ("modell.copilot", false),
        ("modell.lokal", true),
        ("anmeldung.lokal", false),
    ] {
        c.senden(&json!({"fassung": 1, "id": "q-1", "name": name, "op": "schluessel.vorhanden"}));
        let z = c.bis_schluss();
        assert_eq!(z[0]["vorhanden"], json!(soll), "{name}");
    }
    assert_eq!(c.marken(), 0);
}

/// `schluessel.vorhanden` by `ziel` (section 4): the holder takes the name from `halter.json`; a known target with its
/// secret is `true`, without it `false`, an unknown target `halter.ziel_unbekannt`; another role than `bruecke` with
/// `ziel` gets `halter.rolle`, and `name` keeps working for it.
#[test]
fn vorhanden_nach_ziel() {
    let frage =
        |ziel: &str| json!({"fassung": 1, "id": "q-1", "op": "schluessel.vorhanden", "ziel": ziel});
    let mit = heim_mit(&[("anmeldung.copilot", MARKE)]);
    let d = dienst("bruecke", 9, &mit, START);
    let mut c = verbinden(&d);
    c.senden(&frage("copilot"));
    assert_eq!(c.bis_schluss()[0]["vorhanden"], json!(true));
    c.senden(&frage("fremd"));
    assert_eq!(
        letzte_code(&c.bis_schluss()).as_deref(),
        Some("halter.ziel_unbekannt")
    );
    c.senden(&json!({"fassung": 1, "id": "q-1", "name": "anmeldung.copilot", "op": "schluessel.vorhanden",
        "ziel": "copilot"}));
    assert_eq!(
        letzte_code(&c.bis_schluss()).as_deref(),
        Some("halter.schluessel_unbekannt")
    );
    c.senden(&json!({"fassung": 1, "id": "q-1", "op": "schluessel.vorhanden"}));
    assert_eq!(
        letzte_code(&c.bis_schluss()).as_deref(),
        Some("halter.schluessel_fehlt")
    );
    assert_eq!(c.marken(), 0);
    let ohne = heim_mit(&[("mcp.tickets", MARKE)]);
    let d = dienst("bruecke", 9, &ohne, START);
    let mut c = verbinden(&d);
    c.senden(&frage("copilot"));
    let z = c.bis_schluss();
    assert_eq!(z[0]["ok"], json!(true));
    assert_eq!(
        z[0]["vorhanden"],
        json!(false),
        "a known target without its secret"
    );
    for rolle in ["hub", "oberflaeche"] {
        let d = dienst(rolle, 9, &mit, START);
        let mut c = verbinden(&d);
        c.senden(&frage("copilot"));
        assert_eq!(
            letzte_code(&c.bis_schluss()).as_deref(),
            Some("halter.rolle"),
            "{rolle}"
        );
        c.senden(&json!({"fassung": 1, "id": "q-1", "name": "anmeldung.copilot", "op": "schluessel.vorhanden"}));
        assert_eq!(
            c.bis_schluss()[0]["vorhanden"],
            json!(true),
            "{rolle} by name"
        );
    }
}

/// N1: a path outside the rules of the target ends with `halter.pfad_verboten`, reaches no target and writes the chain
/// line `halter.pfad_verboten` with method and path, without the fixed journal line; a path inside the rules is the
/// counter-check and reaches the target.
#[test]
fn n1_pfad_ausserhalb_der_regeln_geht_in_die_kette() {
    let a = attrappe(vec![vec![http("200 OK", "", b"ok")]]);
    let heim = heim_mit(&[("anmeldung.copilot", MARKE)]);
    let d = dienst("bruecke", a.port, &heim, START);
    let mut c = verbinden(&d);
    assert!(!d.kette_gemeldet());
    c.senden(&aufruf("GET", "/user/repos", b""));
    assert_eq!(
        letzte_code(&c.bis_schluss()).as_deref(),
        Some("halter.pfad_verboten")
    );
    assert_eq!(
        a.anfragen.lock().expect("lock").len(),
        0,
        "nothing reached the target"
    );
    let k = kette_text(&heim);
    let zeile = k
        .lines()
        .map(|z| serde_json::from_str::<Value>(z).expect("line"))
        .find(|z| z["aktionsname"] == "halter.pfad_verboten");
    // the register of contract 5 holds the kind: the line is in the chain, and the fixed journal line "Kette nicht
    // fortgeschrieben" does not come
    let z = zeile.expect("the chain line halter.pfad_verboten");
    let mut felder: Vec<&str> = z
        .as_object()
        .expect("object")
        .keys()
        .map(String::as_str)
        .collect();
    felder.sort_unstable();
    assert_eq!(
        felder,
        [
            "aktionsname",
            "ergebnis",
            "fassung",
            "grund",
            "methode",
            "op",
            "pfad",
            "vorher",
            "zeit",
            "ziel"
        ]
    );
    assert_eq!(z["methode"], "GET");
    assert_eq!(z["op"], "modell.aufruf");
    assert_eq!(z["pfad"], "/user/repos");
    assert_eq!(z["ziel"], "copilot");
    assert!(
        !d.kette_gemeldet(),
        "no journal line of a chain that cannot be continued"
    );
    let p = formate::kette::pruefe_quelle(
        &heim.join("kette"),
        "halter",
        formate::kette::NULL_HASH,
        None,
    )
    .expect("source");
    assert!(p.ok(), "{:?}", p.bruch);
    assert_eq!(zaehlen(k.as_bytes(), MARKE.as_bytes()), 0);
    c.senden(&aufruf("POST", "/chat/completions", b"{}"));
    let z = c.bis_schluss();
    assert_eq!(
        z.last().expect("end")["ok"],
        json!(true),
        "a path in the rules goes through"
    );
    assert_eq!(a.anfragen.lock().expect("lock").len(), 1);
}

/// Counter-check to `n1_pfad_ausserhalb_der_regeln_geht_in_die_kette`: with a broken anchor the chain cannot take the
/// line of a refused path, and the server notes the fixed journal line (F16) once.
#[test]
fn n1_kaputte_kette_meldet_die_feste_journalzeile() {
    let heim = heim_mit(&[("anmeldung.copilot", MARKE)]);
    let d = dienst("bruecke", 9, &heim, START);
    std::fs::write(heim.join("kette").join("halter.anker.jsonl"), b"kaputt\n").expect("anchor");
    let mut c = verbinden(&d);
    assert!(!d.kette_gemeldet());
    c.senden(&aufruf("GET", "/user/repos", b""));
    assert_eq!(
        letzte_code(&c.bis_schluss()).as_deref(),
        Some("halter.pfad_verboten")
    );
    assert!(
        d.kette_gemeldet(),
        "the fixed journal line of a broken chain"
    );
}

/// A server with the role `bruecke` and one target `copilot` at `port` with the rules `pfade` and, if given, the switch
/// of the first start `erststart_antwortform`.
fn dienst_erststart(
    port: u16,
    heim: &Path,
    pfade: &[&str],
    antwortform: Option<&[&str]>,
) -> Arc<Dienst> {
    let mut ziel = json!({"host": "127.0.0.1", "port": port, "tls": false, "praefix": "", "kopf": "authorization",
        "vorsatz": "Bearer ", "schluessel": "anmeldung.copilot", "pfade": pfade});
    if let Some(a) = antwortform {
        ziel["erststart_antwortform"] = json!(a);
    }
    let k = json!({"fassung": 1, "konten": {uid().to_string(): {"person": PERSON}}, "rollen": {"bruecke": "wb-test"},
        "ziele": {"copilot": ziel}, "server": {}, "kids": {}});
    let konfig = konfig::lesen(k.to_string().as_bytes()).expect("konfig");
    let kette = heim.join("kette");
    std::fs::create_dir_all(&kette).expect("chain folder");
    Arc::new(Dienst::neu(
        konfig,
        Ablage::neu(heim, uid()),
        uid(),
        START,
        eigene_gid_als("bruecke"),
        Some(kette),
        uhr,
    ))
}

/// The rules of the profile of the first start at the model host (decision of the lead, 25.11.2026).
const ERSTSTART: [&str; 2] = ["GET /models", "POST /chat/completions"];

/// The profile of the first start: each of the five paths not released at the first start ends with
/// `halter.pfad_verboten`, reaches no target and stands in the chain with method and path; the two released paths are
/// the counter-check and reach the target.
#[test]
fn erststart_sperrt_die_fuenf_pfade_mit_kettenzeile() {
    let a = attrappe(vec![
        vec![http("200 OK", "", b"{}")],
        vec![http("200 OK", "", b"{}")],
    ]);
    let heim = heim_mit(&[("anmeldung.copilot", MARKE)]);
    let d = dienst_erststart(a.port, &heim, &ERSTSTART, None);
    let mut c = verbinden(&d);
    let gesperrt = [
        ("modell.strom", "GET", "/responses"),
        ("modell.aufruf", "POST", "/auto"),
        ("modell.aufruf", "POST", "/models/session"),
        ("modell.aufruf", "POST", "/models/session/intent"),
        ("modell.aufruf", "POST", "/agents/sessions"),
    ];
    for (op, methode, pfad) in gesperrt {
        if op == "modell.strom" {
            c.senden(&strom_anfrage());
        } else {
            c.senden(&aufruf(methode, pfad, b"{}"));
        }
        assert_eq!(
            letzte_code(&c.bis_schluss()).as_deref(),
            Some("halter.pfad_verboten"),
            "{methode} {pfad}"
        );
    }
    assert_eq!(
        a.anfragen.lock().expect("lock").len(),
        0,
        "nothing reached the target"
    );
    let zeilen: Vec<Value> = kette_text(&heim)
        .lines()
        .map(|z| serde_json::from_str::<Value>(z).expect("line"))
        .filter(|z| z["aktionsname"] == "halter.pfad_verboten")
        .collect();
    assert_eq!(zeilen.len(), gesperrt.len());
    for (z, (op, methode, pfad)) in zeilen.iter().zip(gesperrt) {
        assert_eq!(z["op"], op);
        assert_eq!(z["methode"], methode);
        assert_eq!(z["pfad"], pfad);
        assert_eq!(z["ziel"], "copilot");
        assert_eq!(z["grund"], "pfad");
    }
    assert!(!d.kette_gemeldet());
    let p = formate::kette::pruefe_quelle(
        &heim.join("kette"),
        "halter",
        formate::kette::NULL_HASH,
        None,
    )
    .expect("source");
    assert!(p.ok(), "{:?}", p.bruch);
    // counter-check: the two released paths go through
    c.senden(&aufruf("GET", "/models", b""));
    assert_eq!(
        c.bis_schluss().last().expect("end")["ok"],
        json!(true),
        "GET /models"
    );
    c.senden(&aufruf("POST", "/chat/completions", b"{}"));
    assert_eq!(
        c.bis_schluss().last().expect("end")["ok"],
        json!(true),
        "POST /chat/completions"
    );
    assert_eq!(a.anfragen.lock().expect("lock").len(), 2);
    assert_eq!(d.antwortform_zahl(), 0, "no diagnosis without the switch");
    assert_eq!(d.antwortform_letzte(), None);
}

/// The answer form of the first start (target 4 of the order): with the switch for `POST /auto`, the answer
/// `{"data":[{"id":..}],"access_token":..}` gives the names `data`, `data[].id` and `access_token` and the mark "stays
/// closed"; the mark value stands neither in the journal line nor in the chain. The answer itself reaches the bridge
/// unchanged (the value is no held secret, so the leak lock lets it pass as before). A path in the rules without the
/// switch gives no line.
#[test]
fn antwortform_nennt_die_namen_ohne_werte() {
    const WERT: &str = "hgx9-antwortwert-0815";
    let koerper = format!(r#"{{"access_token":"{WERT}","data":[{{"id":"{WERT}"}}]}}"#);
    let a = attrappe(vec![
        vec![http(
            "200 OK",
            "content-type: application/json\r\n",
            koerper.as_bytes(),
        )],
        vec![http(
            "200 OK",
            "content-type: application/json\r\n",
            koerper.as_bytes(),
        )],
        vec![http(
            "200 OK",
            "content-type: application/json\r\n",
            koerper.as_bytes(),
        )],
    ]);
    let heim = heim_mit(&[("anmeldung.copilot", MARKE)]);
    let d = dienst_erststart(
        a.port,
        &heim,
        &["GET /models", "POST /auto", "POST /chat/completions"],
        Some(&["POST /auto"]),
    );
    let mut c = verbinden(&d);
    c.senden(&aufruf("POST", "/auto", b"{}"));
    let z = c.bis_schluss();
    assert_eq!(z.last().expect("end")["ok"], json!(true));
    assert_eq!(
        teil_bytes(&z),
        koerper.as_bytes(),
        "the answer goes on unchanged"
    );
    assert_eq!(d.antwortform_zahl(), 1);
    let zeile = d.antwortform_letzte().expect("the journal line");
    let text = zeile.strip_prefix("halter.antwortform: ").expect("prefix");
    let v: Value = serde_json::from_str(text.trim_end()).expect("line");
    assert_eq!(v["namen"], json!(["access_token", "data", "data[].id"]));
    assert_eq!(v["zu"], json!(true));
    assert_eq!(v["form"], "json");
    assert_eq!(v["methode"], "POST");
    assert_eq!(v["pfad"], "/auto");
    assert_eq!(v["ziel"], "copilot");
    assert_eq!(v["status"], 200);
    assert_eq!(zaehlen(zeile.as_bytes(), WERT.as_bytes()), 0);
    assert_eq!(zaehlen(zeile.as_bytes(), MARKE.as_bytes()), 0);
    let k = kette_text(&heim);
    assert_eq!(zaehlen(k.as_bytes(), WERT.as_bytes()), 0);
    assert_eq!(c.marken(), 0);
    // a path in the rules without the switch: no further line
    c.senden(&aufruf("POST", "/chat/completions", b"{}"));
    assert_eq!(c.bis_schluss().last().expect("end")["ok"], json!(true));
    assert_eq!(d.antwortform_zahl(), 1);
    // the same rule again: a second line, again without the value
    c.senden(&aufruf("POST", "/auto", b"{}"));
    assert_eq!(c.bis_schluss().last().expect("end")["ok"], json!(true));
    assert_eq!(d.antwortform_zahl(), 2);
    assert_eq!(d.antwortform_letzte().as_deref(), Some(zeile.as_str()));
}

/// Without the switch there is no diagnosis: the same answer on the same path gives no line, in every build.
#[test]
fn ohne_schalter_keine_antwortform() {
    let koerper = br#"{"access_token":"x","data":[{"id":"y"}]}"#;
    let a = attrappe(vec![vec![http("200 OK", "", koerper)]]);
    let heim = heim_mit(&[("anmeldung.copilot", MARKE)]);
    let d = dienst_erststart(a.port, &heim, &["POST /auto"], None);
    let mut c = verbinden(&d);
    c.senden(&aufruf("POST", "/auto", b"{}"));
    let z = c.bis_schluss();
    assert_eq!(z.last().expect("end")["ok"], json!(true));
    assert_eq!(teil_bytes(&z), koerper);
    assert_eq!(d.antwortform_zahl(), 0);
    assert_eq!(d.antwortform_letzte(), None);
}

#[test]
fn aufruf_traegt_schluessel_und_identity_und_gibt_alles_weiter() {
    // L9 (c): a body of 40 bytes arrives whole, then `ende` without a code
    let koerper = [b'k'; 40];
    let a = attrappe(vec![vec![http(
        "200 OK",
        "content-type: text/event-stream\r\nset-cookie: a=b\r\n",
        &koerper,
    )]]);
    let heim = heim_mit(&[("anmeldung.copilot", MARKE)]);
    let d = dienst("bruecke", a.port, &heim, START);
    let mut c = verbinden(&d);
    c.senden(&aufruf("POST", "/chat/completions", b"{\"x\":1}"));
    let z = c.bis_schluss();
    assert_eq!(z[0]["art"], "beginn");
    assert_eq!(z[0]["status"], 200);
    assert_eq!(z[0]["kopf"]["content-type"], "text/event-stream");
    assert!(z[0]["kopf"].get("set-cookie").is_none());
    assert_eq!(teil_bytes(&z), koerper);
    assert_eq!(z.last().expect("end")["ok"], json!(true));
    let anfrage = String::from_utf8(a.anfragen.lock().expect("lock")[0].clone()).expect("utf-8");
    assert!(anfrage.starts_with("POST /chat/completions HTTP/1.1\r\n"));
    assert!(
        anfrage.contains(&format!("authorization: Bearer {MARKE}\r\n")),
        "the target got the key"
    );
    assert!(anfrage.contains("accept-encoding: identity\r\n"));
    assert!(
        !anfrage
            .to_ascii_lowercase()
            .contains("sec-websocket-extensions")
    );
    assert!(anfrage.ends_with("{\"x\":1}"));
    assert_eq!(c.marken(), 0);
}

#[test]
fn w3_leck_im_koerper_im_kopf_und_ueber_teilgrenzen() {
    let marke = MARKE.as_bytes();
    let (vorne, hinten) = marke.split_at(7);
    let mut geteilt1 = b"HTTP/1.1 200 OK\r\ntransfer-encoding: chunked\r\n\r\n".to_vec();
    geteilt1.extend_from_slice(format!("{:x}\r\n", 9 + vorne.len()).as_bytes());
    geteilt1.extend_from_slice(b"daten vor");
    geteilt1.extend_from_slice(vorne);
    geteilt1.extend_from_slice(b"\r\n");
    let mut geteilt2 = format!("{:x}\r\n", hinten.len() + 4).into_bytes();
    geteilt2.extend_from_slice(hinten);
    geteilt2.extend_from_slice(b" end\r\n0\r\n\r\n");
    let a = attrappe(vec![
        vec![http(
            "200 OK",
            "",
            format!("fehler: token {MARKE} ungueltig").as_bytes(),
        )],
        vec![http(
            "401 Unauthorized",
            &format!("x-echo: Bearer {MARKE}\r\n"),
            b"",
        )],
        vec![geteilt1, geteilt2],
    ]);
    let heim = heim_mit(&[("anmeldung.copilot", MARKE)]);
    let d = dienst("bruecke", a.port, &heim, START);
    let mut c = verbinden(&d);
    c.senden(&aufruf("GET", "/copilot_internal/user", b""));
    let z = c.bis_schluss();
    assert_eq!(z.last().expect("end")["art"], "ende");
    assert_eq!(letzte_code(&z).as_deref(), Some("halter.leck_gesperrt"));
    c.senden(&aufruf("GET", "/copilot_internal/user", b""));
    let z = c.bis_schluss();
    assert_eq!(z.len(), 1, "no beginn with the key in a head");
    assert_eq!(letzte_code(&z).as_deref(), Some("halter.leck_gesperrt"));
    c.senden(&aufruf("GET", "/copilot_internal/user", b""));
    let z = c.bis_schluss();
    assert_eq!(letzte_code(&z).as_deref(), Some("halter.leck_gesperrt"));
    assert_eq!(c.marken(), 0, "no mark in any byte to the bridge");
    assert_eq!(zaehlen(&teil_bytes(&z), marke), 0);
    assert_eq!(
        a.anfragen.lock().expect("lock").len(),
        3,
        "the target got every call"
    );
    let k = kette_text(&heim);
    assert_eq!(zaehlen(k.as_bytes(), marke), 0, "no value in the chain");
    assert_eq!(d.leck_treffer(), 3, "one journal line per hit");
    // F4: one chain line per hit, with the op and the target only
    leck_zeilen_pruefen(&heim, 3, "modell.aufruf", "copilot");
}

/// The chain of `heim` holds exactly `n` lines `halter.leck_gesperrt`, each with the fields of the contract (section 9),
/// `op` and `ziel` as given, no mark of the value, and the chain is whole for `formate` (the kind is in its register).
fn leck_zeilen_pruefen(heim: &Path, n: usize, op: &str, ziel: &str) {
    let k = kette_text(heim);
    assert_eq!(
        zaehlen(k.as_bytes(), MARKE.as_bytes()),
        0,
        "no value in the chain"
    );
    let zeilen: Vec<Value> = k
        .lines()
        .map(|z| serde_json::from_str::<Value>(z).expect("line"))
        .filter(|z| z["aktionsname"] == "halter.leck_gesperrt")
        .collect();
    assert_eq!(zeilen.len(), n, "one chain line per hit");
    for z in &zeilen {
        let mut felder: Vec<&str> = z
            .as_object()
            .expect("object")
            .keys()
            .map(String::as_str)
            .collect();
        felder.sort_unstable();
        assert_eq!(
            felder,
            [
                "aktionsname",
                "ergebnis",
                "fassung",
                "grund",
                "op",
                "vorher",
                "zeit",
                "ziel"
            ]
        );
        assert_eq!(z["ergebnis"], "abgelehnt");
        assert_eq!(z["grund"], "leck");
        assert_eq!(z["op"], op);
        assert_eq!(z["ziel"], ziel);
    }
    let p = formate::kette::pruefe_quelle(
        &heim.join("kette"),
        "halter",
        formate::kette::NULL_HASH,
        None,
    )
    .expect("source");
    assert!(p.ok(), "{:?}", p.bruch);
}

fn prozent(bytes: &[u8], gross: bool) -> String {
    bytes
        .iter()
        .map(|b| {
            if gross {
                format!("%{b:02X}")
            } else {
                format!("%{b:02x}")
            }
        })
        .collect()
}

#[test]
fn w3_b_kodierte_rueckgabe_wird_gesperrt_harmloses_geht_durch() {
    let mit = b64::kodieren(format!("x{MARKE}").as_bytes());
    let ohne_url = mit
        .trim_end_matches('=')
        .replace('+', "-")
        .replace('/', "_");
    let mut geteilt1 = b"HTTP/1.1 200 OK\r\ntransfer-encoding: chunked\r\n\r\n".to_vec();
    let p = prozent(MARKE.as_bytes(), false);
    let (vorne, hinten) = p.split_at(20);
    geteilt1.extend_from_slice(format!("{:x}\r\n{vorne}\r\n", vorne.len()).as_bytes());
    let geteilt2 = format!("{:x}\r\n{hinten}\r\n0\r\n\r\n", hinten.len()).into_bytes();
    let harmlos = b"kein Schluessel hier, 100% harmlos, aGd4MS1n".to_vec();
    let a = attrappe(vec![
        vec![http(
            "200 OK",
            "",
            format!("{{\"t\":\"{mit}\"}}").as_bytes(),
        )],
        vec![http("200 OK", &format!("x-echo: {ohne_url}\r\n"), b"")],
        vec![http(
            "200 OK",
            &format!("x-echo: {}\r\n", prozent(MARKE.as_bytes(), true)),
            b"",
        )],
        vec![geteilt1, geteilt2],
        vec![http("200 OK", "x-echo: harmlos\r\n", &harmlos)],
    ]);
    let heim = heim_mit(&[("anmeldung.copilot", MARKE)]);
    let d = dienst("bruecke", a.port, &heim, START);
    let mut c = verbinden(&d);
    for fall in [
        "base64 im koerper",
        "base64url im kopf",
        "prozent im kopf",
        "prozent ueber teile",
    ] {
        c.senden(&aufruf("GET", "/copilot_internal/user", b""));
        let z = c.bis_schluss();
        assert_eq!(
            letzte_code(&z).as_deref(),
            Some("halter.leck_gesperrt"),
            "{fall}"
        );
        let t = teil_bytes(&z);
        assert_eq!(zaehlen(&t, &mit.as_bytes()[..20]), 0, "{fall}");
        assert_eq!(
            zaehlen(&t, vorne.as_bytes()),
            0,
            "the first part stayed back: {fall}"
        );
    }
    c.senden(&aufruf("GET", "/copilot_internal/user", b""));
    let z = c.bis_schluss();
    assert_eq!(
        z.last().expect("end")["ok"],
        json!(true),
        "harmless text goes through"
    );
    assert_eq!(teil_bytes(&z), harmlos);
    assert_eq!(d.leck_treffer(), 4, "the harmless answer is no hit");
}

#[test]
fn l7_a_komprimiert_geht_nicht_durch() {
    let a = attrappe(vec![vec![http(
        "200 OK",
        "content-encoding: gzip\r\n",
        b"\x1f\x8b\x08\x00abcdef",
    )]]);
    let heim = heim_mit(&[("anmeldung.copilot", MARKE)]);
    let d = dienst("bruecke", a.port, &heim, START);
    let mut c = verbinden(&d);
    c.senden(&aufruf("GET", "/copilot_internal/user", b""));
    let z = c.bis_schluss();
    assert_eq!(z.len(), 1);
    assert_eq!(letzte_code(&z).as_deref(), Some("halter.ziel_komprimiert"));
    assert_eq!(teil_bytes(&z), Vec::<u8>::new());
}

#[test]
fn w7_rolle_trennt_die_ops() {
    let a = attrappe(vec![]);
    let heim = heim_mit(&[("anmeldung.copilot", MARKE), ("mcp.tickets", MARKE)]);
    let d = dienst("hub", a.port, &heim, START);
    let mut c = verbinden(&d);
    c.senden(&aufruf("GET", "/copilot_internal/user", b""));
    assert_eq!(
        letzte_code(&c.bis_schluss()).as_deref(),
        Some("halter.rolle")
    );
    c.senden(&json!({"fassung": 1, "id": "s-1", "nachweis": "nw", "name": "mcp.tickets", "op": "schluessel.setzen",
        "wert_b64": "YmVpc3BpZWx3ZXJ0"}));
    assert_eq!(
        letzte_code(&c.bis_schluss()).as_deref(),
        Some("halter.rolle")
    );
    let d = dienst("bruecke", a.port, &heim, START);
    let mut c = verbinden(&d);
    c.senden(
        &json!({"argumente": {}, "fassung": 1, "id": 7, "lauf": "l-beispiel", "op": "mcp.aufruf",
        "person": PERSON, "server": "tickets", "vorschau_sha256": null, "werkzeug": "get"}),
    );
    assert_eq!(
        letzte_code(&c.bis_schluss()).as_deref(),
        Some("halter.rolle")
    );
    assert_eq!(
        a.anfragen.lock().expect("lock").len(),
        0,
        "the stand-in counts 0"
    );
}

/// L3 (addendum "Rolle aus der Gruppe"): the role comes from the effective gid of the peer only. The same peer (this
/// process) gets `halter.peer` when its gid is not the gid of a role group, and the role when it is (counter-check);
/// a peer of another uid gets no role with the right gid.
#[test]
fn l3_rolle_nur_mit_der_gid_der_rollengruppe() {
    let heim = heim_mit(&[("anmeldung.copilot", MARKE)]);
    let k = json!({"fassung": 1, "konten": {uid().to_string(): {"person": null}},
        "rollen": {"bruecke": "wb-bruecke"}, "ziele": {}, "server": {}, "kids": {}});
    let mit = |gruppen: BTreeMap<u32, Rolle>, konto: u32| {
        Arc::new(Dienst::neu(
            konfig::lesen(k.to_string().as_bytes()).expect("konfig"),
            Ablage::neu(&heim, uid()),
            konto,
            START,
            gruppen,
            None,
            uhr,
        ))
    };
    let frage = json!({"fassung": 1, "id": "q-1", "name": "anmeldung.copilot", "op": "schluessel.vorhanden"});
    // another gid: no role, the connection is closed
    let mut c = verbinden(&mit(
        BTreeMap::from([(egid().wrapping_add(1), Rolle::Bruecke)]),
        uid(),
    ));
    c.senden(&frage);
    let z = c.bis_schluss();
    assert_eq!(letzte_code(&z).as_deref(), Some("halter.peer"));
    assert_eq!(z[0]["id"], Value::Null);
    c.strom
        .set_read_timeout(Some(Duration::from_secs(5)))
        .expect("timeout");
    let mut rest = [0u8; 16];
    assert_eq!(
        c.strom
            .read(&mut rest)
            .expect("the holder closes after halter.peer, no timeout"),
        0
    );
    // no role group at all
    let mut c = verbinden(&mit(BTreeMap::new(), uid()));
    c.senden(&frage);
    assert_eq!(
        letzte_code(&c.bis_schluss()).as_deref(),
        Some("halter.peer")
    );
    // counter-check: the same peer with the gid of the role group gets the role
    let mut c = verbinden(&mit(eigene_gid_als("bruecke"), uid()));
    c.senden(&frage);
    assert_eq!(c.bis_schluss()[0]["vorhanden"], json!(true));
    // the right gid, but the holder runs under another uid: the peer is no account of the holder
    let mut c = verbinden(&mit(eigene_gid_als("bruecke"), uid().wrapping_add(1)));
    c.senden(&frage);
    assert_eq!(
        letzte_code(&c.bis_schluss()).as_deref(),
        Some("halter.peer")
    );
}

/// Case and counter-check of L3 across processes, without root: a client program (the interpreter of the system) asks
/// `schluessel.vorhanden` once as it is and once started through `newgrp` on a supplementary group of the test user, so
/// that its effective gid is that group. Only the second gets the role. Skipped (with a line on stderr) when the
/// machine has no interpreter, no `newgrp`, the user no supplementary group, or `newgrp` does not change the gid (a
/// process under `no_new_privs`, as inside an enclosure).
#[test]
#[allow(
    clippy::print_stderr,
    clippy::too_many_lines,
    reason = "a skip and the measured gids are named on stderr; one test holds case and counter-check"
)]
fn l3_gleicher_peer_mit_und_ohne_rollengruppe() {
    let Ok(py) = std::fs::canonicalize("/usr/bin/python3") else {
        eprintln!("skipped: no /usr/bin/python3");
        return;
    };
    let Some(newgrp) = ["/usr/bin/newgrp", "/bin/newgrp"]
        .into_iter()
        .map(PathBuf::from)
        .find(|p| p.exists())
    else {
        eprintln!("skipped: no newgrp");
        return;
    };
    let eigene = egid();
    let Some(gruppe) = rustix::process::getgroups()
        .unwrap_or_default()
        .into_iter()
        .map(rustix::process::Gid::as_raw)
        .find(|g| *g != eigene && *g != 0)
    else {
        eprintln!("skipped: the test user has no supplementary group");
        return;
    };
    let name = std::fs::read_to_string("/etc/group")
        .unwrap_or_default()
        .lines()
        .find_map(|z| {
            let f: Vec<&str> = z.split(':').collect();
            (f.len() == 4 && f[2].parse::<u32>().ok() == Some(gruppe)).then(|| f[0].to_owned())
        });
    let Some(name) = name else {
        eprintln!("skipped: the supplementary group {gruppe} has no name in /etc/group");
        return;
    };
    let heim = heim_mit(&[("anmeldung.copilot", MARKE)]);
    let k = json!({"fassung": 1, "konten": {uid().to_string(): {"person": null}},
        "rollen": {"bruecke": "wb-bruecke"}, "ziele": {}, "server": {}, "kids": {}});
    let d = Arc::new(Dienst::neu(
        konfig::lesen(k.to_string().as_bytes()).expect("konfig"),
        Ablage::neu(&heim, uid()),
        uid(),
        START,
        BTreeMap::from([(gruppe, Rolle::Bruecke)]),
        None,
        uhr,
    ));
    let o = ordner();
    let sock = o.join("l3.sock");
    let hoerer = std::os::unix::net::UnixListener::bind(&sock).expect("bind");
    let skript = o.join("client.py");
    std::fs::write(
        &skript,
        "import os, socket, sys\n\
         s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)\ns.connect(sys.argv[1])\n\
         s.sendall(b'{\"fassung\":1,\"id\":\"q-1\",\"name\":\"anmeldung.copilot\",\"op\":\"schluessel.vorhanden\"}\\n')\n\
         b = b''\nwhile not b.endswith(b'\\n'):\n    t = s.recv(4096)\n    if not t: break\n    b += t\n\
         open(sys.argv[2], 'w').write(str(os.getegid()) + ' ' + b.decode())\n",
    )
    .expect("script");
    let lauf = |mit_gruppe: bool, aus: &Path| {
        let d = Arc::clone(&d);
        let h = hoerer.try_clone().expect("listener");
        let server = std::thread::spawn(move || {
            let (s, _) = h.accept().expect("accept");
            d.verbindung(s);
        });
        let mut kind = if mit_gruppe {
            let mut k = std::process::Command::new(&newgrp)
                .arg(&name)
                .stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn()
                .expect("newgrp");
            let zeile = format!(
                "exec {} -I {} {} {}\n",
                py.display(),
                skript.display(),
                sock.display(),
                aus.display()
            );
            k.stdin
                .take()
                .expect("stdin")
                .write_all(zeile.as_bytes())
                .expect("write");
            k
        } else {
            std::process::Command::new(&py)
                .arg("-I")
                .arg(&skript)
                .arg(&sock)
                .arg(aus)
                .env_clear()
                .spawn()
                .expect("python")
        };
        let st = kind.wait().expect("wait");
        if !st.success() {
            // the client never connected; let the server thread go
            let _ = UnixStream::connect(&sock);
        }
        server.join().expect("server");
        std::fs::read_to_string(aus).unwrap_or_default()
    };
    let ohne = lauf(false, &o.join("ohne.txt"));
    let mit = lauf(true, &o.join("mit.txt"));
    let gid_von = |t: &str| t.split(' ').next().and_then(|g| g.parse::<u32>().ok());
    if gid_von(&mit) != Some(gruppe) {
        eprintln!("skipped: newgrp did not change the effective gid ({mit:?})");
        return;
    }
    assert_eq!(gid_von(&ohne), Some(eigene));
    assert!(
        ohne.contains("\"code\":\"halter.peer\""),
        "without the role group: {ohne}"
    );
    assert!(
        mit.contains("\"ok\":true,\"vorhanden\":true"),
        "with the role group as effective gid: {mit}"
    );
    eprintln!("measured: gid {eigene} -> halter.peer, gid {gruppe} ({name}) -> role bruecke");
}

/// One stream: the lines to the bridge, the marks in their bytes, the stand-in and the hits of the leak lock.
fn strom_lauf(nach_101: Vec<Vec<u8>>) -> (Vec<Value>, usize, Attrappe, u64) {
    let mut teile = vec![ACHT_ZEHN_EINS.to_vec()];
    teile.extend(nach_101);
    let a = attrappe(vec![teile]);
    let heim = heim_mit(&[("anmeldung.copilot", MARKE)]);
    let d = dienst("bruecke", a.port, &heim, START);
    let mut c = verbinden(&d);
    c.senden(&strom_anfrage());
    let z = c.bis_schluss();
    let m = c.marken();
    let treffer = d.leck_treffer();
    leck_zeilen_pruefen(
        &heim,
        usize::try_from(treffer).expect("count"),
        "modell.strom",
        "copilot",
    );
    (z, m, a, treffer)
}

#[test]
fn l6_leck_im_strom_ueber_zwei_rahmen_mit_ping() {
    let (vorne, hinten) = MARKE.as_bytes().split_at(9);
    let (z, m, _a, t) = strom_lauf(vec![
        rahmen(false, 0x1, vorne),
        rahmen(true, 0x9, b"p"),
        rahmen(true, 0x0, hinten),
    ]);
    assert_eq!(z[0]["status"], 101);
    assert!(z[0]["kopf"].get("upgrade").is_none() && z[0]["kopf"].get("connection").is_none());
    assert_eq!(letzte_code(&z).as_deref(), Some("halter.leck_gesperrt"));
    assert_eq!(m, 0);
    assert_eq!(t, 1, "one journal line per hit");
    let (z, m, _a, t) = strom_lauf(vec![rahmen(true, 0x1, format!("x {MARKE} y").as_bytes())]);
    assert_eq!(letzte_code(&z).as_deref(), Some("halter.leck_gesperrt"));
    assert_eq!(m, 0);
    assert_eq!(t, 1);
    let kodiert = b64::kodieren(MARKE.as_bytes());
    let (z, _m, _a, t) = strom_lauf(vec![rahmen(true, 0x1, kodiert.as_bytes())]);
    assert_eq!(
        letzte_code(&z).as_deref(),
        Some("halter.leck_gesperrt"),
        "Base64 in a frame"
    );
    assert_eq!(t, 1);
}

#[test]
fn l7_b_und_c_strom_komprimiert() {
    let a = attrappe(vec![vec![
        b"HTTP/1.1 101 Switching Protocols\r\nupgrade: websocket\r\nconnection: Upgrade\r\nSec-WebSocket-Extensions: permessage-deflate\r\n\r\n".to_vec(),
        rahmen(true, 0x1, b"abc"),
    ]]);
    let heim = heim_mit(&[("anmeldung.copilot", MARKE)]);
    let d = dienst("bruecke", a.port, &heim, START);
    let mut c = verbinden(&d);
    c.senden(&strom_anfrage());
    let z = c.bis_schluss();
    assert_eq!(z.len(), 1, "no 101 goes out");
    assert_eq!(letzte_code(&z).as_deref(), Some("halter.ziel_komprimiert"));
    let anfrage = String::from_utf8(a.anfragen.lock().expect("lock")[0].clone()).expect("utf-8");
    assert!(
        anfrage.contains("accept-encoding: identity\r\n")
            && anfrage.contains("upgrade: websocket\r\n")
    );
    assert!(!anfrage.to_ascii_lowercase().contains("extensions"));
    let mut rsv = rahmen(true, 0x1, b"abcdef");
    rsv[0] |= 0x40;
    let (z, _, _a, _) = strom_lauf(vec![rsv]);
    assert_eq!(letzte_code(&z).as_deref(), Some("halter.ziel_komprimiert"));
    assert!(teil_bytes(&z).is_empty(), "0 bytes of the frame");
}

#[test]
fn l8_rahmenregeln() {
    let verstoesse = [
        rahmen(false, 0x9, b"x"),
        rahmen(true, 0xA, &[0u8; 126]),
        rahmen(true, 0x8, &[0u8; 126]),
        rahmen(true, 0x0, b"x"),
        [rahmen(false, 0x1, b"a"), rahmen(true, 0x1, b"b")].concat(),
    ];
    for (i, v) in verstoesse.into_iter().enumerate() {
        let vorher = rahmen(true, 0x2, b"gut");
        let gesendet = [vorher.clone(), v.clone()].concat();
        let (z, _, a, t) = strom_lauf(vec![vorher.clone(), v]);
        assert_eq!(
            letzte_code(&z).as_deref(),
            Some("halter.ziel_fehler"),
            "case {i}"
        );
        assert_eq!(t, 0, "case {i}: a break is no hit of the leak lock");
        let bytes = teil_bytes(&z);
        // the payload "gut" is still kept back by the leak lock and is dropped with the break; at most the head of
        // the frame before the violation went out
        assert!(
            gesendet.starts_with(&bytes) && bytes.len() <= vorher.len() + 2,
            "case {i}: nothing after the break"
        );
        // the holder closed towards the stand-in: the stand-in saw the end
        for _ in 0..100 {
            if !a.danach.lock().expect("lock").is_empty() {
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        assert_eq!(
            a.danach.lock().expect("lock").len(),
            1,
            "case {i}: connection to the stand-in closed"
        );
    }
    let f = [
        rahmen(false, 0x1, b"abc"),
        rahmen(true, 0x9, b"12345"),
        rahmen(true, 0x0, b"def"),
    ]
    .concat();
    let (z, _, _a, _) = strom_lauf(vec![f.clone()]);
    assert_eq!(z.last().expect("end")["ok"], json!(true));
    assert_eq!(teil_bytes(&z), f);
    let g = rahmen(true, 0x9, &[7u8; 125]);
    let (z, _, _a, _) = strom_lauf(vec![g.clone()]);
    assert_eq!(teil_bytes(&z), g);
}

#[test]
fn l9_zurueckgehaltenes_geht_am_ende_hinaus() {
    let r = rahmen(true, 0x1, &[b'n'; 40]);
    let (z, _, _a, _) = strom_lauf(vec![r.clone()]);
    assert_eq!(teil_bytes(&z), r);
    assert_eq!(z.last().expect("end")["ok"], json!(true));
    let mut b = r.clone();
    b.extend_from_slice(&rahmen(true, 0x1, &[b'o'; 30])[..12]);
    let (z, _, _a, _) = strom_lauf(vec![b.clone()]);
    assert_eq!(teil_bytes(&z), b, "everything the stand-in sent");
    assert!(z.last().expect("end").get("code").is_none());
}

#[test]
fn strom_teil_und_ende_dann_wieder_anfragen() {
    let a = attrappe_mit(
        vec![vec![ACHT_ZEHN_EINS.to_vec(), rahmen(true, 0x1, b"hallo")]],
        false,
    );
    let heim = heim_mit(&[("anmeldung.copilot", MARKE)]);
    let d = dienst("bruecke", a.port, &heim, START);
    let mut c = verbinden(&d);
    c.senden(&strom_anfrage());
    // beginn and the first part
    let b = c.eine().expect("beginn");
    assert_eq!(b["status"], 101);
    let t = c.eine().expect("teil");
    assert_eq!(t["art"], "teil");
    // another op while the stream is open: halter.strom
    c.senden(&json!({"fassung": 1, "id": "q-1", "name": "anmeldung.copilot", "op": "schluessel.vorhanden"}));
    assert_eq!(c.eine().expect("refusal")["code"], "halter.strom");
    let harness = [
        0x81u8,
        0x85,
        1,
        2,
        3,
        4,
        b'a' ^ 1,
        b'b' ^ 2,
        b'c' ^ 3,
        b'd' ^ 4,
        b'e' ^ 1,
    ];
    c.senden(&json!({"daten_b64": b64::kodieren(&harness), "fassung": 1, "id": "m-2", "op": "strom.teil"}));
    c.senden(&json!({"fassung": 1, "id": "m-2", "op": "strom.ende"}));
    let z = c.bis_schluss();
    assert_eq!(z.last().expect("end")["ok"], json!(true));
    c.senden(&json!({"fassung": 1, "id": "q-2", "name": "anmeldung.copilot", "op": "schluessel.vorhanden"}));
    assert_eq!(c.bis_schluss()[0]["vorhanden"], json!(true));
    drop(c);
    for _ in 0..100 {
        if !a.danach.lock().expect("lock").is_empty() {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    assert_eq!(
        a.danach.lock().expect("lock")[0],
        harness,
        "the bytes of the harness arrive unchanged"
    );
}

const SCHEIN: &str = r#"{"daten":{"ablauf":1795000030000,"anfrage":"mf_0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d","art":"mcp-schein","ausgestellt":1794999990000,"digest":"ffc2016a50770971ab9be5aac167e9165ac7a0a189d6e5f7c9fac57af4acb258","einmal":"5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e","fassung":1,"kid":"stelle-mcp-1","lauf":"l-beispiel"},"signatur":"G7BKnSp/7wiP0ydW6cycjMf96T3EtW05zH1Rs8wexOQiD5QO1LdIuk31jKXY/pWMRyj4bat0H+907SiaxUx9Cg=="}"#;

fn schreibaufruf(status: &str) -> Value {
    json!({"argumente": {"id": "t-1", "status": status}, "fassung": 1, "id": 7, "lauf": "l-beispiel",
        "op": "mcp.aufruf", "person": PERSON, "schein": serde_json::from_str::<Value>(SCHEIN).expect("ticket"),
        "server": "tickets", "vorschau_sha256": "c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3",
        "werkzeug": "update"})
}

fn mcp_antwort() -> Vec<Vec<u8>> {
    vec![http(
        "200 OK",
        "content-type: application/json\r\n",
        br#"{"id":1,"jsonrpc":"2.0","result":{"content":[{"text":"ok","type":"text"}]}}"#,
    )]
}

fn kette_text(heim: &Path) -> String {
    let mut t = String::new();
    for e in std::fs::read_dir(heim.join("kette")).expect("chain") {
        let p = e.expect("entry").path();
        if p.file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.starts_with("halter-"))
        {
            t.push_str(&std::fs::read_to_string(p).expect("read"));
        }
    }
    t
}

#[test]
fn w4_einmal_einloesung() {
    // (a) the same ticket twice: ok, then benutzt; (d) a changed argument: digest
    let a = attrappe(vec![mcp_antwort(), mcp_antwort()]);
    let heim = heim_mit(&[("mcp.tickets", MARKE)]);
    let d = dienst("hub", a.port, &heim, START);
    let mut c = verbinden(&d);
    c.senden(&schreibaufruf("erledigt"));
    let z = c.bis_schluss();
    assert_eq!(z[0]["ok"], json!(true), "{z:?}");
    assert_eq!(z[0]["ergebnis"]["content"][0]["text"], "ok");
    c.senden(&schreibaufruf("erledigt"));
    assert_eq!(
        letzte_code(&c.bis_schluss()).as_deref(),
        Some("halter.schein_benutzt")
    );
    c.senden(&schreibaufruf("offen"));
    assert_eq!(
        letzte_code(&c.bis_schluss()).as_deref(),
        Some("halter.schein_digest")
    );
    assert_eq!(
        a.anfragen.lock().expect("lock").len(),
        1,
        "the stand-in counts 1"
    );
    let anfrage = String::from_utf8(a.anfragen.lock().expect("lock")[0].clone()).expect("utf-8");
    assert!(anfrage.contains(&format!("authorization: Bearer {MARKE}\r\n")));
    assert!(anfrage.contains(r#""method":"tools/call""#));
    let k = kette_text(&heim);
    assert!(
        k.contains(r#""grund":"benutzt""#) && k.contains(r#""grund":"digest""#),
        "{k}"
    );
    assert!(!k.contains(MARKE) && !k.contains(PERSON) && !k.contains("t-1"));
    assert_eq!(c.marken(), 0);
}

#[test]
fn w4_b_acht_verbindungen_gleichzeitig() {
    let a = attrappe((0..8).map(|_| mcp_antwort()).collect());
    let heim = heim_mit(&[("mcp.tickets", MARKE)]);
    let d = dienst("hub", a.port, &heim, START);
    let faeden: Vec<_> = (0..8)
        .map(|_| {
            let d = Arc::clone(&d);
            std::thread::spawn(move || {
                let mut c = verbinden(&d);
                c.senden(&schreibaufruf("erledigt"));
                c.bis_schluss()
            })
        })
        .collect();
    let ergebnisse: Vec<Vec<Value>> = faeden
        .into_iter()
        .map(|f| f.join().expect("thread"))
        .collect();
    let ok = ergebnisse
        .iter()
        .filter(|z| z[0]["ok"] == json!(true))
        .count();
    let benutzt = ergebnisse
        .iter()
        .filter(|z| letzte_code(z).as_deref() == Some("halter.schein_benutzt"))
        .count();
    assert_eq!((ok, benutzt), (1, 7));
    std::thread::sleep(Duration::from_millis(200));
    assert_eq!(
        a.anfragen.lock().expect("lock").len(),
        1,
        "exactly one call"
    );
}

#[test]
fn w4_c_nach_neustart_abgelaufen() {
    let a = attrappe(vec![mcp_antwort()]);
    let heim = heim_mit(&[("mcp.tickets", MARKE)]);
    // a holder started after the ticket was issued
    let d = dienst("hub", a.port, &heim, 1_794_999_995_000);
    let mut c = verbinden(&d);
    c.senden(&schreibaufruf("erledigt"));
    assert_eq!(
        letzte_code(&c.bis_schluss()).as_deref(),
        Some("halter.schein_abgelaufen")
    );
    assert_eq!(a.anfragen.lock().expect("lock").len(), 0);
    assert!(kette_text(&heim).contains(r#""grund":"abgelaufen""#));
}

#[test]
fn w8_fremde_person_bekommt_keinen_token() {
    let a = attrappe(vec![]);
    let heim = heim_mit(&[("mcp.tickets", MARKE)]);
    let d = dienst("hub", a.port, &heim, START);
    let mut c = verbinden(&d);
    let mut v = schreibaufruf("erledigt");
    v["person"] = json!("ps_b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2");
    c.senden(&v);
    assert_eq!(
        letzte_code(&c.bis_schluss()).as_deref(),
        Some("halter.kein_schluessel")
    );
    assert_eq!(a.anfragen.lock().expect("lock").len(), 0);
}

#[test]
fn mcp_leck_im_ergebnis_wird_gesperrt_und_gezaehlt() {
    let a = attrappe(vec![
        vec![http(
            "200 OK",
            "content-type: application/json\r\n",
            format!(r#"{{"id":1,"jsonrpc":"2.0","result":{{"content":[{{"text":"{}","type":"text"}}]}}}}"#,
                b64::kodieren(MARKE.as_bytes())).as_bytes(),
        )],
        vec![http("500 Internal Server Error", "", b"")],
    ]);
    let heim = heim_mit(&[("mcp.tickets", MARKE)]);
    let d = dienst("hub", a.port, &heim, START);
    let mut c = verbinden(&d);
    let lesend = json!({"argumente": {"id": "t-1"}, "fassung": 1, "id": 8, "lauf": "l-beispiel", "op": "mcp.aufruf",
        "person": PERSON, "server": "tickets", "vorschau_sha256": null, "werkzeug": "get"});
    c.senden(&lesend);
    assert_eq!(
        letzte_code(&c.bis_schluss()).as_deref(),
        Some("halter.leck_gesperrt")
    );
    assert_eq!(d.leck_treffer(), 1);
    c.senden(&lesend);
    assert_eq!(
        letzte_code(&c.bis_schluss()).as_deref(),
        Some("halter.ziel_fehler")
    );
    assert_eq!(d.leck_treffer(), 1, "another error is no hit");
    leck_zeilen_pruefen(&heim, 1, "mcp.aufruf", "tickets");
}

#[test]
fn mcp_fehler_nur_als_zahl() {
    let a = attrappe(vec![vec![http(
        "200 OK",
        "content-type: application/json\r\n",
        format!(r#"{{"error":{{"code":-32603,"message":"token {MARKE} ungueltig"}},"id":1,"jsonrpc":"2.0"}}"#).as_bytes(),
    )]]);
    let heim = heim_mit(&[("mcp.tickets", MARKE)]);
    let d = dienst("hub", a.port, &heim, START);
    let mut c = verbinden(&d);
    c.senden(&json!({"argumente": {"id": "t-1"}, "fassung": 1, "id": 8, "lauf": "l-beispiel", "op": "mcp.aufruf",
        "person": PERSON, "server": "tickets", "vorschau_sha256": null, "werkzeug": "get"}));
    let z = c.bis_schluss();
    assert_eq!(z[0]["code"], "halter.mcp_fehler");
    assert_eq!(z[0]["mcp_code"], -32603);
    assert_eq!(c.marken(), 0);
}

#[test]
fn suche_zaehlt_nur_zahlen_und_folgt_keinem_verweis() {
    let heim = heim_mit(&[("anmeldung.copilot", MARKE)]);
    let arbeit = ordner();
    std::fs::write(arbeit.join("a.txt"), format!("x {MARKE} y {MARKE}")).expect("write");
    std::fs::create_dir(arbeit.join("unter")).expect("dir");
    std::fs::write(arbeit.join("unter/b.txt"), "nichts").expect("write");
    let datei = heim.join(".local/state/agent-workbench/halter/geheim/anmeldung.copilot");
    std::os::unix::fs::symlink(&datei, arbeit.join("verweis")).expect("link");
    let d = dienst("bruecke", 9, &heim, START);
    let mut c = verbinden(&d);
    c.senden(&json!({"fassung": 1, "id": "u-1", "op": "modell.suche", "pfade": [arbeit.to_str().expect("utf-8")],
        "ziel": "copilot"}));
    let z = c.bis_schluss();
    assert_eq!(
        z[0]["suche"],
        json!({"dateien": 2, "nicht_lesbar": 0, "treffer": 2, "uebersprungen": 1})
    );
    assert_eq!(c.marken(), 0);
}

/// `Peer::lebt` through the pidfd: a peer program (the interpreter of the system, if there is one) connects, is
/// measured, then ends; the bound process is gone and no new one takes its place.
#[test]
fn peer_lebt_nur_solange_der_prozess_laeuft() {
    let Ok(py) = std::fs::canonicalize("/usr/bin/python3") else {
        return;
    };
    let o = ordner();
    let pfad = o.join("p.sock");
    let hoerer = std::os::unix::net::UnixListener::bind(&pfad).expect("bind");
    let mut kind = std::process::Command::new(&py)
        .arg("-I")
        .arg("-c")
        .arg("import socket,sys\ns=socket.socket(socket.AF_UNIX)\ns.connect(sys.argv[1])\nsys.stdin.read()")
        .arg(&pfad)
        .stdin(std::process::Stdio::piped())
        .env_clear()
        .spawn()
        .expect("python");
    let (strom, _) = hoerer.accept().expect("accept");
    let p = halter::peer::messen(&strom).expect("peer");
    assert_eq!(p.pid, kind.id());
    assert_eq!(p.programm.as_deref(), Some(py.as_path()));
    assert!(p.lebt());
    let gruppen = BTreeMap::from([(p.gid, Rolle::Bruecke)]);
    assert_eq!(
        halter::peer::rolle(&p, uid(), &gruppen),
        Some(Rolle::Bruecke)
    );
    drop(kind.stdin.take());
    kind.wait().expect("wait");
    assert!(!p.lebt(), "the pidfd shows the end of the process");
    assert_eq!(
        halter::peer::rolle(&p, uid(), &gruppen),
        None,
        "an ended peer has no role"
    );
}

#[test]
fn zeile_genau_an_der_grenze_wird_gelesen() {
    let heim = heim_mit(&[]);
    let d = dienst("bruecke", 9, &heim, START);
    let mut c = verbinden(&d);
    let mut z = vec![b'x'; halter::pruefung::ZEILE_MAX];
    z.push(b'\n');
    let s = c.strom.try_clone().expect("clone");
    let schreiber = std::thread::spawn(move || {
        let mut s = s;
        let _ = s.write_all(&z);
    });
    // exactly 32 MiB is no break of stage 1: the line is read and refused as JSON, and the connection stays
    assert_eq!(
        letzte_code(&c.bis_schluss()).as_deref(),
        Some("halter.json_ungueltig")
    );
    let _ = schreiber.join();
    c.senden(&json!({"fassung": 1, "id": "q-1", "name": "mcp.x", "op": "schluessel.vorhanden"}));
    assert_eq!(c.bis_schluss()[0]["vorhanden"], json!(false));
}

#[test]
fn zu_grosse_zeile_schliesst_ohne_rest() {
    let heim = heim_mit(&[]);
    let d = dienst("bruecke", 9, &heim, START);
    let mut c = verbinden(&d);
    let gross = vec![b'x'; halter::pruefung::ZEILE_MAX + 10];
    let s = c.strom.try_clone().expect("clone");
    let schreiber = std::thread::spawn(move || {
        let mut s = s;
        let _ = s.write_all(&gross);
    });
    let z = c.bis_schluss();
    assert_eq!(letzte_code(&z).as_deref(), Some("halter.zu_gross"));
    assert!(c.eine().is_none());
    let _ = schreiber.join();
}

/// A short folder for a socket (a Unix socket path has at most 107 bytes): below `TMPDIR` if that is short (the
/// enclosure sets it), else below `$HOME/.cache`.
fn kurzer_ordner() -> PathBuf {
    let n = ZAEHLER.fetch_add(1, Ordering::Relaxed);
    let basis = std::env::var_os("TMPDIR")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute() && p.as_os_str().len() <= 48)
        .unwrap_or_else(|| PathBuf::from(std::env::var_os("HOME").expect("HOME")).join(".cache"));
    let p = basis.join(format!("hd-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p).expect("short folder");
    std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o700)).expect("mode");
    p
}

/// A client on the socket at `p` that waits at most 3 s for a line.
fn verbinden_mit(p: &Path) -> Client {
    let a = UnixStream::connect(p).expect("connect");
    a.set_read_timeout(Some(Duration::from_secs(3)))
        .expect("timeout");
    Client {
        strom: a,
        vorrat: Vec::new(),
        roh: Vec::new(),
        seit: std::time::Instant::now(),
    }
}

/// `true` if the connection was served: the answer to `schluessel.vorhanden` comes.
fn bedient(c: &mut Client) -> bool {
    // a connection closed unanswered may refuse the request already
    let frage = json!({"fassung": 1, "id": "q-1", "name": "anmeldung.copilot", "op": "schluessel.vorhanden"});
    if c.strom.write_all(&zeile(&frage)).is_err() {
        return false;
    }
    c.bis_schluss()
        .first()
        .is_some_and(|z| z["vorhanden"] == json!(true))
}

/// The accept loop of the program (section 3): it serves a connection, closes one above the limit unanswered, serves
/// again once a place is free and ends after the stop flag (SIGTERM and SIGINT set it in the program).
#[test]
fn annahme_bedient_haelt_die_grenze_und_endet_mit_halt() {
    use std::os::unix::net::UnixListener;
    use std::sync::atomic::AtomicBool;

    let heim = heim_mit(&[("anmeldung.copilot", MARKE)]);
    let d = dienst("bruecke", 9, &heim, START);
    let o = kurzer_ordner();
    let pfad = o.join("h.sock");
    let hoerer = UnixListener::bind(&pfad).expect("bind");
    hoerer.set_nonblocking(true).expect("non-blocking");
    let halt = Arc::new(AtomicBool::new(false));
    let (fertig_tx, fertig_rx) = std::sync::mpsc::channel();
    let halt2 = Arc::clone(&halt);
    let schleife = std::thread::spawn(move || {
        halter::dienst::annehmen(&d, &hoerer, &halt2, 1);
        let _ = fertig_tx.send(());
    });
    // the first connection is served and stays open
    let mut a = verbinden_mit(&pfad);
    assert!(bedient(&mut a), "the first connection was not served");
    // a second one above the limit of 1 is closed without an answer
    let mut b = verbinden_mit(&pfad);
    let mut buf = [0u8; 16];
    assert_eq!(
        b.strom.read(&mut buf).expect("closed, not waiting"),
        0,
        "the connection above the limit was not closed"
    );
    // after the end of the first one there is a place again
    drop(a);
    let ende = std::time::Instant::now() + Duration::from_secs(10);
    loop {
        let mut c = verbinden_mit(&pfad);
        if bedient(&mut c) {
            break;
        }
        assert!(
            std::time::Instant::now() < ende,
            "no place within 10 s after the end of the first connection"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
    halt.store(true, Ordering::Relaxed);
    assert!(
        fertig_rx.recv_timeout(Duration::from_secs(5)).is_ok(),
        "the accept loop did not end within 5 s after the stop flag"
    );
    schleife.join().expect("loop");
    let _ = std::fs::remove_dir_all(&o);
}
