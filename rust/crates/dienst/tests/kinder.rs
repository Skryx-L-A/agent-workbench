//! The fixed child programs (`wb-dienst --kind <name> ...`, DI8) with exact bytes. They run on pipes here, not in a PTY: `stty`
//! finds no terminal and does nothing, the rest of the program does not care. Every child writes `BEREIT\r\n` first and reads one
//! start byte; the rest of the input feeds what it reads next. Nothing here depends on the time of day.
#![cfg(unix)] // the crate is Unix only (see `src/lib.rs`)

use std::fmt::Write as _;
use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const DIENST: &str = env!("CARGO_BIN_EXE_wb-dienst");

/// Runs a child with `eingabe` on standard input (then the end of the input) and returns the exit code and standard output.
fn kind(args: &[&str], eingabe: &[u8]) -> (Option<i32>, Vec<u8>) {
    kind_mit_env(args, eingabe, &[])
}

fn kind_mit_env(args: &[&str], eingabe: &[u8], env: &[(&str, &str)]) -> (Option<i32>, Vec<u8>) {
    let mut befehl = Command::new(DIENST);
    befehl
        .arg("--kind")
        .args(args)
        .env_clear()
        .envs(env.iter().copied())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    let mut kind = befehl.spawn().expect("child starts");
    let mut stdin = kind.stdin.take().expect("stdin");
    let daten = eingabe.to_vec();
    let schreiber = std::thread::spawn(move || {
        let _ = stdin.write_all(&daten);
    });
    let mut aus = Vec::new();
    kind.stdout
        .take()
        .expect("stdout")
        .read_to_end(&mut aus)
        .expect("read");
    let status = kind.wait().expect("wait");
    let _ = schreiber.join();
    (status.code(), aus)
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

#[test]
fn unbekannter_name_endet_mit_64_ohne_ausgabe() {
    let (code, aus) = kind(&["nirgends"], b"g");
    assert_eq!(code, Some(64));
    assert_eq!(aus, Vec::<u8>::new());
}

#[test]
fn ausgabe_und_ende() {
    let (code, aus) = kind(&["ausgabe", "48414c4c4f0d0a"], b"g");
    assert_eq!(
        (code, text(&aus)),
        (Some(0), "BEREIT\r\nHALLO\r\n".to_owned())
    );
    let (code, aus) = kind(&["ende", "7"], b"g");
    assert_eq!((code, text(&aus)), (Some(7), "BEREIT\r\n".to_owned()));
    let (code, _) = kind(&["ende", "0"], b"g");
    assert_eq!(code, Some(0));
    let (code, _) = kind(&["ende", "300"], b"g");
    assert_eq!(code, Some(44), "the exit code is a byte");
}

#[test]
fn nichts_wird_vor_dem_startbyte_getan() {
    // the end of the input without a start byte: the child reads nothing and ends after BEREIT, without its action
    let (code, aus) = kind(&["ausgabe", "4142"], b"");
    assert_eq!((code, text(&aus)), (Some(0), "BEREIT\r\nAB".to_owned()));
}

#[test]
fn geteilt_schreibt_zwei_stuecke() {
    let (code, aus) = kind(&["geteilt", "41", "42"], b"g");
    assert_eq!((code, text(&aus)), (Some(0), "BEREIT\r\nAB".to_owned()));
    let t = Instant::now();
    let (_, aus) = kind(&["geteilt", "41", "42", "0.3"], b"g");
    assert_eq!(text(&aus), "BEREIT\r\nAB");
    assert!(
        t.elapsed() >= Duration::from_millis(300),
        "waits after the second piece"
    );
}

#[test]
fn ausgabewarte_wartet() {
    let t = Instant::now();
    let (code, aus) = kind(&["ausgabewarte", "4142", "0.3"], b"g");
    assert_eq!((code, text(&aus)), (Some(0), "BEREIT\r\nAB".to_owned()));
    assert!(t.elapsed() >= Duration::from_millis(300));
}

#[test]
fn zeilen_zaehlt_mit_sechs_ziffern() {
    let (code, aus) = kind(&["zeilen", "3"], b"g");
    assert_eq!(
        (code, text(&aus)),
        (
            Some(0),
            "BEREIT\r\nZ000001\r\nZ000002\r\nZ000003\r\n".to_owned()
        )
    );
    let (_, aus) = kind(&["zeilen", "0"], b"g");
    assert_eq!(text(&aus), "BEREIT\r\n");
}

#[test]
fn eingabe_und_klammer_geben_die_bytes_als_hex_zurueck() {
    let (code, aus) = kind(&["eingabe", "3"], b"gabc");
    assert_eq!(
        (code, text(&aus)),
        (Some(0), "BEREIT\r\nEINGABE 616263\r\n".to_owned())
    );
    let (_, aus) = kind(&["eingabe", "5"], b"gab");
    assert_eq!(
        text(&aus),
        "BEREIT\r\nEINGABE 6162\r\n",
        "the end of the input ends the read"
    );
    let (code, aus) = kind(&["klammer", "2"], b"g\x01\xff");
    assert_eq!(
        (code, text(&aus)),
        (
            Some(0),
            "BEREIT\r\n\x1b[?2004hKLAMMER\r\nEINGABE 01ff\r\n".to_owned()
        )
    );
}

#[test]
fn bilder_zeichnet_je_byte_ein_bild() {
    let (code, aus) = kind(&["bilder", "2"], b"gxxx");
    let x40 = "x".repeat(40);
    let bild = |k: u32| {
        let mut b = String::from("\x1b[H");
        for j in 0..4 {
            let _ = write!(b, "Bild {k:04} Zeile {j} {x40}\r\n");
        }
        b
    };
    assert_eq!(
        (code, text(&aus)),
        (Some(0), format!("BEREIT\r\n{}{}ENDE\r\n", bild(1), bild(2)))
    );
    let t = Instant::now();
    let (_, aus) = kind(&["bilder", "0", "0.3"], b"gx");
    assert_eq!(text(&aus), "BEREIT\r\nENDE\r\n");
    assert!(t.elapsed() >= Duration::from_millis(300));
}

#[test]
fn groesse_schreibt_die_zeile_mit_der_groesse() {
    // no terminal on a pipe, so `stty size` answers nothing; the line has its form
    let (code, aus) = kind(&["groesse"], b"g");
    assert_eq!(code, Some(0));
    assert!(
        text(&aus).starts_with("BEREIT\r\nGROESSE "),
        "{:?}",
        text(&aus)
    );
    assert!(text(&aus).ends_with("\r\n"));
}

#[test]
fn umgebung_zeigt_nur_was_gesetzt_ist() {
    let (code, aus) = kind_mit_env(
        &["umgebung", "A", "FEHLT", "B"],
        b"g",
        &[
            ("A", "eins"),
            ("B", ""),
            ("SDKROOT", "x"),
            ("CPATH", "y"),
            ("MANPATH", "z"),
            ("LIBRARY_PATH", "w"),
            ("__CF_USER_TEXT_ENCODING", "v"),
        ],
    );
    let ordner = std::env::current_dir().expect("cwd").display().to_string();
    assert_eq!(
        (code, text(&aus)),
        (
            Some(0),
            format!("BEREIT\r\nA=eins\r\nFEHLT=-\r\nB=\r\nCWD={ordner}\r\nANZAHL=2\r\n")
        ),
        "the five variables of the Xcode wrapper are not counted"
    );
    let (_, aus) = kind_mit_env(&["umgebung"], b"g", &[]);
    assert!(text(&aus).ends_with("ANZAHL=0\r\n"), "{:?}", text(&aus));
}

#[test]
fn titel_schreibt_osc_2_und_fertig() {
    let (code, aus) = kind(&["titel", "Mein Titel"], b"g");
    assert_eq!(
        (code, text(&aus)),
        (
            Some(0),
            "BEREIT\r\n\x1b]2;Mein Titel\x07fertig\r\n".to_owned()
        )
    );
}

/// A child on pipes whose input stays open: the output so far can be looked at between two bytes of input.
struct Offen {
    kind: std::process::Child,
    stdin: std::process::ChildStdin,
    aus: std::sync::Arc<std::sync::Mutex<Vec<u8>>>,
}

impl Offen {
    fn neu(args: &[&str]) -> Self {
        let mut kind = Command::new(DIENST)
            .arg("--kind")
            .args(args)
            .env_clear()
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("child starts");
        let stdin = kind.stdin.take().expect("stdin");
        let mut stdout = kind.stdout.take().expect("stdout");
        let aus = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let sammler = std::sync::Arc::clone(&aus);
        std::thread::spawn(move || {
            let mut puffer = [0_u8; 4096];
            while let Ok(n) = stdout.read(&mut puffer) {
                if n == 0 {
                    break;
                }
                sammler
                    .lock()
                    .expect("lock")
                    .extend_from_slice(&puffer[..n]);
            }
        });
        Self { kind, stdin, aus }
    }

    fn senden(&mut self, bytes: &[u8]) {
        self.stdin.write_all(bytes).expect("write");
    }

    /// Waits until the output is `soll`; fails if it is something else for good (after 5 s).
    fn warte_auf(&self, soll: &str) {
        let frist = Instant::now() + Duration::from_secs(5);
        loop {
            let ist = text(&self.aus.lock().expect("lock"));
            if ist == soll {
                return;
            }
            assert!(Instant::now() < frist, "output {ist:?} instead of {soll:?}");
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    /// After 300 ms the output is still `soll` (the child waits for a byte).
    fn bleibt(&self, soll: &str) {
        std::thread::sleep(Duration::from_millis(300));
        assert_eq!(text(&self.aus.lock().expect("lock")), soll);
    }
}

impl Drop for Offen {
    fn drop(&mut self) {
        let _ = self.kind.kill();
        let _ = self.kind.wait();
    }
}

#[test]
fn stufen_schreibt_ein_stueck_je_byte() {
    let mut k = Offen::neu(&["stufen", "41", "42", "43"]);
    k.warte_auf("BEREIT\r\n");
    k.bleibt("BEREIT\r\n");
    k.senden(b"g");
    k.warte_auf("BEREIT\r\nA");
    k.bleibt("BEREIT\r\nA");
    k.senden(b"x");
    k.warte_auf("BEREIT\r\nAB");
    k.bleibt("BEREIT\r\nAB");
    k.senden(b"x");
    k.warte_auf("BEREIT\r\nABC");
    assert_eq!(k.kind.wait().expect("wait").code(), Some(0));
}

#[test]
fn fuellen_schreibt_bilder_bekannter_groesse() {
    let (code, aus) = kind(&["fuellen", "3", "2", "1"], b"gxxx");
    assert_eq!(
        (code, text(&aus)),
        (
            Some(0),
            "BEREIT\r\n\x1b[Haaa\x1b[Hbb\x1b[Hc\x1b[2;1HENDE\r\n".to_owned()
        )
    );
    // the letter changes per picture and wraps after z
    let mut args = vec!["fuellen"];
    args.extend(std::iter::repeat_n("1", 27));
    let (_, aus) = kind(&args, &[b'g'; 40]);
    let t = text(&aus);
    assert!(t.contains("\x1b[Hz\x1b[Ha\x1b[2;1HENDE"), "{t:?}");
    // one picture per byte
    let mut k = Offen::neu(&["fuellen", "2", "1"]);
    k.warte_auf("BEREIT\r\n");
    k.senden(b"g");
    k.warte_auf("BEREIT\r\n\x1b[Haa");
    k.bleibt("BEREIT\r\n\x1b[Haa");
    k.senden(b"x");
    k.warte_auf("BEREIT\r\n\x1b[Haa\x1b[Hb");
    k.bleibt("BEREIT\r\n\x1b[Haa\x1b[Hb");
    k.senden(b"x");
    k.warte_auf("BEREIT\r\n\x1b[Haa\x1b[Hb\x1b[2;1HENDE\r\n");
    // a prelude after the start byte
    let (_, aus) = kind(&["fuellen", "@4142", "1"], b"gx");
    assert_eq!(
        text(&aus),
        "BEREIT\r\n".to_owned() + "AB\x1b[Ha\x1b[2;1HENDE\r\n"
    );
}

#[test]
fn osc52viele_schreibt_zwischenablagen() {
    let (code, aus) = kind(&["osc52viele", "2", "3"], b"g");
    assert_eq!(
        (code, text(&aus)),
        (
            Some(0),
            "BEREIT\r\n\x1b]52;c;a2tr\x07\x1b]52;c;a2tr\x07ENDE\r\n".to_owned()
        )
    );
    // the bytes begin with the word, the length is b
    let (_, aus) = kind(&["osc52viele", "1", "5", "AB"], b"g");
    assert_eq!(text(&aus), "BEREIT\r\n\x1b]52;c;QUJra2s=\x07ENDE\r\n");
    let (_, aus) = kind(&["osc52viele", "1", "2", "ABCDEF"], b"g");
    assert_eq!(
        text(&aus),
        "BEREIT\r\n\x1b]52;c;QUI=\x07ENDE\r\n",
        "cut to b bytes"
    );
    let (_, aus) = kind(&["osc52viele", "0", "3"], b"g");
    assert_eq!(text(&aus), "BEREIT\r\nENDE\r\n");
}

#[test]
fn warte_schreibt_die_prozessnummer() {
    let t = Instant::now();
    let (code, aus) = kind(&["warte", "0.2"], b"g");
    let s = text(&aus);
    assert_eq!(code, Some(0));
    assert!(s.starts_with("BEREIT\r\nPID="), "{s:?}");
    let pid: u32 = s["BEREIT\r\nPID=".len()..].trim_end().parse().expect("pid");
    assert!(pid > 1);
    assert!(t.elapsed() >= Duration::from_millis(200));
}

#[test]
fn signal_beendet_das_kind_durch_das_signal() {
    use std::os::unix::process::ExitStatusExt;
    let mut kind = Command::new(DIENST)
        .args(["--kind", "signal", "15"])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("starts");
    kind.stdin
        .as_mut()
        .expect("stdin")
        .write_all(b"g")
        .expect("write");
    let status = kind.wait().expect("wait");
    assert_eq!(status.signal(), Some(15));
    let mut kind = Command::new(DIENST)
        .args(["--kind", "signal", "9"])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("starts");
    kind.stdin
        .as_mut()
        .expect("stdin")
        .write_all(b"g")
        .expect("write");
    assert_eq!(kind.wait().expect("wait").signal(), Some(9));
}

#[test]
fn still_schreibt_nichts_und_liest_nichts() {
    let t = Instant::now();
    let (code, aus) = kind(&["still", "0.2"], b"g");
    assert_eq!((code, aus.len()), (Some(0), 0));
    assert!(t.elapsed() >= Duration::from_millis(200));
}

#[test]
fn huelle_laesst_shell_weg_ausser_sie_ist_gewollt() {
    let ohne = |flag: &str| {
        let aus = Command::new(DIENST)
            .args(["--exec", flag, "/usr/bin/env"])
            .env_clear()
            .env("A", "1")
            .env("SHELL", "/bin/sh")
            .output()
            .expect("runs");
        let mut zeilen: Vec<String> = text(&aus.stdout).lines().map(str::to_owned).collect();
        zeilen.sort();
        zeilen
    };
    assert_eq!(ohne("ohne-shell"), vec!["A=1"]);
    assert_eq!(ohne("mit-shell"), vec!["A=1", "SHELL=/bin/sh"]);
}
