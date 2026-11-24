//! The ranking core of `brain search` without a model in the process.
//!
//! The crate reproduces a Python model: the tokenizer with the Snowball German stemmer and
//! 4-grams of long stems, BM25 with a title bonus, the cosine list over fixed vectors, reciprocal
//! rank fusion, the navigation bonus, the demotion of derived pages, validity from the frontmatter
//! with successors, spaces, contradictions, excerpts with file lines, and the lexical fallback
//! with a full-text search that needs no `rg`.
//!
//! The corpus and every exclusion rule come from `notiz-kern`; nothing here lists a vault itself.
//! The crate only reads the vault: it never writes a note and starts no process. The program `rangkern` prints
//! every result as one JSON line. `rangkern sitzung` (Unix) is the search of one session behind a socket, with the data
//! gate `datentor` in front of the output; its only writes are its socket and, for blocked hits, the log line of
//! `formate`.

pub mod abschirmung;
pub mod bm25;
pub mod cli;
pub mod embed;
pub mod fulltext;
pub mod hit;
pub mod notes;
pub mod pystr;
pub mod search;
pub mod serve;
#[cfg(unix)]
pub mod sitzung;
pub mod snippet;
pub mod stamp;
pub mod stem;
pub mod tables;
pub mod token;
pub mod validity;
pub mod vault;
pub mod vdb;
pub mod warm;

pub use cli::{run, run_with_input};

#[cfg(test)]
mod tests;
