//! Tests of the corpus rules and the directory walk.

use super::TempDir;
use crate::corpus::{
    EXCLUDED_DIRS, EXCLUDED_ROOT_FILES, corpus, is_excluded, path_info, path_parts, stem_of,
};
use crate::text::exclusion_key;

fn excluded(path: &[&str]) -> (bool, bool) {
    (is_excluded(path, false), is_excluded(path, true))
}

#[test]
fn path_parts_drops_empty_and_dot_parts_but_keeps_dot_dot() {
    assert_eq!(path_parts("a//b/./c/.."), ["a", "b", "c", ".."]);
    assert_eq!(path_parts("/a"), ["a"]);
    assert_eq!(path_parts("a/"), ["a"]);
    assert_eq!(path_parts("./"), Vec::<&str>::new());
    assert_eq!(path_parts(""), Vec::<&str>::new());
    assert_eq!(path_parts(".hidden/.x"), [".hidden", ".x"]);
}

#[test]
fn stem_of_follows_pure_path_stem() {
    assert_eq!(stem_of("a.md"), "a");
    assert_eq!(stem_of("a.b.md"), "a.b");
    assert_eq!(stem_of(".md"), ".md");
    assert_eq!(stem_of(".hidden"), ".hidden");
    assert_eq!(stem_of("x."), "x.");
    assert_eq!(stem_of(".."), "..");
    assert_eq!(stem_of("noext"), "noext");
    assert_eq!(stem_of(""), "");
    assert_eq!(stem_of("A.MD"), "A");
    assert_eq!(stem_of("a.b"), "a");
    assert_eq!(stem_of("..md"), ".");
}

#[test]
fn empty_path_is_excluded() {
    assert_eq!(excluded(&[]), (true, true));
}

#[test]
fn excluded_directories_at_any_depth_and_in_any_case() {
    for path in [
        &["90-secrets", "a.md"][..],
        &["x", "90-SECRETS", "a.md"],
        &["_meta", "a.md"],
        &["a", "_Meta", "b.md"],
        &[".git", "x.md"],
        &[".obsidian", "x.md"],
        &[".hidden", "a.md"],
        &["a", ".hidden", "b", "c.md"],
        &["a", "90-secrets", "b", "c.md"],
    ] {
        assert_eq!(excluded(path), (true, true), "{path:?}");
    }
    for path in [
        &[".hidden.md"][..],
        &["a", ".hidden.md"],
        &["90-secrets.md"],
        &["a", "_meta.md"],
        &["a", "90-secrets2", "b.md"],
        &["_metax", "b.md"],
    ] {
        assert_eq!(excluded(path), (false, false), "{path:?}");
    }
}

#[test]
fn excluded_top_level_files_compare_without_case() {
    for name in [
        "HOT.md",
        "hot.md",
        "Index.md",
        "CRITICAL-FACTS.md",
        "review-queue.md",
        "OPEN-QUESTIONS.md",
        "log.md",
        "LOG.md",
    ] {
        assert_eq!(excluded(&[name]), (true, true), "{name}");
    }
    for path in [
        &["a", "HOT.md"][..],
        &["a", "log.md"],
        &["a", "index.md"],
        &["decisions.md"],
        &["notes.md"],
    ] {
        assert_eq!(excluded(path), (false, false), "{path:?}");
    }
}

#[test]
fn excluded_generated_files_at_any_depth() {
    for path in [
        &["a", "review-queue.md"][..],
        &["review-queue.md"],
        &["a", "DECISIONS.md"],
        &["DECISIONS.md"],
    ] {
        assert_eq!(excluded(path), (true, true), "{path:?}");
    }
}

#[test]
fn moc_is_excluded_from_linking_only() {
    assert_eq!(excluded(&["MOC.md"]), (true, false));
    assert_eq!(excluded(&["a", "b", "MOC.md"]), (true, false));
    assert_eq!(excluded(&["a", "moc.md"]), (false, false));
}

#[test]
fn excluded_report_prefixes() {
    for name in [
        "gardener-report-1.md",
        "brain-health-x.md",
        "dream-report-.md",
        "brain-run-1.md",
        "brain-vorschlaege.md",
        "brain-vorschlaege-x.md",
    ] {
        assert_eq!(excluded(&["a", name]), (true, true), "{name}");
    }
    assert_eq!(excluded(&["a", "a-brain-run-1.md"]), (false, false));
    assert_eq!(excluded(&["a", "brain-run.md"]), (false, false));
}

