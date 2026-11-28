//! The start of a worker by its orchestrator (contract `dienst`, Nachtrag 28.11.2026 "Worker-Start durch den Orchestrator (Stufe
//! 2)"): the program `wb-dienst` with `lauf.stelle`, a stand-in of the authority on `stelle-kern` with the check of the other side
//! (`peerkanal`), and a Python script that plays `lauf`. A valid request after `laeuft` goes to the authority as `worker.anfordern`;
//! after `frei` the service starts the worker through the whole way of `Starten` and asks `sitzung.starten` before `lauf`; every
//! start with `lauf.stelle` waits for that answer.

#![cfg(any(target_os = "linux", target_os = "macos"))]

mod gemeinsam;

use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use gemeinsam::{DIENST, Probe, Prozess, Wegwerf, startdatei};
use peerkanal::{Annahme, Erlaubnis, Kennung, Server, ServerVorgabe, SocketModus, UnixOrt};
use serde_json::{Value, json};

/// A promise in the form of contract `stelle` section 15.3.
const ZS: &str = "zs-1795179540000-1";

/// The play of `lauf`, per session (the identifier comes from `sitzung` of the start file, its copy goes to `start-<sid>.json`).
/// The file `modus-<sid>` names it, else `modus`, else `still`. `still`: connects, sends `laeuft`, waits for `weiter-<sid>` or
/// `weiter`. `anfordern`: as `still`, but first for each line `<hex> <n>` of `schritte-<sid>` sends the bytes and reads `n` answer
/// lines, all of them into `antworten-<sid>`. After `laeuft` every mode says `verbunden-<sid>`. `kurz`: connects, sends `laeuft` and the bytes of `schritte-<sid>`, says `gesendet-<sid>`
/// and ends at once with 0. `anfordern-los` is `anfordern` after the file `los-<sid>`; `warten` sends the steps after `los-<sid>`
/// without reading an answer, says `gesendet-<sid>` and waits as `still`.
const LAUF: &str = r#"#!/usr/bin/python3 -I
import json, os, socket, sys, time
d = os.environ["M_ORDNER"]
def warte(*namen):
    ende = time.time() + 60
    while not any(os.path.exists(d + "/" + n) for n in namen) and time.time() < ende:
        time.sleep(0.01)
def schreiben(name, daten):
    open(d + "/" + name + ".tmp", "wb").write(daten)
    os.rename(d + "/" + name + ".tmp", d + "/" + name)
roh = open(sys.argv[3], "rb").read()
sid = json.loads(roh)["sitzung"]
schreiben("start-" + sid + ".json", roh)
modus = "still"
for n in ("modus-" + sid, "modus"):
    if os.path.exists(d + "/" + n):
        modus = open(d + "/" + n).read().strip()
        break
strom = socket.socket(socket.AF_UNIX)
strom.connect(sys.argv[2])
strom.sendall(b"laeuft\n")
schreiben("verbunden-" + sid, b"")
schritte = []
if os.path.exists(d + "/schritte-" + sid):
    schritte = [z.split(" ") for z in open(d + "/schritte-" + sid).read().splitlines()]
if modus in ("anfordern-los", "warten"):
    warte("los-" + sid)
if modus in ("kurz", "warten"):
    for teil, _ in schritte:
        strom.sendall(bytes.fromhex(teil))
    schreiben("gesendet-" + sid, b"")
    if modus == "kurz":
        sys.exit(0)
if modus in ("anfordern", "anfordern-los"):
    strom.settimeout(30)
    leser = strom.makefile("rb")
    antworten = b""
    for teil, n in schritte:
        strom.sendall(bytes.fromhex(teil))
        for _ in range(int(n)):
            antworten += leser.readline()
    schreiben("antworten-" + sid, antworten)
warte("weiter-" + sid, "weiter")
sys.exit(0)
"#;

/// Settings of fassung 2 with a block `effort`: `claude` may take `low` and `high`, `pi` `off` and `low`; the route `cloud-fest`
/// fixes its model.
const EINST: &str = r#"{"fassung":2,"klassen":["oeffentlich","intern","kundendaten"],"vorgabestufe":"kundendaten",
  "routen":[
    {"schluessel":"cloud-beispiel","art":"cloud","klasse":"intern","netzziele":[{"host":"api.cloud.example","port":443,"zugriff":"kopf-einsetzen","schluessel":true,"pfade":[{"methode":"GET","pfad":"/copilot_internal/*"}]}]},
    {"schluessel":"cloud-fest","art":"cloud","klasse":"intern","modell":"gpt-5.6-sol","netzziele":[{"host":"api.cloud.example","port":443,"zugriff":"kopf-einsetzen","schluessel":true,"pfade":[{"methode":"GET","pfad":"/copilot_internal/*"}]}]},
    {"schluessel":"lokal-beispiel","art":"lokal","klasse":"kundendaten","ziel":"http://127.0.0.1:11434/v1","modell":"qwen3:8b","netzziele":[]}],
  "kunden":[{"schluessel":"k-beispiel","erlaubt":["lokal-beispiel"],"verboten":[]}],
  "kundendaten_an_cloud":{"vorgabe":"nie","freigaben":[]},
  "effort":{"grenzen":[{"harness":"claude","erlaubt":["low","high"]},{"harness":"pi","erlaubt":["off","low"]}]}}"#;

/// What the stand-in answers to a frame: a line, nothing (it closes the connection), or a line after a pause.
enum Antwort {
    Zeile(Value),
    Roh(String),
    Schliessen,
}

type Regel = Box<dyn FnMut(&Value) -> Antwort + Send>;

/// The stand-in of the authority on `stelle-kern`: one frame per connection, recorded with its time; the answer comes from the
/// rule of the test. While `halten` is set it does not answer `sitzung.starten` (the start waits).
struct Stelle {
    gesehen: Arc<Mutex<Vec<(Instant, Value)>>>,
    regel: Arc<Mutex<Regel>>,
    halten: Arc<AtomicBool>,
    /// A pause before each answer of `worker.anfordern`.
    pause: Arc<Mutex<Duration>>,
}

