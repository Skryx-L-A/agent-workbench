//! `rangkern sitzung <vault> --socket <path> --route <key> --sitzung <key> --einstellungen <file> --protokoll <folder>`:
//! the knowledge search of one session behind a Unix socket, with the data gate in front of the output (contract 4 of the
//! walking skeleton, `rust/spec/VERTRAEGE.md`).
//!
//! The route comes from the start, never from a request. The process runs outside the confinement of the agent; only its
//! socket is bound into it. Before the readiness line it checks its start in a fixed order and ends with exit 64 (usage),
//! 78 (settings, route or log folder) or 74 (socket); exit 5 (the core image cannot be shielded) comes before all of them, from
//! `main` (`crate::abschirmung`); after it, it answers one JSON line per request line until SIGTERM or
//! SIGINT, which remove the socket and end it with exit 0.

pub mod anfrage;
pub mod einst;
pub mod konfig;
pub mod lauf;
pub mod loeschen;
pub mod mac;
pub mod probelauf;
pub mod tor;

use std::io::Write;

use crate::cli::EXIT_USAGE_SITZUNG;
use konfig::Konfig;

/// Settings, route or log folder are not usable.
pub const EXIT_CONFIG: u8 = 78;
/// The socket path is taken or the socket cannot be made.
pub const EXIT_SOCKET: u8 = 74;

/// The readiness line: exactly these 15 bytes and a line feed.
pub const BEREIT: &[u8] = b"{\"bereit\":true}\n";

/// Runs the command with the arguments after `sitzung`. Returns the exit code; after the readiness line it returns only
/// when the process fails (a signal ends it through `exit`). The shielding from core images is not a part of it: `main` takes
/// that step first (`crate::abschirmung`), so that a test that calls this function does not shield its own process.
pub fn run<O: Write, E: Write>(args: &[String], out: &mut O, err: &mut E) -> u8 {
    let Some(konfig) = Konfig::lesen(args) else {
        return EXIT_USAGE_SITZUNG;
    };
    lauf::starten(konfig, out, err)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn a_start_with_a_taken_socket_path_ends_with_74_and_a_usage_error_with_64() {
        let ordner = std::path::PathBuf::from(format!("/tmp/rkm-{}", std::process::id()));
        let _ = fs::remove_dir_all(&ordner);
        fs::create_dir_all(ordner.join("vault")).expect("vault");
        fs::create_dir_all(ordner.join("protokoll")).expect("log folder");
        fs::write(
            ordner.join("einst.json"),
            r#"{"fassung":1,"klassen":["oeffentlich","intern"],"vorgabestufe":"intern","routen":[{"schluessel":"cloud","art":"cloud","klasse":"intern"}],"kunden":[],"kundendaten_an_cloud":{"vorgabe":"nie","freigaben":[]}}"#,
        )
        .expect("settings");
        // A file at the socket path: the start ends with 74 and leaves the file as it is.
        let sock = ordner.join("s.sock");
        fs::write(&sock, "x").expect("file");
        let p = |name: &str| ordner.join(name).to_string_lossy().into_owned();
        let args: Vec<String> = [
            &p("vault"),
            "--socket",
            &p("s.sock"),
            "--route",
            "cloud",
            "--sitzung",
            "s-1",
            "--einstellungen",
            &p("einst.json"),
            "--protokoll",
            &p("protokoll"),
        ]
        .iter()
        .map(|s| (*s).to_owned())
        .collect();
        let (mut out, mut err) = (Vec::new(), Vec::new());
        assert_eq!(run(&args, &mut out, &mut err), EXIT_SOCKET);
        assert_eq!(run(&[], &mut out, &mut err), 64);
        assert_eq!(EXIT_USAGE_SITZUNG, 64);
        assert_eq!(out.len(), 0);
        assert_eq!(fs::read_to_string(&sock).expect("untouched"), "x");
        let _ = fs::remove_dir_all(&ordner);
    }
}
