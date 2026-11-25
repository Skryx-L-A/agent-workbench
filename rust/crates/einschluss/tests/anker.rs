//! The anchor ([`ANKER_SKRIPT`]) outside the enclosure, under the real `/usr/bin/sh` of the host. Each case runs the
//! script with a lifeline of its own on descriptor 5 and with `/proc/1/wchan` replaced by a file that says `do_wait`,
//! so the wait of the first step ends at once. The cases with a stand-in replace the path `/usr/bin/dd` in the
//! script, and only in the test, by a script that answers like GNU coreutils, like uutils coreutils, or wrongly. The
//! cases with the real `dd` of the host keep the script as it is, up to the wait.
//!
//! The program behind the anchor is `/usr/bin/printf gestartet`: its line says that the anchor started it.
#![cfg(target_os = "linux")]

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};

use einschluss::{ANKER_ABBRUCH, ANKER_NAME, ANKER_SKRIPT, LEINE_FD};

/// A stand-in for `dd`. It tells the three runs of the anchor apart by their operands: the probe reads
/// `/proc/self/fd/0`, the open has `count=0`, the read is the rest. `DD_MODUS` chooses the answer.
const DD_ATTRAPPE: &str = "#!/bin/sh
case \" $* \" in
  *' if=/proc/self/fd/0 '*) lauf=probe ;;
  *' count=0 '*) lauf=oeffnen ;;
  *) lauf=lesen ;;
esac
case $DD_MODUS in
  ohne_nonblock) echo \"dd: invalid input flag: 'nonblock'\" >&2; exit 1 ;;
esac
case $lauf in
  probe)
    [ \"$DD_MODUS\" = probe_falsch ] && { printf y; exit 0; }
    exec /usr/bin/head -c 1 ;;
  oeffnen)
    [ \"$DD_MODUS\" = oeffnen_scheitert ] && { echo \"dd: failed to open '/proc/self/fd/5'\" >&2; exit 1; }
    exit 0 ;;
esac
case $DD_MODUS in
  gnu) echo \"dd: error reading '/proc/self/fd/5': Resource temporarily unavailable\" >&2; exit 1 ;;
  uutils) echo 'dd: IO error: Would block' >&2; exit 1 ;;
  ende) exit 0 ;;
  daten) printf z; exit 0 ;;
  code2) exit 2 ;;
  signal) kill -KILL $$ ;;
esac
exit 3
";

/// The lifeline of a case on descriptor 5.
#[derive(Clone, Copy, Debug)]
enum Leine {
    /// An empty pipe whose write end the test holds until the anchor has ended.
    Lebt,
    /// A pipe whose write end is closed before the start (the starter died): end of file.
    Tot,
    /// A pipe with one byte in it and a living write end.
    Byte,
    /// No descriptor 5.
    Fehlt,
    /// Descriptor 5 is `/dev/null`, no pipe.
    KeinePipe,
}

/// A folder of its own for each case, removed at the end.
struct Ordner(PathBuf);

impl Ordner {
    fn neu() -> Ordner {
        static ZAEHLER: AtomicUsize = AtomicUsize::new(0);
        let pfad = std::env::temp_dir().join(format!(
            "einschluss-anker-{}-{}",
            std::process::id(),
            ZAEHLER.fetch_add(1, Ordering::SeqCst)
        ));
        std::fs::create_dir(&pfad).expect("own folder");
        Ordner(pfad)
    }
}