/// The answer of a valid authority: `frei` with a promise for `worker.anfordern`, `gestartet` for `sitzung.starten`; a worker
/// without the promise is refused as the authority does (`stelle.zusage_ungueltig`).
fn gute_stelle(rahmen: &Value) -> Antwort {
    let e = &rahmen["eingabe"];
    match rahmen["vorgang"].as_str() {
        Some("worker.anfordern") => Antwort::Zeile(
            json!({"ok": true, "ergebnis": "frei", "sitzung": e["sitzung"], "zusage": ZS, "zusage_bis_ms": 1_795_179_570_000_i64}),
        ),
        Some("sitzung.starten") => {
            if e.get("orchestrator").is_some() && e.get("zusage") != Some(&json!(ZS)) {
                return ablehnung("stelle.zusage_ungueltig");
            }
            let start: Value =
                serde_json::from_str(e["start"]["text"].as_str().unwrap_or("")).unwrap_or_default();
            Antwort::Zeile(
                json!({"ok": true, "ergebnis": "gestartet", "sitzung": start["sitzung"], "route": start["route"], "route_art": "cloud"}),
            )
        }
        _ => Antwort::Schliessen,
    }
}

fn ablehnung(code: &str) -> Antwort {
    Antwort::Zeile(json!({"ok": false, "code": code, "grund": "fester Text"}))
}

impl Stelle {
    fn starten(ordner: &Path) -> Self {
        let uid = std::fs::metadata(ordner).expect("folder").uid();
        let mut v = ServerVorgabe::neu(
            Kennung::neu("stelle-kern").expect("name"),
            vec![Erlaubnis::Unix { uid, gid: None }],
            4,
        );
        v.unix = Some(UnixOrt {
            ordner: ordner.to_path_buf(),
            modus: SocketModus::M0600,
        });
        let server = Server::anlegen(v).expect("server");
        let s = Self {
            gesehen: Arc::new(Mutex::new(Vec::new())),
            regel: Arc::new(Mutex::new(Box::new(gute_stelle))),
            halten: Arc::new(AtomicBool::new(false)),
            pause: Arc::new(Mutex::new(Duration::ZERO)),
        };
        let (gesehen, regel, halten, pause) = (
            Arc::clone(&s.gesehen),
            Arc::clone(&s.regel),
            Arc::clone(&s.halten),
            Arc::clone(&s.pause),
        );
        // the thread ends with the test program
        std::thread::spawn(move || {
            loop {
                let Ok(Annahme::Vorlaeufig(vl)) = server.annehmen() else {
                    std::thread::sleep(Duration::from_millis(5));
                    continue;
                };
                let Ok(v) = vl.bestaetigen() else { continue };
                let mut leser = BufReader::new(v);
                let mut zeile = Vec::new();
                if leser.read_until(b'\n', &mut zeile).unwrap_or(0) == 0 {
                    continue;
                }
                let rahmen: Value = serde_json::from_slice(&zeile).expect("frame");
                gesehen
                    .lock()
                    .expect("lock")
                    .push((Instant::now(), rahmen.clone()));
                if rahmen["vorgang"] == "sitzung.starten" {
                    let bis = Instant::now() + Duration::from_secs(30);
                    while halten.load(Ordering::SeqCst) && Instant::now() < bis {
                        std::thread::sleep(Duration::from_millis(5));
                    }
                } else {
                    std::thread::sleep(*pause.lock().expect("lock"));
                }
                let antwort = (regel.lock().expect("lock"))(&rahmen);
                let text = match antwort {
                    Antwort::Zeile(v) => format!("{v}\n"),
                    Antwort::Roh(t) => t,
                    Antwort::Schliessen => continue,
                };
                let _ = leser.get_mut().write_all(text.as_bytes());
            }
        });
        s
    }

    fn regel(&self, r: impl FnMut(&Value) -> Antwort + Send + 'static) {
        *self.regel.lock().expect("lock") = Box::new(r);
    }

    fn vorgaenge(&self, vorgang: &str) -> Vec<Value> {
        self.gesehen
            .lock()
            .expect("lock")
            .iter()
            .filter(|(_, r)| r["vorgang"] == vorgang)
            .map(|(_, r)| r["eingabe"].clone())
            .collect()
    }

    /// The input of `sitzung.starten` for the session `sid`.
    fn starten_fuer(&self, sid: &str) -> Option<Value> {
        self.vorgaenge("sitzung.starten").into_iter().find(|e| {
            serde_json::from_str::<Value>(e["start"]["text"].as_str().unwrap_or(""))
                .is_ok_and(|s| s["sitzung"] == sid)
        })
    }
}

/// The service with `lauf.stelle` (or without it), the stand-in and one viewer.
fn dienst(weg: &Wegwerf, mit_stelle: bool) -> (Prozess, Probe, Option<Stelle>) {
    let programm = weg.0.join("lauf.py");
    std::fs::write(&programm, LAUF).expect("script");
    std::fs::set_permissions(&programm, std::fs::Permissions::from_mode(0o700)).expect("mode");
    let einst = weg.0.join("einst.json");
    std::fs::write(&einst, EINST).expect("settings");
    let ordner = weg.0.join("st");
    std::fs::create_dir(&ordner).expect("folder");
    std::fs::set_permissions(&ordner, std::fs::Permissions::from_mode(0o700)).expect("mode");
    let stelle = mit_stelle.then(|| Stelle::starten(&ordner));
    let uid = std::fs::metadata(&ordner).expect("folder").uid();
    let mut start = startdatei(weg, &[DIENST], &json!({}));
    let o = start.as_object_mut().expect("object");
    o.remove("kind");
    o.remove("groesse");
    start["anfang"]["sitzungen"] = json!([]);
    start["lauf"] = json!({
        "programm": programm.to_string_lossy(),
        "env": {"PATH": "/usr/bin:/bin", "LC_ALL": "C.UTF-8", "M_ORDNER": weg.0.to_string_lossy()},
        "groesse": [40, 10],
        "einstellungen": einst.to_string_lossy(),
    });
    if mit_stelle {
        start["lauf"]["stelle"] =
            json!({"ordner": ordner.to_string_lossy(), "erwartet": {"uid": uid}});
    }
    let prozess = Prozess::starten(weg, &start);
    let probe = Probe::neu(&weg.socket(), false);
    (prozess, probe, stelle)
}

