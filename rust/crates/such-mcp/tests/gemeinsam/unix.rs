//! The Unix-only helpers of the tests: a short socket folder below /tmp and a stand-in that listens at a socket and
//! records what it receives.

use std::fs;
use std::io::{BufRead, BufReader};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

/// A short folder below /tmp (a socket path is limited to about 104 bytes), removed again when dropped.
pub struct Wegwerf(PathBuf);

impl Wegwerf {
    pub fn neu(name: &str) -> Self {
        static ZAEHLER: AtomicUsize = AtomicUsize::new(0);
        let nummer = ZAEHLER.fetch_add(1, Ordering::SeqCst);
        let pfad = PathBuf::from(format!("/tmp/sm-{}-{nummer}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&pfad);
        fs::create_dir_all(&pfad).expect("create the test folder");
        // 0700, as the folder of a session socket under macOS (the sockets of the tests lie in it).
        fs::set_permissions(&pfad, fs::Permissions::from_mode(0o700))
            .expect("mode of the test folder");
        Self(pfad)
    }

    pub fn pfad(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }

    pub fn ordner(&self) -> &Path {
        &self.0
    }
}

impl Drop for Wegwerf {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// A listener at `pfad`; every connection runs `verhalten` in its own thread with the request line (without LF) and
/// the stream. The request lines are recorded.
pub struct StandIn {
    pub zeilen: Arc<Mutex<Vec<String>>>,
}

impl StandIn {
    pub fn start<F>(pfad: &Path, verhalten: F) -> Self
    where
        F: Fn(String, UnixStream) + Send + Sync + 'static,
    {
        let listener = UnixListener::bind(pfad).expect("bind the stand-in");
        let zeilen = Arc::new(Mutex::new(Vec::new()));
        let verhalten = Arc::new(verhalten);
        let aufgezeichnet = Arc::clone(&zeilen);
        thread::spawn(move || {
            for strom in listener.incoming() {
                let Ok(strom) = strom else { return };
                let (zeilen, verhalten) = (Arc::clone(&aufgezeichnet), Arc::clone(&verhalten));
                thread::spawn(move || {
                    let mut leser = BufReader::new(strom.try_clone().expect("clone the stream"));
                    let mut zeile = String::new();
                    if leser.read_line(&mut zeile).unwrap_or(0) == 0 {
                        return;
                    }
                    let zeile = zeile.trim_end_matches('\n').to_owned();
                    zeilen.lock().expect("lock").push(zeile.clone());
                    verhalten(zeile, strom);
                });
            }
        });
        Self { zeilen }
    }

    pub fn empfangen(&self) -> Vec<String> {
        self.zeilen.lock().expect("lock").clone()
    }
}