#[test]
fn excluded_drop_zone_is_a_text_prefix_of_the_directory() {
    for path in [
        &["00-sources", "drop", "a.md"][..],
        &["00-sources", "dropx", "a.md"],
        &["00-sources", "drop", "sub", "a.md"],
    ] {
        assert_eq!(excluded(path), (true, true), "{path:?}");
    }
    for path in [
        &["00-sources", "a.md"][..],
        &["00-sources", "drop"],
        &["x", "00-sources", "drop", "a.md"],
        &["00-source", "drop", "a.md"],
        &["drop", "a.md"],
    ] {
        assert_eq!(excluded(path), (false, false), "{path:?}");
    }
}

#[test]
fn path_info_describes_a_project_note() {
    let info = path_info("20-projects/p/x/Note.md");
    assert_eq!(info.rel, "20-projects/p/x/Note.md");
    assert!(!info.linking_excluded);
    assert!(!info.search_excluded);
    assert!(info.lint_in_scope);
    assert_eq!(info.branch, "20-projects/p");
    assert_eq!(info.stem_key, "note");
}

#[test]
fn path_info_branches() {
    let branch = |rel: &str| path_info(rel).branch;
    assert_eq!(branch("30-topics/t/a.md"), "30-topics/t");
    assert_eq!(branch("20-projects/p.md"), "20-projects");
    assert_eq!(branch("30-topics/a.md"), "30-topics");
    assert_eq!(branch("10-global/a.md"), "10-global");
    assert_eq!(branch("10-global/x/y/a.md"), "10-global");
    assert_eq!(branch("40-people/a.md"), "40-people");
    assert_eq!(branch("a.md"), "");
    assert_eq!(branch(""), "");
    assert_eq!(branch("20-projects/p/"), "20-projects");
    assert_eq!(branch("./10-global//a.md"), "10-global");
    assert_eq!(branch("../10-global/a.md"), "..");
}

#[test]
fn path_info_moc_and_scope() {
    let info = path_info("30-topics/t/MOC.md");
    assert!(info.linking_excluded);
    assert!(!info.search_excluded);
    assert!(!info.lint_in_scope);
}

#[test]
fn path_info_handles_odd_paths_without_touching_the_file_system() {
    let info = path_info("");
    assert!(info.linking_excluded && info.search_excluded);
    assert_eq!(info.stem_key, "");
    assert!(!info.lint_in_scope);
    let info = path_info("./10-global//a.md");
    assert_eq!(info.rel, "./10-global//a.md");
    assert!(!info.lint_in_scope);
    assert!(!info.linking_excluded);
    assert!(path_info("../x/90-secrets/a.md").search_excluded);
    // `..` starts with a dot, so a path that leaves the vault is excluded like a hidden directory.
    assert!(path_info("../x/a.md").search_excluded);
    assert!(path_info("a/../b.md").linking_excluded);
    assert_eq!(path_info("d/.md").stem_key, ".md");
    assert_eq!(path_info("d/A.b.MD").stem_key, "a.b");
    assert_eq!(path_info("d/Ha\u{308}.md").stem_key, "h\u{e4}");
    assert_eq!(path_info("d/").stem_key, "d");
}

fn vault() -> TempDir {
    let dir = TempDir::new();
    for rel in [
        "10-global/a.md",
        "10-global/b.txt",
        "10-global/Sub/c.md",
        "10-global/sup/MOC.md",
        "10-global/.md",
        "10-global/UPPER.MD",
        "a-b/x.md",
        "a/x.md",
        "z.md",
        "HOT.md",
        "review-queue.md",
        "00-sources/n.md",
        "00-sources/drop/d.md",
        "90-secrets/s.md",
        "10-global/90-Secrets/e.md",
        "_meta/m.md",
        ".hidden/h.md",
        "10-global/.dot/d.md",
        "10-global/_META/m.md",
    ] {
        dir.write(rel, b"x");
    }
    dir
}

#[test]
fn corpus_lists_and_sorts_by_path_parts() {
    let dir = vault();
    let found = corpus(dir.path()).expect("vault is readable");
    let linking = [
        "00-sources/n.md",
        "10-global/.md",
        "10-global/Sub/c.md",
        "10-global/a.md",
        "a/x.md",
        "a-b/x.md",
        "z.md",
    ];
    assert_eq!(found.linking, linking);
    let search = [
        "00-sources/n.md",
        "10-global/.md",
        "10-global/Sub/c.md",
        "10-global/a.md",
        "10-global/sup/MOC.md",
        "a/x.md",
        "a-b/x.md",
        "z.md",
    ];
    assert_eq!(found.search, search);
}

