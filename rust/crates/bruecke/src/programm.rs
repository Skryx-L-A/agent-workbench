//! The program `wb-bruecke <startdatei.json>`: runs until SIGTERM or SIGINT, then closes everything, removes the token file
//! and the port file and exits with 0. Exit codes at the start: 2 call or start file, 3 rights, 1 bind or randomness; each
//! with exactly one line on stderr that begins `wb-bruecke: ` and an empty stdout.

use std::io::{self, Write};
use std::sync::Arc;
use std::thread;

use signal_hook::consts::{SIGINT, SIGTERM};
use signal_hook::iterator::Signals;

use crate::fehler::StartFehler;
use crate::server::{self, Halt};
use crate::startdatei;
use crate::verbindung::Protokoll;

/// Lets SIGTERM and SIGINT set the stop `halt` (and wake the accept loop).
///
/// # Errors
///
/// The handlers or the thread cannot be made.
pub fn signale_anmelden(halt: &Arc<Halt>) -> io::Result<()> {
    let mut signale = Signals::new([SIGTERM, SIGINT])?;
    let halt = Arc::clone(halt);
    thread::Builder::new().spawn(move || {
        for _ in signale.forever() {
            halt.anhalten();
        }
    })?;
    Ok(())
}

fn melden(fehler: &mut impl Write, e: StartFehler) -> u8 {
    let _ = writeln!(fehler, "wb-bruecke: {}", e.text);
    e.code
}

