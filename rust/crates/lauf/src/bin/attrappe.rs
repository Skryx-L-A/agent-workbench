//! The stand-in of the tests (feature `pruefbau` only): the search process, the model path and the harness probe.
//! The behaviours are described in `attrappe/unix.rs`. Unix only: on another system the program says that it is not
//! available.

#[cfg(unix)]
#[path = "attrappe/unix.rs"]
mod unix;

#[cfg(unix)]
fn main() -> std::process::ExitCode {
    unix::main()
}

#[cfg(not(unix))]
fn main() -> std::process::ExitCode {
    use std::io::Write;
    let _ = std::io::stderr().write_all(b"lauf-attrappe: nicht verfuegbar: only on Unix systems\n");
    std::process::ExitCode::from(2)
}
