//! The cross test of B24 (contract 1, "Meldung an den Dienst"; contract `dienst`, section 18): the real core service runs in a
//! thread of this test (`dienst::Dienst::neu_mit`) and starts the real `werkbank-lauf` of the same build with `--meldung`, in the
//! test world of `lauf` (feature `pruefbau`, the stand-ins as search process, model path and harness).
//!
//! The service starts its children through its cover `wb-dienst --exec`. A test of another package has no `wb-dienst` of the
//! same build, so this test target has no test harness (`harness = false`): the program itself is the cover when its first
//! argument is `--exec` (it calls `dienst::programm`), and otherwise it runs the cases.

#![cfg_attr(not(all(feature = "pruefbau", target_os = "linux")), allow(dead_code))]
#![allow(clippy::print_stdout, clippy::print_stderr)]

#[cfg(all(feature = "pruefbau", target_os = "linux"))]
mod gemeinsam;

#[cfg(all(feature = "pruefbau", target_os = "linux"))]
mod kreuz {
    use std::io::{Read, Write};
    use std::os::unix::fs::PermissionsExt;
    use std::os::unix::net::UnixStream;
    use std::path::{Path, PathBuf};
    use std::time::{Duration, Instant};

    use serde_json::{Value, json};

    use crate::gemeinsam::{HAENGESPERRE_MS, Welt, datei, lebt, prozesse_unter};

    /// A viewer that speaks the frames of the service (as the viewer of the tests of `dienst`); pictures are not read.
    struct Betrachter {
        strom: UnixStream,
        puffer: Vec<u8>,
        nachrichten: Vec<Value>,
        ablehnungen: Vec<String>,
        zustaende: Vec<Value>,
    }

    impl Betrachter {
        fn neu(socket: &Path) -> Betrachter {
            let ende = Instant::now() + Duration::from_secs(5);
            let strom = loop {
                match UnixStream::connect(socket) {
                    Ok(s) => break s,
                    Err(e) => {
                        assert!(Instant::now() < ende, "connect: {e}");
                        std::thread::sleep(Duration::from_millis(10));
                    }
                }
            };
            strom
                .set_read_timeout(Some(Duration::from_millis(20)))
                .expect("timeout");
            Betrachter {
                strom,
                puffer: Vec::new(),
                nachrichten: Vec::new(),
                ablehnungen: Vec::new(),
                zustaende: Vec::new(),
            }
        }

        fn rahmen(&mut self, art: u8, nutzlast: &[u8]) {
            let n = u32::try_from(nutzlast.len() + 1).expect("size");
            let mut r = n.to_be_bytes().to_vec();
            r.push(art);
            r.extend_from_slice(nutzlast);
            self.strom.write_all(&r).expect("write");
        }

        fn ereignis(&mut self, e: &Value) {
            self.rahmen(0x01, e.to_string().as_bytes());
        }

        fn lesen(&mut self, dauer: Duration) {
            let ende = Instant::now() + dauer;
            let mut stueck = vec![0_u8; 65536];
            while Instant::now() < ende {
                match self.strom.read(&mut stueck) {
                    Ok(0) => return,
                    Ok(n) => self.puffer.extend_from_slice(&stueck[..n]),
                    Err(e)
                        if matches!(
                            e.kind(),
                            std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                        ) => {}
                    Err(_) => return,
                }
                while self.puffer.len() >= 5 {
                    let n = u32::from_be_bytes([
                        self.puffer[0],
                        self.puffer[1],
                        self.puffer[2],
                        self.puffer[3],
                    ]) as usize;
                    if self.puffer.len() < 4 + n {
                        break;
                    }
                    let art = self.puffer[4];
                    let wert: Value =
                        serde_json::from_slice(&self.puffer[5..4 + n]).unwrap_or(Value::Null);
                    self.puffer.drain(..4 + n);
                    match art {
                        0x06 => self.nachrichten.push(wert),
                        0x07 => self
                            .ablehnungen
                            .push(wert["grund"].as_str().unwrap_or("").to_owned()),
                        0x12 => self.zustaende.push(wert),
                        _ => {}
                    }
                }
            }
        }

        /// A barrier: everything the service sent before the answer is read.
        fn zustand(&mut self) {
            let vorher = self.zustaende.len();
            self.rahmen(0x12, b"");
            let ende = Instant::now() + Duration::from_secs(10);
            while self.zustaende.len() <= vorher {
                assert!(Instant::now() < ende, "no answer to Zustand");
                self.lesen(Duration::from_millis(20));
            }
        }

        /// The entry of the session `id` in the last list.
        fn eintrag(&mut self, id: &str) -> Option<Value> {
            self.zustand();
            self.nachrichten
                .iter()
                .rev()
                .find(|m| m["art"] == "Sitzungsliste")
                .and_then(|m| m["sitzungen"].as_array().cloned())
                .and_then(|l| l.into_iter().find(|s| s["id"] == id))
        }
    }

