//! The program `werkbank-einschluss`: the fixed starter of the Linux enclosure. Exactly `selbsttest` or
//! `start --beschreibung-fd 3`; see `einschluss::starter`.

fn main() -> std::process::ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    einschluss::starter::starten(&args)
}
