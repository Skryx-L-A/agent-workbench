//! Marker blocks: a start marker and an end marker, each alone on its line.
//!
//! A marker counts only when it stands alone on its line. A block is well-formed when exactly one
//! start and exactly one end exist and the start comes first. Anything else is `Malformed` and is
//! never rewritten.

use crate::text::{split_keep, strip};

/// What a text holds structurally for one marker pair.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlockState {
    /// No marker at all.
    None,
    /// Exactly one well-formed pair.
    One,
    /// Markers exist but do not form exactly one pair.
    Malformed,
}

impl BlockState {
    /// The name used in the JSON output: `none`, `one` or `malformed`.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            BlockState::None => "none",
            BlockState::One => "one",
            BlockState::Malformed => "malformed",
        }
    }
}

/// The byte spans of the lines that consist solely of `marker`, line ending included.
fn own_line_spans(text: &str, marker: &str) -> Vec<(usize, usize)> {
    let mut offset = 0;
    let mut spans = Vec::new();
    for line in split_keep(text) {
        if strip(line) == marker {
            spans.push((offset, offset + line.len()));
        }
        offset += line.len();
    }
    spans
}

/// The end of a marker line without its newline.
fn content_end(text: &str, span: (usize, usize)) -> usize {
    text[..span.1].strip_suffix('\n').map_or(span.1, str::len)
}

/// The structural state of the marker pair in `text`.
#[must_use]
pub fn block_state(text: &str, start: &str, end: &str) -> BlockState {
    let (starts, ends) = (own_line_spans(text, start), own_line_spans(text, end));
    if starts.is_empty() && ends.is_empty() {
        BlockState::None
    } else if starts.len() == 1 && ends.len() == 1 && starts[0].0 < ends[0].0 {
        BlockState::One
    } else {
        BlockState::Malformed
    }
}

/// The byte range of the well-formed block, from the start marker to the end of the end marker
/// without its newline.
fn block_range(text: &str, start: &str, end: &str) -> Option<(usize, usize)> {
    if block_state(text, start, end) != BlockState::One {
        return None;
    }
    let open = own_line_spans(text, start)[0].0;
    let close = content_end(text, own_line_spans(text, end)[0]);
    Some((open, close))
}

/// The block including both markers, or `None` when there is no well-formed pair.
#[must_use]
pub fn block_of(text: &str, start: &str, end: &str) -> Option<String> {
    block_range(text, start, end).map(|(open, close)| text[open..close].to_string())
}

/// `text` with the block replaced by `block`; unchanged when there is no marker; `None` when the
/// markers are malformed.
#[must_use]
pub fn replace_block(text: &str, start: &str, end: &str, block: &str) -> Option<String> {
    match block_state(text, start, end) {
        BlockState::Malformed => None,
        BlockState::None => Some(text.to_string()),
        BlockState::One => block_range(text, start, end)
            .map(|(open, close)| format!("{}{block}{}", &text[..open], &text[close..])),
    }
}

/// `text` without every structural block and without the blank lines that separated it.
#[must_use]
pub fn strip_blocks(text: &str, start: &str, end: &str) -> String {
    let mut text = text.to_string();
    loop {
        let starts = own_line_spans(&text, start);
        let ends = own_line_spans(&text, end);
        let Some(opening) = starts.first() else {
            return text;
        };
        let Some(closing) = ends.iter().find(|span| span.0 >= opening.1) else {
            return text;
        };
        let head = text[..opening.0].trim_end_matches('\n');
        let tail = text[closing.1..].trim_start_matches('\n');
        let rewritten = match (head.is_empty(), tail.is_empty()) {
            (false, false) => format!("{head}\n\n{tail}"),
            (false, true) => format!("{head}\n"),
            (true, _) => tail.to_string(),
        };
        if rewritten.len().cmp(&text.len()).is_ge() {
            // Removing a block always shortens the text. A rewrite that does not cannot make
            // progress, so stop instead of looping.
            return text;
        }
        text = rewritten;
    }
}
