//! The excerpt of a hit: the block with the most query stems, with its file lines, and the hook line.

use std::collections::HashSet;

use notiz_kern::text::{is_space, strip};

use crate::token::{Analyzer, tokenize};

/// A text and the first and last file line (1-based) it comes from.
pub type Excerpt = (String, Option<(usize, usize)>);

/// Lines that open a recall hook, in the two spellings of the vault.
const HOOK_PREFIXES: [&str; 2] = ["Für künftige Sessions", "Fuer kuenftige Sessions"];
/// At most this many sentences of a block.
const MAX_SENTENCES: usize = 3;
/// A longer excerpt is cut at a word boundary.
const MAX_CHARS: usize = 220;

/// Function words that do not count for the choice of the block (German and English).
const STOPWORDS: &str = "
der die das den dem des ein eine einer eines einem einen und oder aber doch auch noch nur
schon nicht kein keine nie immer sehr mehr viel viele wenig ich du er sie es wir ihr man
mich dich sich uns euch mir dir ihm ihr ihnen mein dein sein ihr unser euer diese dieser
dieses jener jene jenes welche welcher welches wer was wie wo wann warum wieso weshalb wozu
wobei womit ist sind war waren wird werden wurde wurden bin bist seid habe hast hat haben
hatte hatten kann kannst koennen konnte koennte muss musst muessen musste sollte sollten
will willst wollen wollte darf duerfen durfte in im an am auf aus bei mit nach seit von vor
zu zur zum ueber unter zwischen durch fuer gegen ohne um als wenn weil dass ob damit sodass
so dann daher deshalb also hier da dort dabei dazu davon dafuer dagegen einem einer eines
the a an and or but not no yes is are was were be been being do does did have has had will
would shall should can could may might must this that these those i you he she it we they me
him her us them my your his its our their to of in on at by for with from as about into over
under between through if then so because than too very just also only
";

/// The lines of a file as an editor counts them: split at `\n` only, a trailing `\r` removed.
pub(crate) fn file_lines(text: &str) -> Vec<&str> {
    text.split('\n')
        .map(|line| line.trim_end_matches('\r'))
        .collect()
}

/// The index of the first line after the frontmatter, or 0 without a closed frontmatter.
pub(crate) fn body_start(lines: &[&str]) -> usize {
    if lines.first().is_some_and(|line| strip(line) == "---")
        && let Some(end) = lines[1..].iter().position(|line| strip(line) == "---")
    {
        return end + 2;
    }
    0
}

/// A blank-line-separated block of the body: nearest heading, joined text, first and last line.
pub(crate) struct Block {
    pub(crate) heading: String,
    pub(crate) text: String,
    pub(crate) first: usize,
    pub(crate) last: usize,
}

/// Closes the block collected in `buffer` (if any) under `heading`.
fn flush(buffer: &mut Vec<(usize, &str)>, heading: &str, blocks: &mut Vec<Block>) {
    if let (Some(first), Some(last)) = (buffer.first(), buffer.last()) {
        blocks.push(Block {
            heading: heading.to_string(),
            text: buffer
                .iter()
                .map(|&(_, line)| line)
                .collect::<Vec<_>>()
                .join(" "),
            first: first.0 + 1,
            last: last.0 + 1,
        });
    }
    buffer.clear();
}

/// The blocks of the body (frontmatter skipped), lines 1-based.
pub(crate) fn body_blocks(text: &str) -> Vec<Block> {
    let lines = file_lines(text);
    let mut blocks = Vec::new();
    let mut heading = String::new();
    let mut buffer: Vec<(usize, &str)> = Vec::new();
    for (i, raw) in lines.iter().enumerate().skip(body_start(&lines)) {
        let line = strip(raw);
        if line.starts_with('#') {
            flush(&mut buffer, &heading, &mut blocks);
            heading = strip(line.trim_start_matches('#')).to_string();
        } else if line.is_empty() {
            flush(&mut buffer, &heading, &mut blocks);
        } else {
            buffer.push((i, line));
        }
    }
    flush(&mut buffer, &heading, &mut blocks);
    blocks
}