    /// The real service in a thread, its program the given `lauf`, the cover this program.
    struct Dienst {
        anhalter: dienst::Anhalter,
        faden: Option<std::thread::JoinHandle<i32>>,
        socket: PathBuf,
    }

    impl Dienst {
        fn starten(w: &Welt, programm: &Path) -> Dienst {
            let ordner = w.wurzel.join("o");
            std::fs::create_dir_all(&ordner).expect("folder");
            std::fs::set_permissions(&ordner, std::fs::Permissions::from_mode(0o700))
                .expect("mode");
            let start = json!({
                "socket": {"ordner": ordner.to_string_lossy()},
                "betrachter": {"person": "anna", "raum": "raum-nord", "ansicht": "fenster"},
                "deflate": false,
                "pruefmodus": true,
                "anfang": {
                    "personen": {"anna": {"art": "mensch", "rolle": "nutzer"}},
                    "sitzungen": []
                },
                "lauf": {
                    "programm": programm.to_string_lossy(),
                    "env": {"PATH": "/usr/bin:/bin", "LC_ALL": "C.UTF-8"},
                    "groesse": [40, 10],
                    "einstellungen": w.einstellungen.to_string_lossy(),
                }
            });
            let cfg = dienst::pruefen(&start).expect("start file of the service");
            let optionen = dienst::Optionen {
                huelle: Some(std::env::current_exe().expect("own program")),
                kern_grenze: None,
            };
            let mut d = dienst::Dienst::neu_mit(cfg, &optionen).expect("service starts");
            let anhalter = d.anhalter();
            let faden = std::thread::spawn(move || d.schleife());
            Dienst {
                anhalter,
                faden: Some(faden),
                socket: ordner.join("dienst.sock"),
            }
        }
    }

    impl Drop for Dienst {
        fn drop(&mut self) {
            self.anhalter.anhalten();
            if let Some(f) = self.faden.take() {
                let _ = f.join();
            }
        }
    }

    /// A `bwrap` stand-in whose final call becomes the harness `sleep` and writes its PID first (the ready signal).
    fn schlafender_bwrap(w: &Welt) {
        let pid = w.wurzel.join("harness.pid");
        let skript = format!(
            "#!/bin/sh\nfor a in \"$@\"; do last=\"$a\"; done\nif [ \"$last\" = /usr/bin/true ]; then exit 0; fi\necho $$ > {p}.neu\nmv {p}.neu {p}\nexec sleep 60\n",
            p = pid.display()
        );
        datei(&w.bin.join("bwrap"), skript.as_bytes(), 0o755);
    }

    /// `Starten` with the local start of the world (`pi` on `lokal-beispiel`); returns the identifier of `Gestartet`.
    fn starten(b: &mut Betrachter, w: &Welt) -> String {
        let arbeit = w.wurzel.join("home/beispiel/projekt");
        let n = b.nachrichten.len();
        b.ereignis(&json!({"art": "Starten", "start": {
            "fassung": 1, "harness": "pi", "modell": "qwen3:8b", "route": "lokal-beispiel",
            "kunde": "k-beispiel", "arbeitsordner": arbeit.to_string_lossy()}}));
        let ende = Instant::now() + Duration::from_secs(10);
        loop {
            if let Some(id) = b.nachrichten[n..]
                .iter()
                .find(|m| m["art"] == "Gestartet")
                .and_then(|m| m["sitzung"].as_str())
            {
                return id.to_owned();
            }
            assert!(Instant::now() < ende, "no Gestartet: {:?}", b.ablehnungen);
            b.lesen(Duration::from_millis(20));
        }
    }

    /// Waits until the session `id` has ended and returns its entry (the upper halt only against hanging).
    fn beendet(b: &mut Betrachter, id: &str) -> Value {
        let ende = Instant::now() + Duration::from_secs(60);
        loop {
            if let Some(e) = b.eintrag(id)
                && e["zustand"] == "beendet"
            {
                return e;
            }
            assert!(Instant::now() < ende, "{id} did not end");
            b.lesen(Duration::from_millis(50));
        }
    }

    fn welt_leer(w: &Welt) {
        let ende = Instant::now() + Duration::from_millis(HAENGESPERRE_MS);
        while !prozesse_unter(&w.wurzel).is_empty() && Instant::now() < ende {
            std::thread::sleep(Duration::from_millis(20));
        }
        assert_eq!(prozesse_unter(&w.wurzel), [], "nothing is left");
    }

