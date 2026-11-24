//! Reader for Markdown vault notes: frontmatter, schema 4 lint, corpus rules, marker blocks and
//! sections.
//!
//! The crate reads and never writes. It reproduces a Python model exactly, including its quirks:
//! frontmatter values stay text, comments are cut by whitespace rules, and corpus and path rules
//! match the Python implementation. The program `notiz-kern` prints every result as one JSON line.
//!
//! The library functions work on text and paths; only [`read_text`], [`corpus`] and [`run`] touch
//! the file system, and only to read.

pub mod blocks;
pub mod cli;
pub mod corpus;
pub mod frontmatter;
pub mod note;
pub mod schema;
pub mod section;
pub mod text;

pub use blocks::{BlockState, block_of, block_state, replace_block, strip_blocks};
pub use cli::run;
pub use corpus::{Corpus, PathInfo, corpus, is_excluded, path_info, path_parts, stem_of};
pub use frontmatter::{Field, Fields, parse_fields, split_blocks};
pub use note::{Note, parse_note};
pub use schema::{Finding, LintReport, lint};
pub use section::section;
pub use text::{key_of, read_text};

#[cfg(test)]
mod tests;
