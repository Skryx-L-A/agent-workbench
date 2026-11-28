//! `werkbank-stelle --konfig <pfad>`: the decision point with its shell (contract 6, sections 13 and 14). It returns only on
//! a failure of the start: exit code 78 (configuration, settings, key, store or chain), 74 (a channel), 70 (the loop
//! ended), 64 (call), with the fixed reason on stderr.

use std::process::ExitCode;

#[cfg(unix)]
fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    let [_, schalter, pfad] = args.as_slice() else {
        return fehler(64, "aufruf");
    };
    if schalter != "--konfig" {
        return fehler(64, "aufruf");
    }
    let zweiter = cfg!(feature = "pruefbau").then(|| rustix::process::geteuid().as_raw());
    let (code, grund) = stelle::huelle::laufen(std::path::Path::new(pfad), zweiter);
    fehler(code, grund)
}

/// Under Windows the shell is not built yet (open): the point does not start.
#[cfg(not(unix))]
fn main() -> ExitCode {
    fehler(78, "plattform_gesperrt")
}

fn fehler(code: u8, grund: &str) -> ExitCode {
    #[allow(
        clippy::print_stderr,
        reason = "the operating log of the point is stderr"
    )]
    {
        eprintln!("werkbank-stelle: {grund}");
    }
    ExitCode::from(code)
}