    /// Case 1 as a check: the service starts `programm`; the session runs and the harness is reached while it runs. `Err` names
    /// what was seen instead (the entry of the session, when it ended).
    fn start_gelingt(w: &Welt, programm: &Path) -> Result<(), String> {
        let _ = w.lokal();
        schlafender_bwrap(w);
        let d = Dienst::starten(w, programm);
        let mut b = Betrachter::neu(&d.socket);
        let id = starten(&mut b, w);
        let pid_datei = w.wurzel.join("harness.pid");
        let ende = Instant::now() + Duration::from_secs(60);
        let harness = loop {
            if let Some(p) = std::fs::read_to_string(&pid_datei)
                .ok()
                .and_then(|t| t.trim().parse::<i32>().ok())
            {
                break p;
            }
            if let Some(e) = b.eintrag(&id)
                && e["zustand"] == "beendet"
            {
                return Err(format!("the session ended before the harness: {e}"));
            }
            assert!(Instant::now() < ende, "neither harness nor end of {id}");
            b.lesen(Duration::from_millis(50));
        };
        let e = b.eintrag(&id).expect("entry");
        if e["zustand"] != "laeuft" || !lebt(harness) {
            return Err(format!("harness {harness}, entry {e}"));
        }
        drop(b);
        drop(d);
        Ok(())
    }

    /// Case 1: the start succeeds through the real service.
    pub fn der_start_gelingt() {
        let w = Welt::neu();
        let programm = w.bin.join("werkbank-lauf");
        assert_eq!(start_gelingt(&w, &programm), Ok(()));
        welt_leer(&w);
    }

    /// Case 2: a fault before the starter reaches the list of sessions as code and reason (the model path ends before its ready
    /// line: `69 lauf.modellweg_nicht_bereit`).
    pub fn ein_fehler_vor_dem_starter_kommt_als_code_und_grund_an() {
        let w = Welt::neu();
        let _ = w.lokal();
        schlafender_bwrap(&w);
        w.verhalten("modellweg", "ende:3");
        let d = Dienst::starten(&w, &w.bin.join("werkbank-lauf"));
        let mut b = Betrachter::neu(&d.socket);
        let id = starten(&mut b, &w);
        let e = beendet(&mut b, &id);
        assert_eq!(e["code"], json!(69), "{e}");
        assert_eq!(e["grund"], json!("lauf.modellweg_nicht_bereit"), "{e}");
        assert!(!w.wurzel.join("harness.pid").exists(), "no harness");
        drop(b);
        drop(d);
        welt_leer(&w);
    }

    /// Counter probe in the test: a `lauf` that does not know `--meldung` (its step 1 refuses the call, as step 1 of the version before
    /// `--meldung` refuses three arguments) does not start through the service. Case 1 fails, and the session ends with 64 and no
    /// reason (no connection, the code is the one of the child).
    pub fn gegenprobe_ein_lauf_ohne_meldung_startet_nicht() {
        let w = Welt::neu();
        let alt = w.wurzel.join("lauf-ohne-meldung");
        let skript = format!(
            "#!/bin/sh\nexec {} \"$@\" --ohne-meldung\n",
            w.bin.join("werkbank-lauf").display()
        );
        datei(&alt, skript.as_bytes(), 0o755);
        let ergebnis = start_gelingt(&w, &alt);
        let fehler = ergebnis.expect_err("a lauf without the option must not start");
        assert!(
            fehler.contains("\"code\":64") && !fehler.contains("\"grund\""),
            "{fehler}"
        );
        welt_leer(&w);
    }
}

#[cfg(all(feature = "pruefbau", target_os = "linux"))]
fn main() {
    let args: Vec<std::ffi::OsString> = std::env::args_os().skip(1).collect();
    // the cover of the service: `wb-dienst --exec ...`
    if args.first().is_some_and(|a| a == "--exec") {
        std::process::exit(dienst::programm(&args));
    }
    let faelle: [(&str, fn()); 3] = [
        ("der_start_gelingt", kreuz::der_start_gelingt),
        (
            "ein_fehler_vor_dem_starter_kommt_als_code_und_grund_an",
            kreuz::ein_fehler_vor_dem_starter_kommt_als_code_und_grund_an,
        ),
        (
            "gegenprobe_ein_lauf_ohne_meldung_startet_nicht",
            kreuz::gegenprobe_ein_lauf_ohne_meldung_startet_nicht,
        ),
    ];
    let filter: Vec<String> = args
        .iter()
        .map(|a| a.to_string_lossy().into_owned())
        .filter(|a| !a.starts_with('-'))
        .collect();
    let mut gut = 0;
    let mut rot = Vec::new();
    for (name, fall) in faelle {
        if !filter.is_empty() && !filter.iter().any(|f| name.contains(f.as_str())) {
            continue;
        }
        if std::panic::catch_unwind(fall).is_ok() {
            println!("test {name} ... ok");
            gut += 1;
        } else {
            println!("test {name} ... FAILED");
            rot.push(name);
        }
    }
    println!(
        "test result: {}. {gut} passed; {} failed; 0 ignored; 0 measured; 0 filtered out",
        if rot.is_empty() { "ok" } else { "FAILED" },
        rot.len()
    );
    if !rot.is_empty() {
        std::process::exit(101);
    }
}

#[cfg(not(all(feature = "pruefbau", target_os = "linux")))]
fn main() {}