impl Drop for Ordner {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Which `dd` the script calls.
#[derive(Debug)]
enum Dd<'a> {
    /// `/usr/bin/dd` of the host.
    Echt,
    /// The stand-in with this mode.
    Attrappe(&'a str),
    /// A path where no program is.
    Fehlt,
}

/// Runs `skript` like the anchor runs and gives its exit code and its standard output.
fn lauf(skript: &str, dd: &Dd, leine: Leine) -> (Option<i32>, String) {
    let ordner = Ordner::neu();
    let wchan = ordner.0.join("wchan");
    std::fs::write(&wchan, "do_wait").expect("wchan");
    let mut skript = skript.replace("/proc/1/wchan", &s(&wchan));
    let mut modus = "";
    match dd {
        Dd::Echt => {}
        Dd::Attrappe(m) => {
            let attrappe = ordner.0.join("dd");
            std::fs::write(&attrappe, DD_ATTRAPPE).expect("stand-in");
            std::fs::set_permissions(
                &attrappe,
                std::os::unix::fs::PermissionsExt::from_mode(0o755),
            )
            .expect("mode");
            skript = skript.replace("/usr/bin/dd", &s(&attrappe));
            modus = m;
        }
        Dd::Fehlt => skript = skript.replace("/usr/bin/dd", &s(&ordner.0.join("kein-dd"))),
    }
    let fd = LEINE_FD;
    let (vorne, eingabe, schreiber) = match leine {
        Leine::Fehlt => (String::new(), Stdio::null(), None),
        Leine::KeinePipe => (format!("exec {fd}</dev/null\n"), Stdio::null(), None),
        Leine::Lebt | Leine::Tot | Leine::Byte => {
            let (leser, mut schreiber) = std::io::pipe().expect("pipe");
            if matches!(leine, Leine::Byte) {
                schreiber.write_all(b"z").expect("byte");
            }
            let schreiber = (!matches!(leine, Leine::Tot)).then_some(schreiber);
            // the read end moves from standard input to descriptor 5, standard input becomes empty
            (
                format!("exec {fd}<&0 0</dev/null\n"),
                Stdio::from(leser),
                schreiber,
            )
        }
    };
    let aus = Command::new("/usr/bin/sh")
        .args(["-c", &format!("{vorne}{skript}"), ANKER_NAME])
        .args(["/usr/bin/printf", "gestartet"])
        .env_clear()
        .env("DD_MODUS", modus)
        .stdin(eingabe)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .expect("sh runs");
    drop(schreiber);
    (
        aus.status.code(),
        String::from_utf8_lossy(&aus.stdout).into_owned(),
    )
}

fn s(p: &Path) -> String {
    p.to_str().expect("utf-8 path").to_owned()
}

fn startet(dd: &Dd, leine: Leine) {
    assert_eq!(
        lauf(ANKER_SKRIPT, dd, leine),
        (Some(0), "gestartet".to_owned()),
        "{dd:?} {leine:?}"
    );
}

fn bricht_ab(dd: &Dd, leine: Leine) {
    let (code, aus) = lauf(ANKER_SKRIPT, dd, leine);
    assert_eq!(code, Some(ANKER_ABBRUCH), "{dd:?} {leine:?}");
    assert!(!aus.contains("gestartet"), "{dd:?} {leine:?}: {aus}");
}

#[test]
fn the_anchor_starts_the_program_on_eagain_with_the_text_of_gnu_and_of_uutils() {
    startet(&Dd::Attrappe("gnu"), Leine::Lebt);
    startet(&Dd::Attrappe("uutils"), Leine::Lebt);
}

#[test]
fn the_anchor_ends_on_every_other_answer_of_dd() {
    for modus in [
        "ende",
        "daten",
        "code2",
        "signal",
        "ohne_nonblock",
        "probe_falsch",
        "oeffnen_scheitert",
        "unbekannt",
    ] {
        let (code, aus) = lauf(ANKER_SKRIPT, &Dd::Attrappe(modus), Leine::Lebt);
        assert_eq!(code, Some(ANKER_ABBRUCH), "{modus}");
        assert!(!aus.contains("gestartet"), "{modus}: {aus}");
    }
    bricht_ab(&Dd::Fehlt, Leine::Lebt);
}

#[test]
fn the_anchor_ends_without_a_lifeline_or_with_one_that_is_no_pipe() {
    for leine in [Leine::Fehlt, Leine::KeinePipe] {
        bricht_ab(&Dd::Attrappe("uutils"), leine);
        bricht_ab(&Dd::Echt, leine);
    }
}

#[test]
fn with_the_real_dd_only_an_empty_pipe_with_a_living_writer_starts_the_program() {
    startet(&Dd::Echt, Leine::Lebt);
    bricht_ab(&Dd::Echt, Leine::Tot);
    bricht_ab(&Dd::Echt, Leine::Byte);
}
