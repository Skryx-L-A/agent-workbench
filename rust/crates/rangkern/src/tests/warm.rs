use std::path::Path;

use super::TempDir;
use crate::stamp::{Detect, Stamp, now_ns};
use crate::token::Analyzer;
use crate::vault::Unreadable;
use crate::warm::Warm;

/// A long time after every file: stamps taken with this clock are settled.
const LATER: i128 = i128::MAX;

fn rels(warm: &Warm) -> Vec<String> {
    warm.korpus().notes().map(|note| note.rel.clone()).collect()
}

fn all_rels(warm: &Warm) -> Vec<String> {
    warm.korpus()
        .entry_notes()
        .map(|note| note.rel.clone())
        .collect()
}

fn refresh(warm: &mut Warm, analyzer: &mut Analyzer, root: &Path) -> Result<(), Unreadable> {
    warm.refresh(root, analyzer, now_ns())
}

fn text_of(warm: &Warm, rel: &str) -> String {
    warm.korpus()
        .notes()
        .find(|note| note.rel == rel)
        .map(|note| note.text.clone())
        .expect("note is in the corpus")
}

#[test]
fn the_first_refresh_reads_the_corpus_and_a_vanished_vault_empties_the_state() {
    let dir = TempDir::new();
    dir.write("b.md", b"zwei\n");
    dir.write("a/x.md", b"eins\n");
    dir.write("90-secrets/s.md", b"geheim\n");
    dir.write("t.txt", b"kein Markdown\n");
    let mut analyzer = Analyzer::new();
    let mut warm = Warm::new(Detect::Metadata);
    assert!(warm.korpus().notes().next().is_none());
    refresh(&mut warm, &mut analyzer, dir.path()).expect("readable");
    assert_eq!(rels(&warm), ["a/x.md", "b.md"]);
    assert_eq!(warm.korpus().entry_count(), 2);
    assert!(
        refresh(&mut warm, &mut analyzer, &dir.path().join("fehlt")).is_err(),
        "a path that is no folder is unreadable"
    );
    assert_eq!(warm.korpus().entry_count(), 0);
    assert!(refresh(&mut warm, &mut analyzer, &dir.path().join("b.md")).is_err());
    refresh(&mut warm, &mut analyzer, dir.path()).expect("readable again");
    assert_eq!(rels(&warm), ["a/x.md", "b.md"]);
}

#[test]
fn added_removed_renamed_and_rewritten_notes_are_seen_at_once() {
    let dir = TempDir::new();
    dir.write("a.md", b"eins\n");
    dir.write("b.md", b"zwei\n");
    let mut analyzer = Analyzer::new();
    let mut warm = Warm::new(Detect::Metadata);
    refresh(&mut warm, &mut analyzer, dir.path()).expect("readable");
    dir.write("c/n.md", b"drei\n");
    refresh(&mut warm, &mut analyzer, dir.path()).expect("readable");
    assert_eq!(rels(&warm), ["a.md", "b.md", "c/n.md"]);
    std::fs::remove_file(dir.path().join("a.md")).expect("removed");
    refresh(&mut warm, &mut analyzer, dir.path()).expect("readable");
    assert_eq!(rels(&warm), ["b.md", "c/n.md"]);
    std::fs::rename(dir.path().join("b.md"), dir.path().join("z.md")).expect("renamed");
    refresh(&mut warm, &mut analyzer, dir.path()).expect("readable");
    assert_eq!(rels(&warm), ["c/n.md", "z.md"]);
    assert_eq!(text_of(&warm, "z.md"), "zwei\n");
    dir.write("z.md", b"vier\n");
    refresh(&mut warm, &mut analyzer, dir.path()).expect("readable");
    assert_eq!(text_of(&warm, "z.md"), "vier\n");
    std::fs::rename(dir.path().join("z.md"), dir.path().join("90-secrets/z.md"))
        .expect_err("folder is missing");
    std::fs::create_dir_all(dir.path().join("90-secrets")).expect("created");
    std::fs::rename(dir.path().join("z.md"), dir.path().join("90-secrets/z.md")).expect("moved");
    refresh(&mut warm, &mut analyzer, dir.path()).expect("readable");
    assert_eq!(rels(&warm), ["c/n.md"]);
}

