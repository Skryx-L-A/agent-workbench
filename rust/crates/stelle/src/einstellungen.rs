//! The settings file (contract 2, `rust/spec/VERTRAEGE.md`) with the block `mcp` of contract 6,
//! section 2.
//!
//! The crate `einstellungen` reads the file and checks every stage. This module keeps the typed result of the
//! crate and the view of the block `mcp` that the point needs, with the deadlines as `i64`.

use std::collections::{BTreeMap, BTreeSet};

use ::einstellungen as datei;

use crate::Code;

/// One MCP server of the block `mcp`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Server {
    /// Tool names that only read. Every other name writes (contract 6, A89).
    pub lesen: BTreeSet<String>,
    /// Option "release per write call" (default on).
    pub freigabe: bool,
    /// Longest deadline for the decision of a human.
    pub frist_ms: i64,
    /// Deadline for redeeming after the release.
    pub einloesefrist_ms: i64,
}

/// A checked settings file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Einstellungen {
    /// The typed file as the crate `einstellungen` returns it: routes, models, customers.
    pub datei: datei::Einstellungen,
    /// The MCP servers by key; empty without a block `mcp`.
    pub mcp: BTreeMap<String, Server>,
}

/// A deadline of the crate (`u64`) as `i64`; a value that does not fit is refused.
fn frist(ms: u64) -> Result<i64, Code> {
    i64::try_from(ms).map_err(|_| Code::FRIST_BEREICH)
}

/// Reads and checks a settings file through the crate `einstellungen`.
///
/// # Errors
/// The code and the reason of the crate for the first violated stage (`einst.*`), also for a violation that
/// this version of the point does not know; `einst.frist_bereich` when a deadline does not fit in `i64`.
pub fn lesen(bytes: &[u8]) -> Result<Einstellungen, Code> {
    let datei = datei::lesen(bytes)?;
    let mcp = datei
        .mcp()
        .iter()
        .map(|server| {
            let eintrag = Server {
                lesen: server.lesen().iter().cloned().collect(),
                freigabe: server.freigabe(),
                frist_ms: frist(server.frist_ms())?,
                einloesefrist_ms: frist(server.einloesefrist_ms())?,
            };
            Ok((server.name().to_owned(), eintrag))
        })
        .collect::<Result<_, Code>>()?;
    Ok(Einstellungen { datei, mcp })
}

#[cfg(test)]
mod tests {
    use super::frist;
    use crate::Code;

    #[test]
    fn a_deadline_that_does_not_fit_is_refused() {
        assert_eq!(frist(0), Ok(0));
        assert_eq!(frist(60_000), Ok(60_000));
        assert_eq!(frist(i64::MAX as u64), Ok(i64::MAX));
        assert_eq!(frist(i64::MAX as u64 + 1), Err(Code::FRIST_BEREICH));
        assert_eq!(frist(u64::MAX), Err(Code::FRIST_BEREICH));
        assert_eq!(Code::FRIST_BEREICH.code(), "einst.frist_bereich");
        assert_eq!(Code::FRIST_BEREICH.grund(), "Frist ausserhalb des Bereichs");
    }
}