/// The start structure of an orchestrator (fassung 2 without a choice of effort).
fn orchestrator(route: &str, kunde: &Value) -> Value {
    json!({"fassung": 2, "harness": "claude", "modell": "m", "route": route, "kunde": kunde, "arbeitsordner": "/tmp",
           "effort": null, "name": "Haupt"})
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    bytes.iter().fold(String::new(), |mut s, b| {
        let _ = write!(s, "{b:02x}");
        s
    })
}

/// What the session `sid` plays: its mode and its steps (`<bytes> <number of answers>`).
fn spielen(weg: &Wegwerf, sid: &str, modus: &str, schritte: &[(&[u8], usize)]) {
    std::fs::write(weg.0.join(format!("modus-{sid}")), modus).expect("modus");
    let text: Vec<String> = schritte
        .iter()
        .map(|(b, n)| format!("{} {n}", hex(b)))
        .collect();
    std::fs::write(weg.0.join(format!("schritte-{sid}")), text.join("\n")).expect("steps");
}

fn warte_datei(weg: &Wegwerf, name: &str) -> Vec<u8> {
    let ende = Instant::now() + Duration::from_secs(30);
    loop {
        if let Ok(b) = std::fs::read(weg.0.join(name)) {
            return b;
        }
        assert!(Instant::now() < ende, "{name} did not appear");
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// Sends `Starten` and returns the identifier of `Gestartet`, or the refusal.
fn starten(p: &mut Probe, start: &Value, extra: &Value) -> Result<String, Value> {
    let (n, a) = (p.nachrichten.len(), p.ablehnungen_voll.len());
    let mut e = json!({"art": "Starten", "start": start});
    for (k, v) in extra.as_object().into_iter().flatten() {
        e[k] = v.clone();
    }
    p.ereignis(&e);
    assert!(
        p.warte(Duration::from_secs(20), |p| p.ablehnungen_voll.len() > a
            || p.nachrichten[n..].iter().any(|m| m["art"] == "Gestartet")),
        "no answer to {e}"
    );
    if let Some(m) = p.nachrichten[n..].iter().find(|m| m["art"] == "Gestartet") {
        return Ok(m["sitzung"].as_str().expect("id").to_owned());
    }
    Err(p.ablehnungen_voll[a].clone())
}

/// The entries of the last list.
fn liste(p: &mut Probe) -> Vec<Value> {
    let _ = p.zustand();
    p.nachrichten
        .iter()
        .rev()
        .find(|m| m["art"] == "Sitzungsliste")
        .and_then(|m| m["sitzungen"].as_array().cloned())
        .unwrap_or_default()
}

fn ende(mut prozess: Prozess, weg: &Wegwerf) {
    std::fs::write(weg.0.join("weiter"), "").expect("weiter");
    prozess.signal("TERM");
    assert_eq!(prozess.warte_ende(Duration::from_secs(15)), Some(0));
}

/// A request goes to the authority with every field, `frei` starts the worker with `orchestrator` and `zusage` at `sitzung.starten`,
/// and the run gets `frei <sitzung>`. The worker is in the list with its orchestrator and its role; its start file is fassung 2 with
/// the effort of the line, and the bytes at `sitzung.starten` are exactly those of its start file.
#[test]
fn anforderung_frei_startet_den_worker_und_der_lauf_bekommt_frei_mit_der_sitzung() {
    let weg = Wegwerf::neu("anf-frei");
    let (prozess, mut p, stelle) = dienst(&weg, true);
    let stelle = stelle.expect("stand-in");
    spielen(
        &weg,
        "s-1",
        "anfordern",
        &[
            (b"anfordern pruefer claude m low\n", 1),
            (b"anfordern zweiter claude m high\n", 1),
        ],
    );
    let o = starten(
        &mut p,
        &orchestrator("cloud-beispiel", &json!("k-beispiel")),
        &json!({}),
    )
    .expect("s-1");
    assert_eq!(o, "s-1");
    let antworten = warte_datei(&weg, "antworten-s-1");
    assert_eq!(antworten, b"frei s-2\nfrei s-3\n");
    // the second request counts the running worker s-2
    let anf = stelle.vorgaenge("worker.anfordern");
    assert_eq!(
        anf,
        vec![
            json!({"sitzung": "s-1", "rolle": "pruefer", "harness": "claude", "modell": "m", "effort": "low",
                     "laufend": 0, "route": "cloud-beispiel", "kunde": "k-beispiel"}),
            json!({"sitzung": "s-1", "rolle": "zweiter", "harness": "claude", "modell": "m", "effort": "high",
                     "laufend": 1, "route": "cloud-beispiel", "kunde": "k-beispiel"})
        ]
    );
    // the orchestrator itself went to the authority without `orchestrator` and `zusage`, with the bytes of its start file
    let s1 = stelle.starten_fuer("s-1").expect("s-1 at the authority");
    assert_eq!(s1.as_object().map(serde_json::Map::len), Some(1), "{s1}");
    assert_eq!(
        s1["start"]["text"].as_str().map(str::as_bytes),
        Some(&warte_datei(&weg, "start-s-1.json")[..])
    );
    let s2 = stelle.starten_fuer("s-2").expect("s-2 at the authority");
    assert_eq!(
        (&s2["orchestrator"], &s2["zusage"]),
        (&json!("s-1"), &json!(ZS))
    );
    let datei = warte_datei(&weg, "start-s-2.json");
    assert_eq!(
        s2["start"]["text"].as_str().map(str::as_bytes),
        Some(&datei[..])
    );
    let start2: Value = serde_json::from_slice(&datei).expect("start file");
    assert_eq!(
        start2,
        json!({"fassung": 2, "sitzung": "s-2", "harness": "claude", "modell": "m", "route": "cloud-beispiel",
               "kunde": "k-beispiel", "arbeitsordner": "/tmp", "effort": "low", "name": null, "fortsetzung_von": null})
    );
    // the worker is in the list, with its orchestrator and its role; the list of the workers names it
    let eintrag = liste(&mut p)
        .into_iter()
        .find(|e| e["id"] == "s-2")
        .expect("s-2 in the list");
    assert_eq!(eintrag["zustand"], "laeuft", "{eintrag}");
    let agenten = p
        .nachrichten
        .iter()
        .rev()
        .find(|m| m["art"] == "Agenten")
        .cloned()
        .expect("Agenten");
    let a = &agenten["agenten"][0];
    assert_eq!(
        (&a["worker"], &a["sitzung"], &a["rolle"]),
        (&json!("s-2"), &json!("s-1"), &json!("pruefer")),
        "{agenten}"
    );
    // a second request counts the running worker
    std::fs::write(weg.0.join("weiter-s-1"), "").expect("weiter");
    ende(prozess, &weg);
}

/// Every refusal of the authority goes to the run as `abgelehnt <code>`, and nothing starts. A refusal of `sitzung.starten` of the
/// worker removes its folder; the identifier stays taken. A model of another route ends with the check of `Starten`.
#[test]
fn jede_ablehnung_geht_als_code_an_den_lauf_und_startet_nichts() {
    let weg = Wegwerf::neu("anf-abgelehnt");
    let (prozess, mut p, stelle) = dienst(&weg, true);
    let stelle = stelle.expect("stand-in");
    let codes = [
        "stelle.vorgabe_fehlt",
        "stelle.sitzung_unbekannt",
        "stelle.orchestrator_ist_worker",
        "stelle.route_unbekannt",
        "stelle.worker_klasse_hoeher",
        "stelle.worker_fremder_kunde",
        "stelle.harness_nicht_erlaubt",
        "stelle.modell_nicht_erlaubt",
        "stelle.effort_nicht_erlaubt",
        "stelle.worker_hoechstzahl",
        "stelle.zusagen_erschoepft",
    ];
    let liste_der_codes = Arc::new(Mutex::new(
        codes.iter().map(|c| (*c).to_owned()).collect::<Vec<_>>(),
    ));
    let l = Arc::clone(&liste_der_codes);
    stelle.regel(move |r| {
        if r["vorgang"] == "worker.anfordern" {
            let code = l.lock().expect("lock").remove(0);
            return ablehnung(&code);
        }
        gute_stelle(r)
    });
    let zeile: &[u8] = b"anfordern pruefer claude m low\n";
    let schritte: Vec<(&[u8], usize)> = codes.iter().map(|_| (zeile, 1)).collect();
    spielen(&weg, "s-1", "anfordern", &schritte);
    starten(
        &mut p,
        &orchestrator("cloud-beispiel", &json!("k-beispiel")),
        &json!({}),
    )
    .expect("s-1");
    let antworten = String::from_utf8(warte_datei(&weg, "antworten-s-1")).expect("text");
    let erwartet: String = codes.iter().fold(String::new(), |mut t, c| {
        t.push_str("abgelehnt ");
        t.push_str(c);
        t.push('\n');
        t
    });
    assert_eq!(antworten, erwartet);
    assert_eq!(stelle.vorgaenge("sitzung.starten").len(), 1, "only s-1");
    // a refusal at `sitzung.starten` of the worker, a model of another route and an effort out of the block (the checks of
    // `Starten` after `frei`, before the folder)
    stelle.regel(|r| {
        if r["vorgang"] == "sitzung.starten" && r["eingabe"].get("orchestrator").is_some() {
            return ablehnung("stelle.zusage_ungueltig");
        }
        gute_stelle(r)
    });
    // `cloud-fest` fixes the model `gpt-5.6-sol`: an orchestrator with `m` is refused by the check and takes no identifier
    let a = starten(
        &mut p,
        &orchestrator("cloud-fest", &json!("k-beispiel")),
        &json!({}),
    )
    .expect_err("model of another route");
    assert_eq!(a["code"], "start.modell_route", "{a}");
    spielen(
        &weg,
        "s-2",
        "anfordern",
        &[
            (b"anfordern pruefer claude gpt-5.6-sol low\n", 1),
            (b"anfordern pruefer claude qwen3:8b low\n", 1),
            (b"anfordern pruefer claude gpt-5.6-sol xhigh\n", 1),
        ],
    );
    let mut orch = orchestrator("cloud-fest", &json!("k-beispiel"));
    orch["modell"] = json!("gpt-5.6-sol");
    let o = starten(&mut p, &orch, &json!({})).expect("orchestrator on cloud-fest");
    assert_eq!(o, "s-2", "a refused start takes no identifier");
    let antworten = String::from_utf8(warte_datei(&weg, "antworten-s-2")).expect("text");
    assert_eq!(
        antworten,
        "abgelehnt stelle.zusage_ungueltig\nabgelehnt start.modell_route\nabgelehnt start.effort_grenze\n"
    );
    let s3 = stelle
        .starten_fuer("s-3")
        .expect("the worker s-3 was asked for");
    assert_eq!(
        (&s3["orchestrator"], &s3["zusage"]),
        (&json!("s-2"), &json!(ZS))
    );
    assert!(!weg.ordner().join("s-3").exists(), "its folder is removed");
    assert!(
        stelle.starten_fuer("s-4").is_none(),
        "the checks refused before the folder"
    );
    assert_eq!(
        stelle.vorgaenge("sitzung.starten").len(),
        3,
        "s-1, s-2, s-3"
    );
    std::fs::write(weg.0.join("weiter-s-1"), "").expect("weiter");
    std::fs::write(weg.0.join("weiter-s-2"), "").expect("weiter");
    ende(prozess, &weg);
}

/// `sitzung.starten` of a worker refused by the authority: no start, the folder removed, the identifier taken.
#[test]
fn eine_ablehnung_von_sitzung_starten_entfernt_den_ordner_und_verbraucht_die_kennung() {
    let weg = Wegwerf::neu("anf-zusage");
    let (prozess, mut p, stelle) = dienst(&weg, true);
    let stelle = stelle.expect("stand-in");
    stelle.regel(|r| {
        if r["vorgang"] == "sitzung.starten" && r["eingabe"].get("orchestrator").is_some() {
            return ablehnung("stelle.zusage_ungueltig");
        }
        gute_stelle(r)
    });
    spielen(
        &weg,
        "s-1",
        "anfordern",
        &[
            (b"anfordern pruefer claude m low\n", 1),
            (b"anfordern pruefer claude m high\n", 1),
        ],
    );
    starten(
        &mut p,
        &orchestrator("cloud-beispiel", &json!("k-beispiel")),
        &json!({}),
    )
    .expect("s-1");
    let antworten = String::from_utf8(warte_datei(&weg, "antworten-s-1")).expect("text");
    assert_eq!(
        antworten,
        "abgelehnt stelle.zusage_ungueltig\nabgelehnt stelle.zusage_ungueltig\n"
    );
    for sid in ["s-2", "s-3"] {
        let e = stelle.starten_fuer(sid).expect("asked");
        assert_eq!(
            (&e["orchestrator"], &e["zusage"]),
            (&json!("s-1"), &json!(ZS))
        );
        assert!(
            !weg.ordner().join(sid).exists(),
            "the folder of {sid} is removed"
        );
    }
    assert!(
        liste(&mut p)
            .iter()
            .all(|e| e["id"] != "s-2" && e["id"] != "s-3")
    );
    // the identifiers stay taken: the next start is s-4
    stelle.regel(gute_stelle);
    spielen(&weg, "s-4", "still", &[]);
    let n = starten(
        &mut p,
        &orchestrator("cloud-beispiel", &json!("k-beispiel")),
        &json!({}),
    )
    .expect("s-4");
    assert_eq!(n, "s-4");
    std::fs::write(weg.0.join("weiter-s-1"), "").expect("weiter");
    ende(prozess, &weg);
}

/// A failure of the authority is never `frei`: no answer (closed), no JSON, an old form without a promise, another session, a
/// promise out of form, a line without LF. The run gets `stelle_nicht_erreichbar` or `stelle_antwort_ungueltig`, and no worker is
/// asked for at `sitzung.starten`.
#[test]
fn ausfall_bei_worker_anfordern_ist_nie_frei() {
    let weg = Wegwerf::neu("anf-ausfall");
    let (prozess, mut p, stelle) = dienst(&weg, true);
    let stelle = stelle.expect("stand-in");
    let falsch = Arc::new(Mutex::new(vec![
        Antwort::Schliessen,
        Antwort::Roh("kein json\n".to_owned()),
        Antwort::Zeile(json!({"ok": true, "ergebnis": "frei", "sitzung": "s-1"})),
        Antwort::Zeile(
            json!({"ok": true, "ergebnis": "frei", "sitzung": "s-9", "zusage": ZS, "zusage_bis_ms": 1}),
        ),
        Antwort::Zeile(
            json!({"ok": true, "ergebnis": "frei", "sitzung": "s-1", "zusage": "zs-1-0", "zusage_bis_ms": 1}),
        ),
        Antwort::Roh(format!(
            "{}",
            json!({"ok": true, "ergebnis": "frei", "sitzung": "s-1", "zusage": ZS, "zusage_bis_ms": 1})
        )),
    ]));
    let f = Arc::clone(&falsch);
    stelle.regel(move |r| {
        if r["vorgang"] == "worker.anfordern" {
            return f.lock().expect("lock").remove(0);
        }
        gute_stelle(r)
    });
    let zeile: &[u8] = b"anfordern pruefer claude m low\n";
    spielen(
        &weg,
        "s-1",
        "anfordern",
        &[
            (zeile, 1),
            (zeile, 1),
            (zeile, 1),
            (zeile, 1),
            (zeile, 1),
            (zeile, 1),
        ],
    );
    starten(
        &mut p,
        &orchestrator("cloud-beispiel", &json!("k-beispiel")),
        &json!({}),
    )
    .expect("s-1");
    let antworten = String::from_utf8(warte_datei(&weg, "antworten-s-1")).expect("text");
    assert_eq!(
        antworten,
        "abgelehnt stelle_nicht_erreichbar\nabgelehnt stelle_antwort_ungueltig\nabgelehnt stelle_antwort_ungueltig\n\
         abgelehnt stelle_antwort_ungueltig\nabgelehnt stelle_antwort_ungueltig\nabgelehnt stelle_nicht_erreichbar\n"
    );
    assert_eq!(
        stelle.vorgaenge("sitzung.starten").len(),
        1,
        "no worker was asked for"
    );
    std::fs::write(weg.0.join("weiter-s-1"), "").expect("weiter");
    ende(prozess, &weg);
}

/// `sitzung.starten` fails (closed, a wrong form, another session, a refusal): the start of a viewer is refused with the reason,
/// `lauf` never starts, the folder goes.
#[test]
fn ausfall_bei_sitzung_starten_startet_nichts() {
    let weg = Wegwerf::neu("anf-ausfall2");
    let (prozess, mut p, stelle) = dienst(&weg, true);
    let stelle = stelle.expect("stand-in");
    let s = Arc::new(Mutex::new(vec![
        Antwort::Schliessen,
        Antwort::Roh("{\"ok\": true}\n".to_owned()),
        Antwort::Zeile(
            json!({"ok": true, "ergebnis": "gestartet", "sitzung": "s-99", "route": "cloud-beispiel", "route_art": "cloud"}),
        ),
        ablehnung("stelle.sitzung_vergeben"),
    ]));
    let s2 = Arc::clone(&s);
    stelle.regel(move |_| s2.lock().expect("lock").remove(0));
    let mut gruende = Vec::new();
    for _ in 0..4 {
        let a = starten(
            &mut p,
            &orchestrator("cloud-beispiel", &json!("k-beispiel")),
            &json!({}),
        )
        .expect_err("refused");
        gruende.push((a["grund"].clone(), a["code"].clone()));
    }
    assert_eq!(
        gruende,
        vec![
            (
                json!("stelle_nicht_erreichbar"),
                json!("stelle_nicht_erreichbar")
            ),
            (
                json!("stelle_antwort_ungueltig"),
                json!("stelle_antwort_ungueltig")
            ),
            (
                json!("stelle_antwort_ungueltig"),
                json!("stelle_antwort_ungueltig")
            ),
            (json!("stelle_abgelehnt"), json!("stelle.sitzung_vergeben")),
        ]
    );
    for sid in ["s-1", "s-2", "s-3", "s-4"] {
        assert!(!weg.ordner().join(sid).exists(), "{sid} removed");
        assert!(
            !weg.0.join(format!("start-{sid}.json")).exists(),
            "{sid}: lauf never started"
        );
    }
    assert!(liste(&mut p).is_empty(), "no session started");
    ende(prozess, &weg);
}

/// Two requests in one send: the first goes to the authority, the second gets `anfordern_offen` without a question; the answers
/// come in the order they are ready.
#[test]
fn eine_zweite_anforderung_vor_der_antwort_ist_offen() {
    let weg = Wegwerf::neu("anf-zwei");
    let (prozess, mut p, stelle) = dienst(&weg, true);
    let stelle = stelle.expect("stand-in");
    *stelle.pause.lock().expect("lock") = Duration::from_millis(300);
    spielen(
        &weg,
        "s-1",
        "anfordern",
        &[(
            b"anfordern pruefer claude m low\nanfordern zweiter claude m low\n",
            2,
        )],
    );
    starten(
        &mut p,
        &orchestrator("cloud-beispiel", &json!("k-beispiel")),
        &json!({}),
    )
    .expect("s-1");
    let antworten = String::from_utf8(warte_datei(&weg, "antworten-s-1")).expect("text");
    assert_eq!(antworten, "abgelehnt anfordern_offen\nfrei s-2\n");
    assert_eq!(stelle.vorgaenge("worker.anfordern").len(), 1);
    assert_eq!(stelle.vorgaenge("worker.anfordern")[0]["rolle"], "pruefer");
    std::fs::write(weg.0.join("weiter-s-1"), "").expect("weiter");
    ende(prozess, &weg);
}

/// The orchestrator ends while the authority decides: `frei` after its end starts no worker, and no `sitzung.starten` goes.
#[test]
fn frei_nach_dem_ende_des_orchestrators_startet_keinen_worker() {
    let weg = Wegwerf::neu("anf-ende");
    let (prozess, mut p, stelle) = dienst(&weg, true);
    let stelle = stelle.expect("stand-in");
    *stelle.pause.lock().expect("lock") = Duration::from_millis(1500);
    spielen(
        &weg,
        "s-1",
        "kurz",
        &[(b"anfordern pruefer claude m low\n", 1)],
    );
    starten(
        &mut p,
        &orchestrator("cloud-beispiel", &json!("k-beispiel")),
        &json!({}),
    )
    .expect("s-1");
    warte_datei(&weg, "gesendet-s-1");
    let bis = Instant::now() + Duration::from_secs(20);
    while stelle.vorgaenge("worker.anfordern").is_empty() {
        assert!(Instant::now() < bis, "the request reaches the authority");
        std::thread::sleep(Duration::from_millis(10));
    }
    // the answer comes 1.5 s later; the orchestrator ended already
    std::thread::sleep(Duration::from_secs(3));
    let l = liste(&mut p);
    assert_eq!(l.len(), 1, "{l:?}");
    assert_eq!(l[0]["zustand"], "beendet", "{l:?}");
    assert_eq!(stelle.vorgaenge("sitzung.starten").len(), 1, "only s-1");
    assert!(!weg.ordner().join("s-2").exists());
    ende(prozess, &weg);
}

/// The two steps of a start without a worker: `sitzung.starten` before `lauf`, `Gestartet` only after the answer. A waiting start
/// counts to the 8 running sessions. A worker started by a viewer names its orchestrator and has no promise: the authority refuses it.
#[test]
fn ein_start_wartet_auf_die_stelle_und_zaehlt_zu_den_acht() {
    let weg = Wegwerf::neu("anf-zwei-stufen");
    let (prozess, mut p, stelle) = dienst(&weg, true);
    let stelle = stelle.expect("stand-in");
    stelle.halten.store(true, Ordering::SeqCst);
    let n = p.nachrichten.len();
    for _ in 0..8 {
        p.ereignis(&json!({"art": "Starten", "start": orchestrator("cloud-beispiel", &json!("k-beispiel"))}));
    }
    let bis = Instant::now() + Duration::from_secs(10);
    while stelle.vorgaenge("sitzung.starten").is_empty() {
        assert!(Instant::now() < bis, "sitzung.starten comes");
        std::thread::sleep(Duration::from_millis(10));
    }
    // eight starts wait: the ninth is refused, and none has started or run `lauf`
    let a = starten(
        &mut p,
        &orchestrator("cloud-beispiel", &json!("k-beispiel")),
        &json!({}),
    )
    .expect_err("ninth");
    assert_eq!(a["grund"], "zu_viele_sitzungen", "{a}");
    assert!(!p.nachrichten[n..].iter().any(|m| m["art"] == "Gestartet"));
    assert!(
        !weg.0.join("start-s-1.json").exists(),
        "lauf waits for the authority"
    );
    assert!(
        weg.ordner().join("s-1").join("start.json").exists(),
        "the folder is made before"
    );
    stelle.halten.store(false, Ordering::SeqCst);
    assert!(p.warte(Duration::from_secs(30), |p| {
        p.nachrichten[n..]
            .iter()
            .filter(|m| m["art"] == "Gestartet")
            .count()
            == 8
    }));
    let gestartet: Vec<Value> = p.nachrichten[n..]
        .iter()
        .filter(|m| m["art"] == "Gestartet")
        .map(|m| m["sitzung"].clone())
        .collect();
    assert_eq!(
        gestartet,
        (1..=8).map(|i| json!(format!("s-{i}"))).collect::<Vec<_>>()
    );
    for i in 1..=8 {
        let e = stelle.starten_fuer(&format!("s-{i}")).expect("asked");
        assert_eq!(
            e.as_object().map(serde_json::Map::len),
            Some(1),
            "no orchestrator, no promise: {e}"
        );
    }
    // a worker by a viewer: `orchestrator` without `zusage`, the authority refuses
    std::fs::write(weg.0.join("weiter-s-8"), "").expect("weiter");
    assert!(p.warte(Duration::from_secs(20), |p| {
        p.nachrichten
            .iter()
            .rev()
            .find(|m| m["art"] == "Sitzungsliste")
            .is_some_and(|m| {
                m["sitzungen"].as_array().is_some_and(|l| {
                    l.iter()
                        .any(|e| e["id"] == "s-8" && e["zustand"] == "beendet")
                })
            })
    }));
    let a = starten(
        &mut p,
        &orchestrator("cloud-beispiel", &json!("k-beispiel")),
        &json!({"orchestrator": "s-1", "rolle": "pruefer"}),
    )
    .expect_err("worker without promise");
    assert_eq!(
        (&a["grund"], &a["code"]),
        (
            &json!("stelle_abgelehnt"),
            &json!("stelle.zusage_ungueltig")
        ),
        "{a}"
    );
    let e = stelle.starten_fuer("s-9").expect("asked");
    assert_eq!(e["orchestrator"], "s-1");
    assert!(e.get("zusage").is_none(), "{e}");
    assert!(!weg.ordner().join("s-9").exists());
    ende(prozess, &weg);
}

/// Without `lauf.stelle` nothing changes: a valid request gets `stelle_nicht_erreichbar` at once (stage 1), and a start needs no
/// authority.
#[test]
fn ohne_lauf_stelle_bleibt_stufe_1() {
    let weg = Wegwerf::neu("anf-ohne");
    let (prozess, mut p, _) = dienst(&weg, false);
    spielen(
        &weg,
        "s-1",
        "anfordern",
        &[(
            b"anfordern pruefer claude m low\nanfordern zweiter claude m low\n",
            2,
        )],
    );
    starten(
        &mut p,
        &orchestrator("cloud-beispiel", &json!("k-beispiel")),
        &json!({}),
    )
    .expect("s-1");
    let antworten = String::from_utf8(warte_datei(&weg, "antworten-s-1")).expect("text");
    assert_eq!(
        antworten,
        "abgelehnt stelle_nicht_erreichbar\nabgelehnt anfordern_offen\n"
    );
    assert_eq!(liste(&mut p).len(), 1);
    std::fs::write(weg.0.join("weiter-s-1"), "").expect("weiter");
    ende(prozess, &weg);
}

/// `laufend` counts the starts of workers of this orchestrator that wait for the authority: a worker started by a viewer waits at
/// `sitzung.starten` while the run requests one.
#[test]
fn laufend_zaehlt_den_wartenden_start_eines_workers() {
    let weg = Wegwerf::neu("anf-laufend");
    let (prozess, mut p, stelle) = dienst(&weg, true);
    let stelle = stelle.expect("stand-in");
    spielen(
        &weg,
        "s-1",
        "anfordern-los",
        &[(b"anfordern pruefer claude m low\n", 1)],
    );
    starten(
        &mut p,
        &orchestrator("cloud-beispiel", &json!("k-beispiel")),
        &json!({}),
    )
    .expect("s-1");
    // the run of s-1 is connected and waits: the request comes right after `los-s-1`
    warte_datei(&weg, "verbunden-s-1");
    stelle.halten.store(true, Ordering::SeqCst);
    p.ereignis(
        &json!({"art": "Starten", "start": orchestrator("cloud-beispiel", &json!("k-beispiel")),
                       "orchestrator": "s-1", "rolle": "hand"}),
    );
    warte_auf(|| stelle.starten_fuer("s-2").is_some(), "s-2 waits");
    std::fs::write(weg.0.join("los-s-1"), "").expect("los");
    // the request is read while s-2 waits; the authority gets it after the answer to s-2
    std::thread::sleep(Duration::from_millis(500));
    stelle.halten.store(false, Ordering::SeqCst);
    let antworten = warte_datei(&weg, "antworten-s-1");
    assert_eq!(antworten, b"frei s-3\n");
    let anf = stelle.vorgaenge("worker.anfordern");
    assert_eq!(anf.len(), 1);
    assert_eq!(
        anf[0]["laufend"], 1,
        "the waiting start of s-2 counts: {}",
        anf[0]
    );
    std::fs::write(weg.0.join("weiter-s-1"), "").expect("weiter");
    ende(prozess, &weg);
}

/// The orchestrator ends while `sitzung.starten` of its worker waits: the answer `gestartet` starts nothing, the folder goes.
#[test]
fn ende_des_orchestrators_waehrend_sitzung_starten_startet_keinen_worker() {
    let weg = Wegwerf::neu("anf-ende2");
    let (prozess, mut p, stelle) = dienst(&weg, true);
    let stelle = stelle.expect("stand-in");
    spielen(
        &weg,
        "s-1",
        "warten",
        &[(b"anfordern pruefer claude m low\n", 1)],
    );
    starten(
        &mut p,
        &orchestrator("cloud-beispiel", &json!("k-beispiel")),
        &json!({}),
    )
    .expect("s-1");
    stelle.halten.store(true, Ordering::SeqCst);
    std::fs::write(weg.0.join("los-s-1"), "").expect("los");
    warte_auf(|| stelle.starten_fuer("s-2").is_some(), "s-2 waits");
    std::fs::write(weg.0.join("weiter-s-1"), "").expect("weiter");
    let bis = Instant::now() + Duration::from_secs(20);
    while !liste(&mut p)
        .iter()
        .any(|e| e["id"] == "s-1" && e["zustand"] == "beendet")
    {
        assert!(Instant::now() < bis, "s-1 ends");
        p.lesen(Duration::from_millis(20));
    }
    stelle.halten.store(false, Ordering::SeqCst);
    warte_auf(
        || !weg.ordner().join("s-2").exists(),
        "the folder of s-2 goes",
    );
    std::thread::sleep(Duration::from_millis(300));
    let l = liste(&mut p);
    assert_eq!(l.len(), 1, "{l:?}");
    assert!(!weg.0.join("start-s-2.json").exists(), "lauf never started");
    ende(prozess, &weg);
}

/// Two `Fortsetzen` of the same session while the authority decides: the first that is answered continues it, the second is
/// `schon_fortgesetzt` and its folder goes.
#[test]
fn zwei_fortsetzen_waehrend_die_stelle_entscheidet() {
    let weg = Wegwerf::neu("anf-fortsetzen");
    let (prozess, mut p, stelle) = dienst(&weg, true);
    let stelle = stelle.expect("stand-in");
    let alt = starten(
        &mut p,
        &orchestrator("cloud-beispiel", &json!("k-beispiel")),
        &json!({}),
    )
    .expect("s-1");
    std::fs::write(weg.0.join("weiter-s-1"), "").expect("weiter");
    let bis = Instant::now() + Duration::from_secs(20);
    while !liste(&mut p)
        .iter()
        .any(|e| e["id"] == alt && e["zustand"] == "beendet")
    {
        assert!(Instant::now() < bis, "s-1 ends");
        p.lesen(Duration::from_millis(20));
    }
    stelle.halten.store(true, Ordering::SeqCst);
    let (n, a) = (p.nachrichten.len(), p.ablehnungen_voll.len());
    p.ereignis(&json!({"art": "Fortsetzen", "sitzung": alt}));
    p.ereignis(&json!({"art": "Fortsetzen", "sitzung": alt}));
    warte_auf(|| stelle.starten_fuer("s-2").is_some(), "s-2 waits");
    std::thread::sleep(Duration::from_millis(200));
    stelle.halten.store(false, Ordering::SeqCst);
    assert!(
        p.warte(Duration::from_secs(20), |p| p.ablehnungen_voll.len() > a
            && p.nachrichten[n..].iter().any(|m| m["art"] == "Gestartet"))
    );
    let gestartet: Vec<&Value> = p.nachrichten[n..]
        .iter()
        .filter(|m| m["art"] == "Gestartet")
        .collect();
    assert_eq!(gestartet.len(), 1);
    assert_eq!(gestartet[0]["sitzung"], "s-2");
    assert_eq!(p.ablehnungen_voll[a]["grund"], "schon_fortgesetzt");
    assert!(
        stelle.starten_fuer("s-3").is_some(),
        "both went to the authority"
    );
    assert!(!weg.ordner().join("s-3").exists());
    std::fs::write(weg.0.join("weiter-s-2"), "").expect("weiter");
    ende(prozess, &weg);
}

/// The identifier of a refused start stays taken even when an earlier start that waited with it succeeds afterwards.
#[test]
fn eine_kennung_bleibt_verbraucht_auch_hinter_einem_gelungenen_start() {
    let weg = Wegwerf::neu("anf-kennung");
    let (prozess, mut p, stelle) = dienst(&weg, true);
    let stelle = stelle.expect("stand-in");
    stelle.regel(|r| {
        let start: Value =
            serde_json::from_str(r["eingabe"]["start"]["text"].as_str().unwrap_or(""))
                .unwrap_or_default();
        if start["sitzung"] == "s-2" {
            return ablehnung("stelle.sitzung_vergeben");
        }
        gute_stelle(r)
    });
    stelle.halten.store(true, Ordering::SeqCst);
    let (n, a) = (p.nachrichten.len(), p.ablehnungen_voll.len());
    for _ in 0..2 {
        p.ereignis(&json!({"art": "Starten", "start": orchestrator("cloud-beispiel", &json!("k-beispiel"))}));
    }
    warte_auf(|| stelle.starten_fuer("s-1").is_some(), "s-1 waits");
    std::thread::sleep(Duration::from_millis(200));
    stelle.halten.store(false, Ordering::SeqCst);
    assert!(
        p.warte(Duration::from_secs(20), |p| p.ablehnungen_voll.len() > a
            && p.nachrichten[n..].iter().any(|m| m["art"] == "Gestartet"))
    );
    assert_eq!(p.ablehnungen_voll[a]["code"], "stelle.sitzung_vergeben");
    let n3 = starten(
        &mut p,
        &orchestrator("cloud-beispiel", &json!("k-beispiel")),
        &json!({}),
    )
    .expect("third");
    assert_eq!(n3, "s-3", "s-2 stays taken");
    ende(prozess, &weg);
}

fn warte_auf(bedingung: impl Fn() -> bool, was: &str) {
    let bis = Instant::now() + Duration::from_secs(20);
    while !bedingung() {
        assert!(Instant::now() < bis, "{was}");
        std::thread::sleep(Duration::from_millis(5));
    }
}