#[test]
fn a_rewrite_with_the_same_text_keeps_the_note_and_a_new_text_replaces_it() {
    let dir = TempDir::new();
    dir.write("a.md", b"eins\n");
    let mut analyzer = Analyzer::new();
    let mut warm = Warm::new(Detect::Content);
    refresh(&mut warm, &mut analyzer, dir.path()).expect("readable");
    dir.write("a.md", b"eins\n");
    refresh(&mut warm, &mut analyzer, dir.path()).expect("readable");
    assert_eq!(text_of(&warm, "a.md"), "eins\n");
    dir.write("a.md", b"eins\r\n");
    refresh(&mut warm, &mut analyzer, dir.path()).expect("readable");
    assert_eq!(text_of(&warm, "a.md"), "eins\n");
    dir.write("a.md", b"zwei\n");
    refresh(&mut warm, &mut analyzer, dir.path()).expect("readable");
    assert_eq!(text_of(&warm, "a.md"), "zwei\n");
}

#[test]
fn a_settled_stamp_is_trusted_and_a_racy_one_is_checked_by_the_content() {
    let dir = TempDir::new();
    let path = dir.write("a.md", b"aaaa\n");
    let mut analyzer = Analyzer::new();
    let mut warm = Warm::new(Detect::Metadata);
    warm.refresh(dir.path(), &mut analyzer, LATER)
        .expect("readable");
    assert!(!warm.stamps[0].expect("stamped").racy());
    std::fs::write(&path, b"bbbb\n").expect("rewritten");
    let meta = std::fs::metadata(&path).expect("exists");
    let new = Stamp::new(&meta, LATER, Detect::Metadata);
    warm.stamps[0] = Some(new);
    warm.refresh(dir.path(), &mut analyzer, LATER)
        .expect("readable");
    assert_eq!(
        text_of(&warm, "a.md"),
        "aaaa\n",
        "the stamp says nothing changed, so the file is not read"
    );
    let racy = Stamp::new(&meta, 0, Detect::Metadata);
    assert!(racy.racy());
    warm.stamps[0] = Some(racy);
    warm.refresh(dir.path(), &mut analyzer, LATER)
        .expect("readable");
    assert_eq!(text_of(&warm, "a.md"), "bbbb\n");
    std::fs::write(&path, b"cccc\n").expect("rewritten");
    warm.stamps[0] = None;
    warm.refresh(dir.path(), &mut analyzer, LATER)
        .expect("readable");
    assert_eq!(text_of(&warm, "a.md"), "cccc\n");
}

#[cfg(unix)]
#[test]
fn a_rewrite_of_the_same_size_with_the_old_mtime_is_seen_by_the_change_time() {
    use std::fs::{FileTimes, OpenOptions};
    let dir = TempDir::new();
    let path = dir.write("a.md", b"aaaa\n");
    let mut analyzer = Analyzer::new();
    let mut warm = Warm::new(Detect::Metadata);
    warm.refresh(dir.path(), &mut analyzer, LATER)
        .expect("readable");
    let old = std::fs::metadata(&path)
        .and_then(|meta| meta.modified())
        .expect("mtime");
    std::thread::sleep(std::time::Duration::from_millis(30));
    std::fs::write(&path, b"bbbb\n").expect("rewritten");
    OpenOptions::new()
        .write(true)
        .open(&path)
        .expect("opens")
        .set_times(FileTimes::new().set_modified(old))
        .expect("mtime is reset");
    warm.refresh(dir.path(), &mut analyzer, LATER)
        .expect("readable");
    assert_eq!(text_of(&warm, "a.md"), "bbbb\n");
}

#[cfg(unix)]
#[test]
fn an_unreadable_note_makes_the_refresh_fail_and_the_next_one_look_again() {
    use std::os::unix::fs::PermissionsExt;
    let dir = TempDir::new();
    let path = dir.write("a.md", b"eins\n");
    dir.write("b.md", b"zwei\n");
    let mut analyzer = Analyzer::new();
    let mut warm = Warm::new(Detect::Metadata);
    refresh(&mut warm, &mut analyzer, dir.path()).expect("readable");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o000)).expect("mode");
    if std::fs::read(&path).is_ok() {
        return;
    }
    dir.write("b.md", b"drei\n");
    assert!(refresh(&mut warm, &mut analyzer, dir.path()).is_err());
    assert!(refresh(&mut warm, &mut analyzer, dir.path()).is_err());
    assert_eq!(
        text_of(&warm, "b.md"),
        "zwei\n",
        "a failed refresh changes nothing"
    );
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).expect("mode");
    refresh(&mut warm, &mut analyzer, dir.path()).expect("readable again");
    assert_eq!(text_of(&warm, "b.md"), "drei\n");
    assert_eq!(rels(&warm), ["a.md", "b.md"]);
    let sub = dir.write("s/n.md", b"vier\n");
    refresh(&mut warm, &mut analyzer, dir.path()).expect("readable");
    let folder = sub.parent().expect("folder").to_path_buf();
    std::fs::set_permissions(&folder, std::fs::Permissions::from_mode(0o000)).expect("mode");
    refresh(&mut warm, &mut analyzer, dir.path()).expect("an unlistable folder holds no notes");
    assert_eq!(rels(&warm), ["a.md", "b.md"]);
    std::fs::set_permissions(&folder, std::fs::Permissions::from_mode(0o755)).expect("mode");
    refresh(&mut warm, &mut analyzer, dir.path()).expect("readable");
    assert_eq!(rels(&warm), ["a.md", "b.md", "s/n.md"]);
}

