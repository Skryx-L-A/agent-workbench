//! The test program of the crate `kernsperre`: `kernsperre-probe <schritt> ...` runs the steps in the given order and writes one line per
//! step on standard output. The tests of `tests/sperre.rs` start it as a process and read the lines. It is no part of a shipped build.
//!
//! | Step | Line | What it does |
//! | --- | --- | --- |
//! | `sperre` | `sperre rc=0` or `sperre rc=1 grund=<reason>` | installs the lock (`kernsperre::installieren`); a refusal ends the program with 70 |
//! | `grenze` | `grenze weich=<n> hart=<n>` (`-` for unlimited) | reads `RLIMIT_CORE`. In rustix that is `prlimit64(0, …, NULL, &out)`, so it is the read case |
//! | `eins` | `eins rc=0` or `eins rc=-1 fehler=<errno>` | puts the soft limit of `RLIMIT_CORE` at 1 (at 0 when the hard limit is 0), as `kernabbild::abschirmen` does. Before the lock it works, after it is refused |
//! | `senken` | `senken rc=0` or `senken rc=-1 fehler=<errno>` | wants the soft limit of `RLIMIT_CORE` at 0 and keeps the hard one. In rustix that is `prlimit64(0, …, &new, NULL)` |
//! | `fremd` | `fremd rc=0` or `fremd rc=-1 fehler=<errno>` | writes `RLIMIT_NOFILE` with the value it has. The lock holds one resource, not all |
//! | `schale` | `schale rc=<n>` | runs `/bin/sh -c 'ulimit -c 0'`: a caller of the system, through whatever call its shell uses |
//! | `lage` | `lage seccomp=<n> nnp=<n>` | the two lines of `/proc/self/status`: 2 and 1 under the lock |
//! | `fehlfeld` | `fehlfeld wert=-1` | reads a field that `/proc/self/status` does not have, so the fallback of the reader is covered |
//! | `kind <rest>` | `kind rc=<n>` after the lines of the child | starts itself as a child with the rest of the steps and waits. Twice gives a child of a child |
//!
//! A step that is not in the table ends the program with 64.

#[cfg(not(all(
    target_os = "linux",
    any(target_arch = "x86_64", target_arch = "aarch64")
)))]
fn main() {}

#[cfg(all(
    target_os = "linux",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
fn main() -> std::process::ExitCode {
    linux::lauf(&std::env::args().skip(1).collect::<Vec<String>>())
}

#[cfg(all(
    target_os = "linux",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
mod linux {
    use std::io::Write;
    use std::process::{Command, ExitCode};

    use rustix::process::{Resource, Rlimit, getrlimit, setrlimit};

    /// The exit code of a refused lock: the program must not go on.
    const SPERRE_ABGEWIESEN: u8 = 70;
    /// The exit code of a step that does not exist.
    const SCHRITT_UNBEKANNT: u8 = 64;

    /// Writes one line on standard output. A closed output must not change the exit code.
    fn zeile(text: &str) {
        let mut aus = std::io::stdout();
        let _ = writeln!(aus, "{text}");
        let _ = aus.flush();
    }

    /// A limit as a number, `-` for unlimited.
    fn zahl(wert: Option<u64>) -> String {
        wert.map_or_else(|| "-".to_owned(), |n| n.to_string())
    }

    /// The result of a call that may be refused, as `rc=0` or `rc=-1 fehler=<errno>`.
    fn ergebnis(name: &str, antwort: Result<(), rustix::io::Errno>) {
        match antwort {
            Ok(()) => zeile(&format!("{name} rc=0")),
            Err(fehler) => zeile(&format!("{name} rc=-1 fehler={}", fehler.raw_os_error())),
        }
    }

    /// Reads a field of `/proc/self/status` as a number, `-1` when it is missing.
    fn status_feld(name: &str) -> i64 {
        let Ok(text) = std::fs::read_to_string("/proc/self/status") else {
            return -1;
        };
        text.lines()
            .find_map(|z| z.strip_prefix(name)?.trim().parse::<i64>().ok())
            .unwrap_or(-1)
    }

    /// Runs the steps in order; see the description of the program.
    pub fn lauf(schritte: &[String]) -> ExitCode {
        let mut i = 0;
        while let Some(schritt) = schritte.get(i) {
            i += 1;
            match schritt.as_str() {
                "sperre" => {
                    if let Err(grund) = kernsperre::installieren() {
                        zeile(&format!("sperre rc=1 grund={}", grund.grund()));
                        return ExitCode::from(SPERRE_ABGEWIESEN);
                    }
                    zeile("sperre rc=0");
                }
                "grenze" => {
                    let grenze = getrlimit(Resource::Core);
                    zeile(&format!(
                        "grenze weich={} hart={}",
                        zahl(grenze.current),
                        zahl(grenze.maximum)
                    ));
                }
                "eins" => {
                    let hart = getrlimit(Resource::Core).maximum;
                    // a soft limit above the hard one is refused by the kernel, so an inherited hard 0 stays 0
                    let weich = u64::from(hart != Some(0));
                    let neu = Rlimit {
                        current: Some(weich),
                        maximum: hart,
                    };
                    ergebnis("eins", setrlimit(Resource::Core, neu));
                }
                "senken" => {
                    let hart = getrlimit(Resource::Core).maximum;
                    let neu = Rlimit {
                        current: Some(0),
                        maximum: hart,
                    };
                    ergebnis("senken", setrlimit(Resource::Core, neu));
                }
                "fremd" => {
                    let gleich = getrlimit(Resource::Nofile);
                    ergebnis("fremd", setrlimit(Resource::Nofile, gleich));
                }
                "schale" => {
                    let rc = Command::new("/bin/sh")
                        .args(["-c", "ulimit -c 0"])
                        .status()
                        .ok()
                        .and_then(|s| s.code())
                        .unwrap_or(-1);
                    zeile(&format!("schale rc={rc}"));
                }
                "lage" => {
                    zeile(&format!(
                        "lage seccomp={} nnp={}",
                        status_feld("Seccomp:"),
                        status_feld("NoNewPrivs:")
                    ));
                }
                "fehlfeld" => {
                    zeile(&format!(
                        "fehlfeld wert={}",
                        status_feld("GibtEsNichtInStatus:")
                    ));
                }
                "kind" => {
                    let rc = std::env::current_exe()
                        .ok()
                        .and_then(|exe| Command::new(exe).args(&schritte[i..]).status().ok())
                        .and_then(|s| s.code())
                        .unwrap_or(-1);
                    zeile(&format!("kind rc={rc}"));
                    return ExitCode::from(u8::try_from(rc).unwrap_or(1));
                }
                anderer => {
                    zeile(&format!("unbekannt {anderer}"));
                    return ExitCode::from(SCHRITT_UNBEKANNT);
                }
            }
        }
        ExitCode::SUCCESS
    }
}
