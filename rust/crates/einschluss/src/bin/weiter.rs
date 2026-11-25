//! The program `einschluss-weiter`: the forwarder of the cloud exit, run inside the enclosure below `bwrap`.
//! `einschluss-weiter <port> <socket> -- <program> [argument ...]`; see `einschluss::weiter`. On a system without Unix
//! sockets the program only says so and ends with code 2; the enclosure is Linux only.

fn main() -> std::process::ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    // one body with a `cfg` block per system: cargo-mutants does not evaluate `cfg`, so a function for another system
    // alone would be changed without a build that could notice it
    #[cfg(unix)]
    {
        einschluss::weiter::starten(&args)
    }
    #[cfg(not(unix))]
    {
        use std::io::Write;
        let _ = args;
        let _ = std::io::stderr().write_all(b"einschluss-weiter: only on Unix systems\n");
        std::process::ExitCode::from(2)
    }
}
