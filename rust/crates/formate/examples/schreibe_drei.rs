//! Writes three lines `ausgabe.gesperrt` to the chain file named by the first argument (for the hand test of
//! `kette pruefe`): `cargo run --example schreibe_drei -- <datei>`. The times are fixed, no clock is read.

use std::path::Path;
use std::process::ExitCode;

use formate::kette::{Gesperrt, anhaengen_gesperrt};

fn main() -> ExitCode {
    let Some(datei) = std::env::args_os().nth(1) else {
        return ExitCode::from(64);
    };
    let faelle = [
        (
            "kundendaten-an-cloud",
            Some("kundendaten"),
            Some("k-beispiel"),
            1,
        ),
        ("klasse-ueber-route", Some("vertraulich"), None, 3),
        ("einheit-ohne-klasse", None, None, 2),
    ];
    for (i, (grund, klasse, kunde, anzahl)) in faelle.into_iter().enumerate() {
        let g = Gesperrt {
            zeit_ms: 1_795_179_600_000 + 1000 * i64::try_from(i).unwrap_or(0),
            route: "cloud-beispiel".to_owned(),
            sitzung: "s-0001".to_owned(),
            grund: grund.to_owned(),
            klasse: klasse.map(str::to_owned),
            kunde: kunde.map(str::to_owned),
            anzahl,
        };
        if anhaengen_gesperrt(Path::new(&datei), &g).is_err() {
            return ExitCode::from(1);
        }
    }
    ExitCode::SUCCESS
}
