//! The runner side of the outside cases `ort: windows` (contract section "Fälle und Prüfer"). It plays the server or the
//! client of one case and writes one JSON line per step into a log file. The steering script in the VM starts it under the
//! accounts and containers of the case and compares the lines with `erwartet/<case>.json`.
//!
//! ```text
//! peerkanal-aussen server --log F --name N --erlaubt SID[:PAKET] ... [--instanzen n] [--hallo-ms n] [--pflicht]
//!                         [--job NAME] [--zusatz PAKET] [--halt DATEI] [--haken bestaetigen|ablehnen:K|warte:DATEI]
//!                         [--anzahl n]
//! peerkanal-aussen client --log F --name N --erwartet SID [--vorher-ms n] [--frist-ms n] [--senden TEXT] [--halten-ms n] [--anzahl n]
//! peerkanal-aussen roh    --log F --pfad P --zugriff HEX [--senden TEXT] [--lesen-ms n]
//! ```
//!
//! A test program, no part of a shipped build. It has no `unsafe`; the job of a case is opened by name through
//! `JobBezug::oeffnen`.

#![forbid(unsafe_code)]
#![allow(clippy::print_stderr)]

#[cfg(not(windows))]
fn main() {}

#[cfg(windows)]
fn main() {
    std::process::exit(windows::main());
}

#[cfg(windows)]
mod windows {
    use std::collections::HashMap;
    use std::fs::{File, OpenOptions};
    use std::io::{Read, Write};
    use std::os::windows::fs::OpenOptionsExt;
    use std::sync::{Arc, Mutex, mpsc};
    use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

    use peerkanal::{
        Annahme, Aufrufergrund, ClientVorgabe, Erlaubnis, Erwartet, JobBezug, Kennung, PackageSid,
        ServerVorgabe, Sid, testnaht,
    };
    use serde_json::{Value, json};

    fn ms(d: Duration) -> u64 {
        u64::try_from(d.as_millis()).unwrap_or(u64::MAX)
    }

    struct Log(Mutex<File>);