/// Runs the program: `args` without the program name. Returns the exit code.
pub fn lauf(
    args: &[String],
    aus: &mut impl Write,
    fehler: &mut impl Write,
    halt: &Halt,
    protokoll: Protokoll,
) -> u8 {
    let [startdatei] = args else {
        return melden(
            fehler,
            StartFehler {
                code: 2,
                text: "Aufruf: wb-bruecke <startdatei.json>",
            },
        );
    };
    let bruecke =
        match startdatei::lesen(startdatei).and_then(|cfg| server::starten(cfg, protokoll)) {
            Ok(b) => b,
            Err(e) => return melden(fehler, e),
        };
    let _ = writeln!(aus, "PORT {}", bruecke.port());
    let _ = aus.flush();
    bruecke.laufen(halt);
    0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pruefstand::{Ordner, anfrage};
    use std::fs;
    use std::sync::Mutex;
    use std::time::Duration;

    /// A writer the test can read while the program still runs.
    #[derive(Clone, Default)]
    struct Geteilt(Arc<Mutex<Vec<u8>>>);

    impl Write for Geteilt {
        fn write(&mut self, b: &[u8]) -> io::Result<usize> {
            self.0.lock().expect("lock").extend_from_slice(b);
            Ok(b.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    impl Geteilt {
        fn text(&self) -> String {
            String::from_utf8_lossy(&self.0.lock().expect("lock")).into_owned()
        }
    }

    fn stumm() -> Protokoll {
        Arc::new(|_| {})
    }

    fn lauf_mit(args: &[String]) -> (u8, String, String) {
        let (aus, fehler) = (Geteilt::default(), Geteilt::default());
        let code = lauf(
            args,
            &mut aus.clone(),
            &mut fehler.clone(),
            &Halt::neu(),
            stumm(),
        );
        (code, aus.text(), fehler.text())
    }

    fn eine_zeile(fehler: &str) {
        assert_eq!(fehler.lines().count(), 1, "{fehler:?}");
        assert!(
            fehler.starts_with("wb-bruecke: ") && fehler.ends_with('\n') && fehler.len() > 13,
            "{fehler:?}"
        );
    }

    #[test]
    fn a_wrong_call_is_exit_code_2_with_one_line() {
        for args in [vec![], vec!["a".to_string(), "b".to_string()]] {
            let (code, aus, fehler) = lauf_mit(&args);
            assert_eq!((code, aus.as_str()), (2, ""));
            eine_zeile(&fehler);
        }
    }

    #[test]
    fn a_bad_start_file_is_exit_code_2_and_a_bad_folder_3() {
        let o = Ordner::neu();
        let start = o.text("start.json");
        let (code, aus, fehler) = lauf_mit(std::slice::from_ref(&start));
        assert_eq!((code, aus.as_str()), (2, ""));
        eine_zeile(&fehler);
        let cfg = format!(
            r#"{{"dienst": {{"socket": "{}"}}, "port": 0, "token_datei": "{}/nicht/da/t", "deflate": true}}"#,
            o.text("d"),
            o.wurzel().display()
        );
        fs::write(&start, cfg).expect("write");
        let (code, aus, fehler) = lauf_mit(&[start]);
        assert_eq!((code, aus.as_str()), (3, ""));
        eine_zeile(&fehler);
    }

    #[test]
    fn the_program_prints_the_port_line_runs_and_ends_with_0() {
        let o = Ordner::neu();
        let (start, t, p) = (o.text("start.json"), o.text("t"), o.text("p"));
        let cfg = format!(
            r#"{{"dienst": {{"socket": "{}"}}, "port": 0, "token_datei": "{t}", "port_datei": "{p}", "deflate": false}}"#,
            o.text("d")
        );
        fs::write(&start, cfg).expect("write");
        let (aus, fehler) = (Geteilt::default(), Geteilt::default());
        let halt = Arc::new(Halt::neu());
        let (a2, f2, h2) = (aus.clone(), fehler.clone(), Arc::clone(&halt));
        let faden =
            thread::spawn(move || lauf(&[start], &mut a2.clone(), &mut f2.clone(), &h2, stumm()));
        let ende = std::time::Instant::now() + Duration::from_secs(5);
        while !aus.text().ends_with('\n') && std::time::Instant::now() < ende {
            thread::sleep(Duration::from_millis(10));
        }
        let zeile = aus.text();
        let port: u16 = zeile
            .strip_prefix("PORT ")
            .and_then(|r| r.trim_end().parse().ok())
            .expect("a PORT line");
        assert_eq!(zeile, format!("PORT {port}\n"));
        assert!(
            fs::metadata(&t).is_ok(),
            "the token file exists before the PORT line is read"
        );
        assert_eq!(
            fs::read_to_string(&p).expect("port file"),
            format!("{port}\n")
        );
        let g = anfrage(
            port,
            format!("GET / HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n\r\n").as_bytes(),
            Duration::from_secs(5),
        );
        assert_eq!(g.map(|g| g.status), Some(200));
        halt.anhalten();
        assert_eq!(crate::pruefstand::abwarten(faden), Some(0));
        assert_eq!(aus.text(), format!("PORT {port}\n"));
        assert_eq!(fehler.text(), "");
        assert!(fs::metadata(&t).is_err() && fs::metadata(&p).is_err());
    }

    #[test]
    fn a_stop_that_comes_first_ends_the_run_at_once() {
        let o = Ordner::neu();
        let start = o.text("start.json");
        let cfg = format!(
            r#"{{"dienst": {{"socket": "{}"}}, "port": 0, "token_datei": "{}", "deflate": false}}"#,
            o.text("d"),
            o.text("t")
        );
        fs::write(&start, cfg).expect("write");
        let halt = Arc::new(Halt::neu());
        halt.anhalten();
        let (aus, fehler) = (Geteilt::default(), Geteilt::default());
        let h2 = Arc::clone(&halt);
        let faden = thread::spawn(move || {
            lauf(
                &[start],
                &mut aus.clone(),
                &mut fehler.clone(),
                &h2,
                stumm(),
            )
        });
        assert_eq!(crate::pruefstand::abwarten(faden), Some(0));
        assert!(halt.ist_gestoppt());
        assert!(fs::metadata(o.pfad("t")).is_err());
    }
}