/// What the temporary folder of the tests tells apart: `(case, normalisation)`. A file system that
/// folds the case (APFS, the default of macOS) keeps one directory for `Probe` and `probe`; one that
/// folds the normalisation (APFS, too) keeps one file for the NFC and the NFD spelling of a name.
#[allow(
    clippy::print_stdout,
    reason = "the line shows with --nocapture which branch of the test ran on this machine"
)]
fn file_system_tells_apart() -> (bool, bool) {
    let dir = TempDir::new();
    dir.write("Probe/x", b"x");
    dir.write("h\u{e4}", b"x");
    let case = !dir.path().join("probe").exists();
    let normalisation = !dir.path().join("ha\u{308}").exists();
    println!(
        "file system of the temporary folder: tells apart case {case}, normalisation {normalisation}"
    );
    (case, normalisation)
}

/// The corpus lists what the file system holds, as the Python model does when it walks the same
/// folder: where two names fold to one, a single entry stays, spelled as it was created first.
#[test]
fn corpus_lists_names_that_differ_only_in_case_or_normalisation_as_the_file_system_holds_them() {
    let (case, normalisation) = file_system_tells_apart();
    // Directories `Sub` and `sub`: one directory when the case folds, with both files in it.
    let dir = TempDir::new();
    dir.write("10-global/Sub/c.md", b"x");
    dir.write("10-global/sub/MOC.md", b"x");
    let found = corpus(dir.path()).expect("vault is readable");
    assert_eq!(found.linking, ["10-global/Sub/c.md"]);
    let search: &[&str] = if case {
        &["10-global/Sub/c.md", "10-global/sub/MOC.md"]
    } else {
        &["10-global/Sub/MOC.md", "10-global/Sub/c.md"]
    };
    assert_eq!(found.search, search, "directories Sub and sub");
    // Files `a.md` and `A.md`: one file when the case folds.
    let dir = TempDir::new();
    dir.write("10-global/a.md", b"x");
    dir.write("10-global/A.md", b"x");
    let found = corpus(dir.path()).expect("vault is readable");
    let expected: &[&str] = if case {
        &["10-global/A.md", "10-global/a.md"]
    } else {
        &["10-global/a.md"]
    };
    assert_eq!(found.linking, expected, "files a.md and A.md");
    assert_eq!(found.search, expected, "files a.md and A.md");
    // Files `h\u{e4}.md` (NFC) and `ha\u{308}.md` (NFD): one file when the normalisation folds.
    let dir = TempDir::new();
    dir.write("10-global/h\u{e4}.md", b"x");
    dir.write("10-global/ha\u{308}.md", b"x");
    let found = corpus(dir.path()).expect("vault is readable");
    let expected: &[&str] = if normalisation {
        &["10-global/ha\u{308}.md", "10-global/h\u{e4}.md"]
    } else {
        &["10-global/h\u{e4}.md"]
    };
    assert_eq!(found.linking, expected, "files NFC and NFD");
    assert_eq!(found.search, expected, "files NFC and NFD");
}

#[cfg(unix)]
#[test]
fn corpus_never_follows_or_lists_symbolic_links() {
    use std::os::unix::fs::symlink;
    let dir = vault();
    let root = dir.path();
    symlink("a.md", root.join("10-global/link.md")).expect("file link");
    symlink("10-global", root.join("linkdir")).expect("directory link");
    symlink("nowhere", root.join("broken.md")).expect("broken link");
    symlink(root.join("90-secrets"), root.join("secretlink")).expect("link to secrets");
    let found = corpus(root).expect("vault is readable");
    for list in [&found.linking, &found.search] {
        assert!(
            list.iter()
                .all(|p| !p.contains("link") && !p.contains("broken")),
            "{list:?}"
        );
        assert!(list.iter().all(|p| !p.contains("secret")), "{list:?}");
    }
    assert_eq!(found.linking.len(), 7);
    assert_eq!(found.search.len(), 8);
}

#[cfg(unix)]
#[test]
fn corpus_accepts_a_root_that_is_a_link_to_a_directory() {
    use std::os::unix::fs::symlink;
    let dir = TempDir::new();
    dir.write("real/a.md", b"x");
    symlink(dir.path().join("real"), dir.path().join("root")).expect("root link");
    let found = corpus(&dir.path().join("root")).expect("root is readable");
    assert_eq!(found.linking, ["a.md"]);
}

