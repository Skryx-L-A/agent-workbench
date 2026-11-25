//! The program `werkbank-halter` in every build, also without the feature `pruefbau`: step 1 refuses an argument with
//! exit 64 and the one line `halter.aufruf: falscher Aufruf`. Step 1 comes after the shielding of step 0 and before any
//! read, so the program reaches no path of the account here (no seam is needed).
#![cfg(target_os = "linux")]

use std::io::Read;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[test]
fn ein_argument_endet_vor_jedem_lesen_mit_64() {
    let mut kind = Command::new(env!("CARGO_BIN_EXE_werkbank-halter"))
        .arg("x")
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn");
    let ende = Instant::now() + Duration::from_secs(20);
    let st = loop {
        if let Some(st) = kind.try_wait().expect("wait") {
            break st;
        }
        if Instant::now() > ende {
            let _ = kind.kill();
            let _ = kind.wait();
            panic!("the holder did not end within 20 s");
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    let mut e = String::new();
    kind.stderr
        .take()
        .expect("stderr")
        .read_to_string(&mut e)
        .expect("read");
    let mut o = String::new();
    kind.stdout
        .take()
        .expect("stdout")
        .read_to_string(&mut o)
        .expect("read");
    assert_eq!(st.code(), Some(64), "stderr: {e}");
    assert_eq!(e, "halter.aufruf: falscher Aufruf\n");
    assert_eq!(o, "");
}
