//! The program "Lauf starten" (`werkbank-lauf`, contract 1, "Aufruf Lauf starten"): reads the start file of the
//! service, checks it a second time, marks the working folder (E6), starts the search process and the model path
//! outside the enclosure, bound to the harness, and starts the enclosure as its child. It stays the parent and child
//! subreaper of the starter, passes SIGHUP and SIGTERM on and ends like the starter (contract 1, "Lebensdauer").
//!
//! The steps and their codes are those of the table "Ablauf vor dem exec". Every fault before the hand-over writes
//! exactly one line `<code>: <grund>` on standard error and ends with the exit code of the table. After the hand-over
//! the exit code is the one of the starter, and a starter ended by a signal ends this process by the same signal.
//!
//! The delivered build starts the starter of the enclosure (`/usr/lib/werkbank/werkbank-einschluss start
//! --beschreibung-fd 3`) and hands the description over a pipe on descriptor 3. The test build (feature `pruefbau`)
//! calls `bwrap` directly instead; the delivered build has no way to do that.

// Unix only: the product runs on Linux; the code uses Unix sockets, permissions and signals. On another system the crate
// is empty and the program says that it is not available.
#![cfg(unix)]

pub mod ablauf;
pub mod befehl;
#[cfg(target_os = "linux")]
pub mod ca;
pub mod dateien;
pub mod fehler;
pub mod harness;
pub mod kind;
pub mod konfig;
pub mod marke;
pub mod meldung;
pub mod paket;
pub mod uebergabe;
pub mod ziele;

pub use fehler::Fehler;

/// The variables of step 0: set, also empty, they end the start (contract 1, "Schritt 0").
pub const TLS_VARIABLEN: [&str; 3] = ["SSL_CERT_FILE", "SSL_CERT_DIR", "OPENSSL_CONF"];

/// Step 0: one of the three names is set in `namen` (exact names, case counts).
pub fn tls_gesetzt<I, S>(namen: I) -> bool
where
    I: IntoIterator<Item = S>,
    S: AsRef<std::ffi::OsStr>,
{
    namen
        .into_iter()
        .any(|n| TLS_VARIABLEN.iter().any(|t| n.as_ref() == *t))
}

/// The path rules of `arbeitsordner` (contract 1): absolute, normalized, at most 1024 bytes, no control character, not
/// the root alone.
#[must_use]
pub fn ordner_form(text: &str) -> bool {
    text.len() <= 1024
        && text.len() > 1
        && text.starts_with('/')
        && !text.ends_with('/')
        && !text.contains("//")
        && text
            .split('/')
            .skip(1)
            .all(|segment| segment != "." && segment != "..")
        && !text.chars().any(|c| {
            let n = u32::from(c);
            n <= 0x1f || (0x7f..=0x9f).contains(&n)
        })
}

#[cfg(test)]
mod tests {
    use super::{ordner_form, tls_gesetzt};

    #[test]
    fn step_0_takes_exactly_the_three_names() {
        assert!(tls_gesetzt(["PATH", "SSL_CERT_FILE"]));
        assert!(tls_gesetzt(["SSL_CERT_DIR"]));
        assert!(tls_gesetzt(["OPENSSL_CONF"]));
        assert!(!tls_gesetzt([
            "SSL_CERT_FILES",
            "OPENSSL_CONFX",
            "ssl_cert_file",
            "SSL_CERT"
        ]));
        assert!(!tls_gesetzt::<[&str; 0], &str>([]));
    }

    #[test]
    fn the_folder_rules_follow_the_contract() {
        assert!(ordner_form("/a"));
        assert!(ordner_form("/srv/beispiel/.versteckt/...x"));
        assert!(ordner_form(&format!("/{}", "a".repeat(1023))));
        for schlecht in [
            "/", "", "a", "/a/", "//a", "/a//b", "/a/./b", "/a/..", "/a\nb", "/a\u{85}", "/a\u{7f}",
        ] {
            assert!(!ordner_form(schlecht), "{schlecht:?}");
        }
        assert!(!ordner_form(&format!("/{}", "a".repeat(1024))));
    }
}