#[test]
fn corpus_of_an_empty_directory_is_empty() {
    let dir = TempDir::new();
    let found = corpus(dir.path()).expect("directory is readable");
    assert_eq!(found.linking.len(), 0);
    assert_eq!(found.search.len(), 0);
}

#[test]
fn corpus_fails_for_a_file_and_for_a_missing_path() {
    let dir = TempDir::new();
    let file = dir.write("f.md", b"x");
    assert!(corpus(&file).is_err());
    assert!(corpus(&dir.path().join("missing")).is_err());
}

#[test]
fn corpus_paths_do_not_depend_on_where_the_vault_lies() {
    let dir = TempDir::new();
    dir.write("_meta/vault/a.md", b"x");
    dir.write(".hidden/vault/b.md", b"x");
    let found = corpus(&dir.path().join("_meta/vault")).expect("vault is readable");
    assert_eq!(found.linking, ["a.md"]);
    let found = corpus(&dir.path().join(".hidden/vault")).expect("vault is readable");
    assert_eq!(found.search, ["b.md"]);
}

#[test]
fn excluded_lists_are_written_in_the_form_of_the_exclusion_key() {
    // A listed name that the key would change could never match; an NFC umlaut must stay NFC.
    for name in EXCLUDED_DIRS.iter().chain(EXCLUDED_ROOT_FILES.iter()) {
        assert_eq!(exclusion_key(name), *name);
    }
}

#[test]
fn excluded_directories_under_unicode_spellings() {
    for dir in [
        "90-\u{17f}ecrets",
        "90-\u{17f}ECRETS",
        "90-SECRETS",
        "\u{ff19}\u{ff10}-secrets",
        "_M\u{c9}TA",
        "_META",
        "\u{ff3f}meta",
        "\u{ff0e}hidden",
        "\u{ff0e}git",
        "\u{212a}",
    ] {
        let expected = dir != "\u{212a}" && dir != "_M\u{c9}TA";
        assert_eq!(excluded(&[dir, "a.md"]), (expected, expected), "{dir:?}");
        assert_eq!(
            excluded(&["x", dir, "b", "a.md"]),
            (expected, expected),
            "{dir:?}"
        );
    }
}

#[test]
fn excluded_root_files_under_unicode_spellings() {
    for (name, expected) in [
        ("HOT.md", true),
        ("\u{ff28}\u{ff2f}\u{ff34}.md", true),
        ("\u{fb01}.md", false),
        ("CRITICAL-FACTS.MD", true),
        ("l\u{f6}g.md", false),
        ("LOG.md", true),
        ("\u{ff2c}\u{ff2f}\u{ff27}.md", true),
    ] {
        assert_eq!(excluded(&[name]), (expected, expected), "{name:?}");
    }
    // At the top level only, as before.
    assert_eq!(
        excluded(&["a", "\u{ff28}\u{ff2f}\u{ff34}.md"]),
        (false, false)
    );
}

#[test]
fn path_info_excludes_unicode_spellings_of_secrets() {
    for rel in [
        "90-\u{17f}ecrets/x.md",
        "90-SECRETS/x.md",
        "a/90-\u{17f}ECRETS/x.md",
    ] {
        let info = path_info(rel);
        assert!(info.linking_excluded && info.search_excluded, "{rel}");
    }
}

#[test]
fn corpus_does_not_enter_unicode_spellings_of_excluded_directories() {
    // Each spelling of `90-secrets` gets a vault of its own: on a file system that folds the case
    // (APFS folds `\u{17f}` to `s`, too) they would all be one directory, and a spelling would no
    // longer be walked as itself.
    for spelling in ["90-\u{17f}ecrets", "90-SECRETS"] {
        let dir = TempDir::new();
        dir.write(&format!("{spelling}/x.md"), b"a");
        dir.write("keep.md", b"a");
        let result = corpus(dir.path()).expect("vault is readable");
        assert_eq!(result.linking, vec!["keep.md"], "{spelling:?}");
    }
    let dir = TempDir::new();
    dir.write("\u{ff10}/x.md", b"a");
    dir.write("sub/_M\u{212a}/x.md", b"a");
    dir.write("sub/\u{ff0e}hidden/x.md", b"a");
    dir.write("sub/ok.md", b"a");
    dir.write("\u{ff28}\u{ff2f}\u{ff34}.md", b"a");
    dir.write("keep.md", b"a");
    let result = corpus(dir.path()).expect("vault is readable");
    assert_eq!(
        result.linking,
        vec![
            "keep.md",
            "sub/_M\u{212a}/x.md",
            "sub/ok.md",
            "\u{ff10}/x.md"
        ]
    );
}
