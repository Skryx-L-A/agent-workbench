//! One Markdown heading section of a text.

use crate::text::{is_space, split_keep, strip};

/// The number of leading `#` of a heading line (after leading whitespace); zero if it is no heading.
fn heading_level(line: &str) -> Option<usize> {
    let trimmed = line.trim_start_matches(is_space);
    trimmed
        .starts_with('#')
        .then(|| trimmed.len() - trimmed.trim_start_matches('#').len())
}

/// The lower-cased heading text of a heading line with `level` hashes.
fn heading_text(line: &str, level: usize) -> String {
    let trimmed = line.trim_start_matches(is_space);
    strip(strip(&trimmed[level..]).trim_end_matches('#')).to_lowercase()
}

/// The section whose heading matches `requested`, up to the next heading of the same or a higher
/// level, with trailing whitespace removed. `requested` may carry leading `#`; case is ignored.
/// `None` when no heading matches.
#[must_use]
pub fn section(text: &str, requested: &str) -> Option<String> {
    let lines = split_keep(text);
    let wanted = strip(strip(requested).trim_start_matches('#')).to_lowercase();
    let (start, level) = lines.iter().enumerate().find_map(|(index, line)| {
        let level = heading_level(line)?;
        (heading_text(line, level) == wanted).then_some((index, level))
    })?;
    let end = lines[start + 1..]
        .iter()
        .position(|line| heading_level(line).is_some_and(|other| other <= level))
        .map_or(lines.len(), |offset| start + 1 + offset);
    Some(
        lines[start..end]
            .concat()
            .trim_end_matches(is_space)
            .to_string(),
    )
}