/// The first hook line (`Für künftige Sessions ...`), else the first body line that is neither
/// empty nor a heading; with its file line.
#[must_use]
pub fn hook_snippet_span(text: &str) -> Excerpt {
    let lines = file_lines(text);
    let start = body_start(&lines);
    for (i, raw) in lines.iter().enumerate().skip(start) {
        let line = strip(raw);
        if HOOK_PREFIXES.iter().any(|prefix| line.starts_with(prefix)) {
            return (line.to_string(), Some((i + 1, i + 1)));
        }
    }
    for (i, raw) in lines.iter().enumerate().skip(start) {
        let line = strip(raw);
        if !line.is_empty() && !line.starts_with('#') {
            return (line.to_string(), Some((i + 1, i + 1)));
        }
    }
    (String::new(), None)
}

/// Python's `re.split(r"(?<=[.!?])\s+", text)`, without the empty parts.
pub(crate) fn sentences(text: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut start = 0;
    let mut previous: Option<char> = None;
    let mut chars = text.char_indices().peekable();
    while let Some((i, c)) = chars.next() {
        if is_space(c) && matches!(previous, Some('.' | '!' | '?')) {
            let mut end = i + c.len_utf8();
            while let Some((j, next)) = chars.next_if(|&(_, next)| is_space(next)) {
                end = j + next.len_utf8();
            }
            parts.push(&text[start..i]);
            start = end;
            previous = None;
        } else {
            previous = Some(c);
        }
    }
    parts.push(&text[start..]);
    parts.retain(|part| !part.is_empty());
    parts
}

/// Cuts `text` to at most 220 code points at a word boundary and appends `...`.
pub(crate) fn clip(text: &str) -> String {
    if text.chars().count() <= MAX_CHARS {
        return text.to_string();
    }
    let cut: String = text.chars().take(MAX_CHARS).collect();
    let head = cut.rsplit_once(' ').map_or(cut.as_str(), |(head, _)| head);
    format!("{}...", head.trim_end_matches([',', ';', ':']))
}

/// Whether `word` is one of the function words.
pub(crate) fn is_stopword(word: &str) -> bool {
    STOPWORDS.split_whitespace().any(|stop| stop == word)
}

/// The block with the most stems in common with `query` (heading and up to three sentences, cut
/// to 220 code points) and the file lines of that block; the hook line when no block shares a
/// stem with the query or the query has only function words.
pub fn best_snippet_span(analyzer: &mut Analyzer, text: &str, query: &str) -> Excerpt {
    let words: Vec<String> = tokenize(query)
        .into_iter()
        .filter(|word| !is_stopword(word))
        .collect();
    if words.is_empty() {
        return hook_snippet_span(text);
    }
    let query_stems: HashSet<String> = words.iter().map(|word| analyzer.stem_word(word)).collect();
    let mut best: Option<(usize, Block)> = None;
    for block in body_blocks(text) {
        let block_stems: HashSet<String> = analyzer.stem_tokens(&block.text).into_iter().collect();
        let overlap = query_stems.intersection(&block_stems).count();
        if overlap > best.as_ref().map_or(0, |(score, _)| *score) {
            best = Some((overlap, block));
        }
    }
    let Some((_, block)) = best else {
        return hook_snippet_span(text);
    };
    let joined = sentences(&block.text)
        .into_iter()
        .take(MAX_SENTENCES)
        .collect::<Vec<_>>()
        .join(" ");
    let excerpt = clip(strip(&joined));
    let snippet = if block.heading.is_empty() {
        excerpt
    } else {
        format!("{}: {excerpt}", block.heading)
    };
    (snippet, Some((block.first, block.last)))
}