const SOURCE: &[u8] = b"{\"art\": \"quellen\"}";

#[test]
fn originals_of_a_source_stay_out_of_the_corpus_and_come_back_with_the_manifest() {
    let dir = TempDir::new();
    dir.write("q/quellen/orig.md", b"original\n");
    dir.write("q/quellen/mit.md", b"---\nquelle: x\n---\nauszug\n");
    dir.write("q/notiz.md", b"notiz\n");
    let mut analyzer = Analyzer::new();
    let mut warm = Warm::new(Detect::Metadata);
    refresh(&mut warm, &mut analyzer, dir.path()).expect("readable");
    assert_eq!(
        rels(&warm),
        ["q/notiz.md", "q/quellen/mit.md", "q/quellen/orig.md"]
    );
    dir.write("q/.space.yml", SOURCE);
    refresh(&mut warm, &mut analyzer, dir.path()).expect("readable");
    assert_eq!(rels(&warm), ["q/notiz.md", "q/quellen/mit.md"]);
    assert_eq!(all_rels(&warm).len(), 3, "the original is kept for later");
    dir.write("q/.space.yml", b"{\"art\": \"normal\"}");
    refresh(&mut warm, &mut analyzer, dir.path()).expect("readable");
    assert_eq!(rels(&warm).len(), 3);
    dir.write("q/.space.yml", SOURCE);
    refresh(&mut warm, &mut analyzer, dir.path()).expect("readable");
    assert_eq!(rels(&warm).len(), 2);
    std::fs::remove_file(dir.path().join("q/.space.yml")).expect("removed");
    refresh(&mut warm, &mut analyzer, dir.path()).expect("readable");
    assert_eq!(rels(&warm).len(), 3);
    dir.write("q/.space.yml", SOURCE);
    dir.write("q/quellen/orig.md", b"---\nquelle: y\n---\noriginal\n");
    refresh(&mut warm, &mut analyzer, dir.path()).expect("readable");
    assert_eq!(
        rels(&warm),
        ["q/notiz.md", "q/quellen/mit.md", "q/quellen/orig.md"]
    );
    dir.write("q/quellen/neu.md", b"neu\n");
    refresh(&mut warm, &mut analyzer, dir.path()).expect("readable");
    assert_eq!(rels(&warm).len(), 3);
    assert_eq!(all_rels(&warm).len(), 4);
}

#[test]
fn a_source_space_at_the_top_of_the_vault_is_asked_too() {
    let dir = TempDir::new();
    dir.write("quellen/orig.md", b"original\n");
    dir.write("a.md", b"text\n");
    let mut analyzer = Analyzer::new();
    let mut warm = Warm::new(Detect::Metadata);
    refresh(&mut warm, &mut analyzer, dir.path()).expect("readable");
    assert_eq!(rels(&warm).len(), 2);
    dir.write(".space.yml", SOURCE);
    refresh(&mut warm, &mut analyzer, dir.path()).expect("readable");
    assert_eq!(rels(&warm), ["a.md"]);
}

#[test]
fn the_state_has_one_stamp_per_note() {
    let dir = TempDir::new();
    dir.write("a.md", b"eins\n");
    dir.write("b.md", b"zwei\n");
    let mut analyzer = Analyzer::new();
    let mut warm = Warm::new(Detect::Metadata);
    assert!(warm.stamps.is_empty());
    refresh(&mut warm, &mut analyzer, dir.path()).expect("readable");
    assert_eq!(warm.stamps.len(), 2);
    assert!(warm.stamps.iter().all(Option::is_some));
    std::fs::remove_file(dir.path().join("a.md")).expect("removed");
    refresh(&mut warm, &mut analyzer, dir.path()).expect("readable");
    assert_eq!(warm.stamps.len(), 1);
}