    impl Log {
        fn zeile(&self, mut v: Value) {
            let ms = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, |d| d.as_millis());
            v["t_ms"] = json!(u64::try_from(ms).unwrap_or(0));
            v["pid_selbst"] = json!(std::process::id());
            let mut f = self
                .0
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let _ = writeln!(f, "{v}");
            let _ = f.flush();
        }
    }

    struct Args {
        werte: HashMap<String, Vec<String>>,
    }

    impl Args {
        fn lesen(rest: &[String]) -> Self {
            let mut werte: HashMap<String, Vec<String>> = HashMap::new();
            let mut i = 0;
            while i < rest.len() {
                let k = rest[i].trim_start_matches("--").to_owned();
                if k == "pflicht" {
                    werte.entry(k).or_default().push(String::new());
                    i += 1;
                } else {
                    werte
                        .entry(k)
                        .or_default()
                        .push(rest.get(i + 1).cloned().unwrap_or_default());
                    i += 2;
                }
            }
            Self { werte }
        }
        fn eins(&self, k: &str) -> Option<&str> {
            self.werte
                .get(k)
                .and_then(|v| v.first())
                .map(String::as_str)
        }
        fn alle(&self, k: &str) -> Vec<&str> {
            self.werte
                .get(k)
                .map(|v| v.iter().map(String::as_str).collect())
                .unwrap_or_default()
        }
        fn zahl(&self, k: &str, vorgabe: u64) -> u64 {
            self.eins(k).and_then(|s| s.parse().ok()).unwrap_or(vorgabe)
        }
    }

    pub(super) fn main() -> i32 {
        let argv: Vec<String> = std::env::args().collect();
        let Some(rolle) = argv.get(1) else {
            eprintln!("rolle fehlt");
            return 2;
        };
        let a = Args::lesen(&argv[2..]);
        let Some(pfad) = a.eins("log") else {
            eprintln!("--log fehlt");
            return 2;
        };
        let Ok(datei) = OpenOptions::new().create(true).append(true).open(pfad) else {
            eprintln!("log nicht offen");
            return 2;
        };
        let log = Arc::new(Log(Mutex::new(datei)));
        match rolle.as_str() {
            "server" => server(&a, &log),
            "client" => client(&a, &log),
            "roh" => roh(&a, &log),
            _ => 2,
        }
    }

    fn warte_auf(datei: &str, hoechstens: Duration) -> Option<String> {
        let ende = Instant::now() + hoechstens;
        while Instant::now() < ende {
            if let Ok(t) = std::fs::read_to_string(datei) {
                return Some(t.trim().to_owned());
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        None
    }

    fn server(a: &Args, log: &Arc<Log>) -> i32 {
        let erlaubt: Vec<Erlaubnis> = a
            .alle("erlaubt")
            .iter()
            .filter_map(|e| {
                let (sid, paket) = e.split_once(':').map_or((*e, None), |(s, p)| (s, Some(p)));
                Some(Erlaubnis::Windows {
                    sid: Sid::neu(sid).ok()?,
                    appcontainer_sid: match paket {
                        Some(p) => Some(PackageSid::neu(p).ok()?),
                        None => None,
                    },
                })
            })
            .collect();
        let Ok(name) = Kennung::neu(a.eins("name").unwrap_or("")) else {
            return 2;
        };
        let mut v = ServerVorgabe::neu(
            name,
            erlaubt,
            u8::try_from(a.zahl("instanzen", 4)).unwrap_or(0),
        );
        v.hallo_frist = Duration::from_millis(a.zahl("hallo-ms", 2000));
        v.prozess_pflicht = a.eins("pflicht").is_some();
        if let Some(job) = a.eins("job") {
            let j = JobBezug::oeffnen(job).ok();
            log.zeile(json!({"schritt": "job", "offen": j.is_some()}));
            v.job = j;
        }
        let zusatz: Vec<PackageSid> = a
            .alle("zusatz")
            .iter()
            .filter_map(|p| PackageSid::neu(p).ok())
            .collect();
        let halt = a.eins("halt").map(|d| {
            let d = d.to_owned();
            let log = Arc::clone(log);
            Box::new(move || {
                log.zeile(json!({"schritt": "halt", "wartet": d}));
                let _ = warte_auf(&d, Duration::from_secs(120));
                log.zeile(json!({"schritt": "halt", "weiter": true}));
            }) as Box<dyn Fn() + Send + Sync>
        });
        let server = match testnaht::anlegen(v, &zusatz, halt) {
            Ok(s) => s,
            Err(f) => {
                log.zeile(json!({"schritt": "anlegen", "ergebnis": "fehler", "code": f.code.als_str(), "os_fehler": f.os_fehler}));
                return 1;
            }
        };
        log.zeile(json!({"schritt": "anlegen", "ergebnis": "ok"}));
        let haken = a.eins("haken").unwrap_or("bestaetigen").to_owned();
        for _ in 0..a.zahl("anzahl", 1) {
            let start = Instant::now();
            match server.annehmen() {
                Err(f) => {
                    log.zeile(json!({"schritt": "annehmen", "ergebnis": "fehler", "code": f.code.als_str(), "os_fehler": f.os_fehler}));
                    return 1;
                }
                Ok(Annahme::Abgewiesen(e)) => {
                    log.zeile(json!({"schritt": "annahme", "ereignis": e.als_json(), "dauer_ms": ms(start.elapsed())}));
                }
                Ok(Annahme::Vorlaeufig(vl)) => {
                    let prozess = vl
                        .prozess()
                        .map(|p| json!({"pid": p.pid, "startzeit": p.startzeit}));
                    log.zeile(json!({"schritt": "vorlaeufig", "peer": vl.peer().als_json(), "prozess": prozess}));
                    let mut entscheid = haken.clone();
                    if let Some(d) = haken.strip_prefix("warte:") {
                        entscheid = warte_auf(d, Duration::from_secs(120))
                            .unwrap_or_else(|| "bestaetigen".to_owned());
                    }
                    if let Some(k) = entscheid.strip_prefix("ablehnen:") {
                        let k = Kennung::neu(k)
                            .or_else(|_| Kennung::neu("unbekannt"))
                            .expect("form");
                        let e = vl.ablehnen(Aufrufergrund(k));
                        log.zeile(json!({"schritt": "annahme", "ereignis": e.als_json()}));
                        continue;
                    }
                    match vl.bestaetigen() {
                        Err(f) => log.zeile(json!({"schritt": "bestaetigen", "ergebnis": "fehler", "code": f.code.als_str(), "os_fehler": f.os_fehler})),
                        Ok(mut verbindung) => {
                            log.zeile(json!({"schritt": "bestaetigen", "ergebnis": "ok", "peer": verbindung.peer().als_json()}));
                            let log = Arc::clone(log);
                            std::thread::spawn(move || {
                                let mut daten = Vec::new();
                                let r = verbindung.read_to_end(&mut daten);
                                log.zeile(json!({"schritt": "daten", "text": String::from_utf8_lossy(&daten), "laenge": daten.len(), "lesefehler": r.err().map(|e| e.to_string())}));
                            });
                        }
                    }
                }
            }
        }
        // The last connections finish their reads; the runner ends the process.
        std::thread::sleep(Duration::from_millis(a.zahl("nachlauf-ms", 1000)));
        0
    }

    fn client(a: &Args, log: &Arc<Log>) -> i32 {
        let (Ok(name), Some(Ok(sid))) = (
            Kennung::neu(a.eins("name").unwrap_or("")),
            a.eins("erwartet").map(Sid::neu),
        ) else {
            return 2;
        };
        let v = ClientVorgabe {
            name,
            erwartet: Erwartet::Windows { sid },
            frist: Duration::from_millis(a.zahl("frist-ms", 2000)),
            unix_ordner: None,
        };
        std::thread::sleep(Duration::from_millis(a.zahl("vorher-ms", 0)));
        let mut ok = 0;
        for i in 0..a.zahl("anzahl", 1) {
            let start = Instant::now();
            match peerkanal::verbinden(&v) {
                Err(f) => log.zeile(json!({"schritt": "verbinden", "nr": i, "ergebnis": "fehler", "code": f.code.als_str(), "os_fehler": f.os_fehler, "dauer_ms": ms(start.elapsed())})),
                Ok(mut verbindung) => {
                    ok += 1;
                    log.zeile(json!({"schritt": "verbinden", "nr": i, "ergebnis": "ok", "peer": verbindung.peer().als_json(), "dauer_ms": ms(start.elapsed())}));
                    if let Some(t) = a.eins("senden") {
                        let r = verbindung.write_all(t.as_bytes());
                        log.zeile(json!({"schritt": "senden", "nr": i, "ok": r.is_ok()}));
                    }
                    std::thread::sleep(Duration::from_millis(a.zahl("halten-ms", 0)));
                }
            }
        }
        log.zeile(json!({"schritt": "ende", "ok": ok}));
        0
    }

    fn roh(a: &Args, log: &Arc<Log>) -> i32 {
        let pfad = a.eins("pfad").unwrap_or("");
        let zugriff = u32::from_str_radix(
            a.eins("zugriff")
                .unwrap_or("80000002")
                .trim_start_matches("0x"),
            16,
        )
        .unwrap_or(0);
        // SECURITY_IDENTIFICATION (with it the standard library sets SECURITY_SQOS_PRESENT).
        let f = OpenOptions::new()
            .access_mode(zugriff)
            .share_mode(0)
            .security_qos_flags(0x0001_0000)
            .open(pfad);
        let mut f = match f {
            Err(e) => {
                log.zeile(json!({"schritt": "oeffnen", "ergebnis": "fehler", "os_fehler": e.raw_os_error()}));
                return 1;
            }
            Ok(f) => f,
        };
        log.zeile(json!({"schritt": "oeffnen", "ergebnis": "ok"}));
        if let Some(t) = a.eins("senden") {
            let t = t.replace("\\n", "\n");
            let r = f.write_all(t.as_bytes());
            log.zeile(json!({"schritt": "senden", "ok": r.is_ok(), "os_fehler": r.err().and_then(|e| e.raw_os_error())}));
        }
        let (tx, rx) = mpsc::channel();
        let Ok(mut lese) = f.try_clone() else {
            return 1;
        };
        std::thread::spawn(move || {
            let mut puffer = [0u8; 256];
            loop {
                match lese.read(&mut puffer) {
                    Ok(0) => {
                        let _ = tx.send(Err(0));
                        return;
                    }
                    Ok(n) => {
                        let _ = tx.send(Ok(puffer[..n].to_vec()));
                    }
                    Err(e) => {
                        let _ = tx.send(Err(e.raw_os_error().unwrap_or(-1)));
                        return;
                    }
                }
            }
        });
        let ende = Instant::now() + Duration::from_millis(a.zahl("lesen-ms", 1000));
        let mut empfangen = Vec::new();
        let mut schluss = Value::Null;
        loop {
            let rest = ende.saturating_duration_since(Instant::now());
            match rx.recv_timeout(rest) {
                Ok(Ok(d)) => empfangen.extend(d),
                Ok(Err(e)) => {
                    schluss = json!(e);
                    break;
                }
                Err(_) => break,
            }
        }
        log.zeile(json!({"schritt": "empfangen", "laenge": empfangen.len(), "text": String::from_utf8_lossy(&empfangen), "ende_os": schluss}));
        0
    }
}
