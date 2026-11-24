//! The warm state of `rangkern serve`: the notes of the search corpus and their index, kept in
//! memory and brought up to date with the disk before every search.
//!
//! One refresh lists the corpus (the rules of `notiz-kern`, the only source of them), stamps every
//! file and compares the stamp with the one of the last refresh ([`crate::stamp`]). Only a file
//! whose stamp differs, or is in doubt, is read again, and only a note whose text really differs is
//! analysed again. The statistics of the index are then computed again from the stored analysis of
//! all notes by the same code that builds an index from scratch, so the numbers are the ones a fresh
//! build gives.
//!
//! The refresh also finds what the stamps of the notes cannot show: a note that appears, vanishes,
//! moves (the listing changes), and a `.space.yml` that turns a space into a source space (the files
//! are read again on every refresh; there are few) and so makes a note an original that the search
//! leaves out. Nothing is kept open, written or cached on disk.

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use notiz_kern::text::read_text;

use crate::bm25::{LexKorpus, Origin};
use crate::notes::{NoteRec, note_rec};
use crate::stamp::{Detect, Stamp};
use crate::token::Analyzer;
use crate::vault::{Unreadable, corpus_rels, is_original, read_note};

/// The notes of the vault, their index, and what is needed to see a change.
#[derive(Debug)]
pub struct Warm {
    korpus: LexKorpus,
    /// The stamp of every note the index holds, in the order of the index.
    pub(crate) stamps: Vec<Option<Stamp>>,
    /// Whether the note at that position is an original of a source and so left out of the corpus.
    originals: Vec<bool>,
    detect: Detect,
}

impl Warm {
    /// An empty state.
    #[must_use]
    pub fn new(detect: Detect) -> Self {
        Self {
            korpus: LexKorpus::default(),
            stamps: Vec::new(),
            originals: Vec::new(),
            detect,
        }
    }

    /// The index of the notes.
    #[must_use]
    pub fn korpus(&self) -> &LexKorpus {
        &self.korpus
    }

    /// Whether the note at `at` is still the file the stamp `new` describes, by its stamp alone.
    fn unchanged(&self, at: usize, new: Option<&Stamp>) -> bool {
        match (self.stamps.get(at), new) {
            (Some(Some(old)), Some(new)) => old.unchanged(new),
            _ => false,
        }
    }

    /// Brings the state up to date with the vault at `root`; `now` is the time in nanoseconds
    /// since the epoch (see [`crate::stamp::now_ns`]).
    ///
    /// The state does not change when this fails; the next refresh looks again at what failed.
    ///
    /// # Errors
    ///
    /// [`Unreadable`] when `root` is not a directory or a note of the corpus cannot be read.
    pub fn refresh(
        &mut self,
        root: &Path,
        analyzer: &mut Analyzer,
        now: i128,
    ) -> Result<(), Unreadable> {
        if !root.is_dir() {
            *self = Self::new(self.detect);
            return Err(Unreadable);
        }
        let listing = corpus_rels(root);
        let known: HashMap<&str, usize> = self
            .korpus
            .entry_notes()
            .enumerate()
            .map(|(at, note)| (note.rel.as_str(), at))
            .collect();
        let mut plan: Vec<Origin> = Vec::with_capacity(listing.len());
        let mut stamps: Vec<Option<Stamp>> = Vec::with_capacity(listing.len());
        let mut fresh = false;
        for rel in &listing {
            let path = root.join(rel);
            let stamp = fs::symlink_metadata(&path)
                .ok()
                .map(|meta| Stamp::new(&meta, now, self.detect));
            let origin = match known.get(rel.as_str()) {
                Some(&at) if self.unchanged(at, stamp.as_ref()) => Origin::Kept(at),
                Some(&at) => {
                    let text = read_text(&path)?;
                    if self
                        .korpus
                        .entry_note(at)
                        .is_some_and(|note| note.text == text)
                    {
                        Origin::Kept(at)
                    } else {
                        fresh = true;
                        Origin::Fresh(note_rec(rel, text))
                    }
                }
                None => {
                    fresh = true;
                    Origin::Fresh(read_note(root, rel)?)
                }
            };
            plan.push(origin);
            stamps.push(stamp);
        }
        drop(known);
        let flags = self.originals_of(root, &plan);
        // A note that is gone or new changes the length of `flags`, so it differs from `originals`.
        if fresh || flags != self.originals {
            self.originals = flags;
            let listed = plan
                .into_iter()
                .zip(&self.originals)
                .map(|(origin, &original)| (origin, !original))
                .collect();
            self.korpus.rebuild(listed, analyzer);
        }
        self.stamps = stamps;
        Ok(())
    }

    /// For every note of the plan, whether it is an original of a source. The `.space.yml` files
    /// that decide it are read again each time; there are few of them.
    fn originals_of(&self, root: &Path, plan: &[Origin]) -> Vec<bool> {
        let mut kinds = HashMap::new();
        plan.iter()
            .map(|origin| {
                let note: Option<&NoteRec> = match origin {
                    Origin::Kept(at) => self.korpus.entry_note(*at),
                    Origin::Fresh(note) => Some(note),
                };
                note.is_some_and(|note| is_original(root, note, &mut kinds))
            })
            .collect()
    }
}
