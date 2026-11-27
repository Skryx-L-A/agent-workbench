//! The exit codes of `werkbank-vorgabe` without a socket (rust/spec/paket/README.md, part A3): 64 with arguments, 78
//! without a socket from systemd. The program never binds a socket itself.

use std::process::Command;

fn programm() -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_werkbank-vorgabe"));
    c.env_remove("LISTEN_FDS")
        .env_remove("LISTEN_PID")
        .env_remove("LISTEN_FDNAMES");
    c
}

#[test]
fn ohne_socket_und_mit_argumenten() {
    let aus = programm().output().expect("start");
    assert_eq!(aus.status.code(), Some(78));
    assert_eq!(aus.stdout.len(), 0, "nothing on stdout");
    let text = String::from_utf8_lossy(&aus.stderr);
    assert!(text.starts_with("werkbank-vorgabe: "), "{text}");
    assert!(text.ends_with(", Code 78\n"), "{text}");
    let aus = programm().arg("--hilfe").output().expect("start");
    #[cfg(target_os = "linux")]
    {
        assert_eq!(aus.status.code(), Some(64));
        assert_eq!(
            String::from_utf8_lossy(&aus.stderr),
            "werkbank-vorgabe: Aufruf ohne Argumente, Code 64\n"
        );
        // LISTEN_FDS of another process counts as no socket
        let aus = programm()
            .env("LISTEN_FDS", "1")
            .env("LISTEN_PID", "1")
            .output()
            .expect("start");
        assert_eq!(aus.status.code(), Some(78));
        assert_eq!(
            String::from_utf8_lossy(&aus.stderr),
            "werkbank-vorgabe: nicht genau ein Socket von systemd, Code 78\n"
        );
    }
    #[cfg(not(target_os = "linux"))]
    assert_eq!(aus.status.code(), Some(78));
}
