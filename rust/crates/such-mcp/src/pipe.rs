//! `such-mcp` under Windows (addendum to contract 4 for Windows, section 6): the search of the session behind the named pipe
//! `\\.\pipe\wb-such-<32 hex>`, reached through the crate `peerkanal`.
//!
//! The rule of the name is a pure function, so it is tested on every system. The exchange exists for Windows alone:
//! `peerkanal::verbinden` opens with `GENERIC_READ | FILE_WRITE_DATA` and identification only, waits for a free instance within
//! the deadline, checks the owner of the instance against the own SID before any byte, sends the hello `pk1` and waits for
//! the same hello; then one request line goes out and one answer line comes back.

/// The fixed start of the pipe name; 32 characters of `0-9a-f` follow.
pub const PIPE_PRAEFIX: &str = r"\\.\pipe\wb-such-";

/// Whether `name` is a pipe name of the form of the addendum: exactly `\\.\pipe\wb-such-` and 32 characters of `0-9a-f`. A file
/// path, a remote name, capital letters or another length are refused.
#[must_use]
pub fn name_ok(name: &str) -> bool {
    name.strip_prefix(PIPE_PRAEFIX).is_some_and(|zufall| {
        zufall.len() == 32
            && zufall
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    })
}

#[cfg(windows)]
pub(crate) mod tausch {
    use std::io::{Read, Write};
    use std::path::Path;
    use std::time::{Duration, Instant};

    use peerkanal::{ClientVorgabe, Erwartet, Kennung, Sid};

    use crate::suche::Unerreichbar;

    /// One question over the pipe `name`: the server check and the hello of `peerkanal`, then the request line, then the answer
    /// line without its LF. The expected owner of the instance is the own SID, never a value from argv, the environment or a
    /// setting.
    pub fn austausch(
        name: &Path,
        zeile: &str,
        frist: Duration,
        max: usize,
    ) -> Result<String, Unerreichbar> {
        let ende = Instant::now() + frist;
        let kanal = name
            .to_str()
            .filter(|name| super::name_ok(name))
            .and_then(|name| name.strip_prefix(r"\\.\pipe\"))
            .and_then(|kanal| Kennung::neu(kanal).ok())
            .ok_or(Unerreichbar)?;
        let eigene = eigene_sid().ok_or(Unerreichbar)?;
        let rest = ende
            .checked_duration_since(Instant::now())
            .filter(|rest| !rest.is_zero())
            .ok_or(Unerreichbar)?;
        let vorgabe = ClientVorgabe {
            name: kanal,
            erwartet: Erwartet::Windows { sid: eigene },
            frist: rest,
            unix_ordner: None,
        };
        let mut verbindung = peerkanal::verbinden(&vorgabe).map_err(|_| Unerreichbar)?;
        let mut anfrage = Vec::with_capacity(zeile.len() + 1);
        anfrage.extend_from_slice(zeile.as_bytes());
        anfrage.push(b'\n');
        verbindung.write_all(&anfrage).map_err(|_| Unerreichbar)?;
        // The read has no deadline of its own; the caller's `recv_timeout` ends the call at the deadline.
        let mut antwort: Vec<u8> = Vec::new();
        let mut stueck = [0u8; 8192];
        loop {
            let n = match verbindung.read(&mut stueck) {
                Ok(0) | Err(_) => return Err(Unerreichbar),
                Ok(n) => n,
            };
            if let Some(lf) = stueck[..n].iter().position(|&b| b == b'\n') {
                antwort.extend_from_slice(&stueck[..lf]);
                break;
            }
            antwort.extend_from_slice(&stueck[..n]);
            if antwort.len() > max {
                return Err(Unerreichbar);
            }
        }
        if antwort.len() > max {
            return Err(Unerreichbar);
        }
        String::from_utf8(antwort).map_err(|_| Unerreichbar)
    }

    /// The `TokenUser` SID of this process, the only owner the server may have (section 6). `peerkanal` (version 1) offers no
    /// call for it, and this program reads it neither from argv nor from the environment, opens no file and starts no process
    /// (test `the_source_calls_no_environment_file_process_or_network_function`). Without it no question is sent: every call is
    /// "Suche nicht erreichbar" (fail closed) until `peerkanal` names the own SID.
    fn eigene_sid() -> Option<Sid> {
        None
    }
}
