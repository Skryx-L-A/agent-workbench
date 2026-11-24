//! The program `einschluss-weiter`: the forwarder of the cloud exit, run inside the enclosure below `bwrap`.
//! `einschluss-weiter <port> <socket> -- <program> [argument ...]`; see `einschluss::weiter`.

fn main() -> std::process::ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    einschluss::weiter::starten(&args)
}
