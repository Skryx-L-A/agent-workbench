//! The program `datentor`: the case runner of the contract (section 5). The gate itself is the library; this program only
//! reads one case as JSON from stdin and writes the answers. See `programm.rs`.

mod ohne_doppel;
mod programm;
mod programm_klassen;
mod programm_zonen;

use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args_os()
        .skip(1)
        .map(|a| a.to_string_lossy().into_owned())
        .collect();
    let code = programm::lauf(
        &args,
        std::io::stdin().lock(),
        &mut std::io::stdout().lock(),
        &mut std::io::stderr().lock(),
    );
    ExitCode::from(code)
}
