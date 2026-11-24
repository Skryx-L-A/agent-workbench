//! The test program of the crate `kernabbild`: it calls `abschirmen` (or `abschirmen_mit` with the file named after the argument `muster`), tells the
//! result on standard output (`OK`, `PROTOKOLL <text>` or `ABBRUCH <reason>`), with the argument `kind` starts a child (`sleep 30`) and tells its pid
//! (`KIND <pid>`), then says `BEREIT` and waits until standard input ends. An abort ends the program with the exit code 70. The tests of
//! `tests/abschirmen.rs` start it as a process and read `/proc/<pid>`. It is no part of a shipped build.

#[cfg(not(unix))]
fn main() {}

#[cfg(unix)]
use std::io::{BufRead, Write};
#[cfg(unix)]
use std::path::Path;
#[cfg(unix)]
use std::process::{Command, Stdio};

/// `lage [muster <file>]`: asks for the decision without effect, tells it (`LAGE <text>`) and the limit of `RLIMIT_CORE` as `GRENZE <soft> <hard>`
/// (`-` for unlimited), then ends. A call of `lage` must not change the limit.
#[cfg(unix)]
fn lage_melden(muster: Option<&String>) {
    use rustix::process::{Resource, getrlimit};
    let entscheidung = match muster {
        Some(pfad) => kernabbild::lage_mit(Path::new(pfad)),
        None => kernabbild::lage(),
    };
    let grenze = getrlimit(Resource::Core);
    let zahl = |wert: Option<u64>| wert.map_or_else(|| "-".to_owned(), |n| n.to_string());
    let mut aus = std::io::stdout();
    let _ = writeln!(aus, "LAGE {entscheidung:?}");
    let _ = writeln!(
        aus,
        "GRENZE {} {}",
        zahl(grenze.current),
        zahl(grenze.maximum)
    );
}

#[cfg(unix)]
fn main() {
    let argumente: Vec<String> = std::env::args().skip(1).collect();
    let muster = argumente
        .iter()
        .position(|a| a == "muster")
        .and_then(|stelle| argumente.get(stelle + 1));
    if argumente.iter().any(|a| a == "lage") {
        lage_melden(muster);
        return;
    }
    let ergebnis = match muster {
        Some(pfad) => kernabbild::abschirmen_mit(Path::new(pfad)),
        None => kernabbild::abschirmen(),
    };
    let mut aus = std::io::stdout();
    match ergebnis {
        Err(grund) => {
            let _ = writeln!(aus, "ABBRUCH {}", grund.grund());
            std::process::exit(70);
        }
        Ok(abgeschirmt) => {
            let _ = match abgeschirmt.hinweis {
                None => writeln!(aus, "OK"),
                Some(hinweis) => writeln!(aus, "PROTOKOLL {}", hinweis.text()),
            };
        }
    }
    let mut kind = None;
    if argumente.iter().any(|a| a == "kind")
        && let Ok(prozess) = Command::new("sleep")
            .arg("30")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .spawn()
    {
        let _ = writeln!(aus, "KIND {}", prozess.id());
        kind = Some(prozess);
    }
    let _ = writeln!(aus, "BEREIT");
    let _ = aus.flush();
    let mut zeile = String::new();
    let _ = std::io::stdin().lock().read_line(&mut zeile);
    if let Some(mut prozess) = kind {
        let _ = prozess.kill();
        let _ = prozess.wait();
    }
}
