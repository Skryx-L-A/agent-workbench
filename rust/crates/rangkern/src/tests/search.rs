use std::collections::HashSet;

use super::TempDir;
use crate::hit::SearchHit;
use crate::search::{Params, filter_space, flag_contradictions, search};
use crate::vault::Vectors;

fn hit_in(rel: &str, space: &str) -> SearchHit {
    let mut hit = SearchHit::new(rel, 1.0);
    hit.space = space.to_string();
    hit
}

#[test]
fn the_space_filter_takes_one_name_or_a_comma_list() {
    let hits = || vec![hit_in("a", "kern"), hit_in("b", "pp"), hit_in("c", "top")];
    let names = |space: Option<&str>| -> Vec<String> {
        filter_space(hits(), space)
            .into_iter()
            .map(|h| h.rel)
            .collect()
    };
    assert_eq!(names(None), ["a", "b", "c"]);
    assert_eq!(names(Some("")), ["a", "b", "c"]);
    assert_eq!(names(Some("pp")), ["b"]);
    assert_eq!(names(Some("kern,top")), ["a", "c"]);
    assert_eq!(names(Some(" pp , top ")), ["b", "c"]);
    assert!(names(Some(",")).is_empty());
    assert!(names(Some("p")).is_empty());
}

#[test]
fn open_contradictions_mark_the_hits_of_their_notes() {
    let dir = TempDir::new();
    dir.write("_meta/state/contradictions.json", br#"{"findings": {"1": {"status": "open", "note_a": {"rel": "a.md"}, "note_b": {"rel": "z.md"}}}}"#);
    let mut hits = vec![SearchHit::new("a.md", 1.0), SearchHit::new("b.md", 1.0)];
    flag_contradictions(&mut hits, dir.path());
    assert_eq!(
        (hits[0].contradiction, hits[1].contradiction),
        (true, false)
    );
    let mut none = vec![SearchHit::new("a.md", 1.0)];
    flag_contradictions(&mut none, TempDir::new().path());
    assert!(!none[0].contradiction);
}

fn params<'a>(k: usize, vectors: Option<&'a Vectors>, space: Option<&'a str>) -> Params<'a> {
    Params {
        k,
        vectors: vectors.map(Vectors::view),
        space,
        today: "2026-11-18",
    }
}

#[test]
fn a_space_filter_reads_a_deeper_pool_than_k() {
    let dir = TempDir::new();
    dir.write(
        ".brain/spaces.lock",
        br#"{"spaces": [{"space": "pp", "mount": "p"}]}"#,
    );
    for i in 0..12 {
        dir.write(
            &format!("k/k{i:02}.md"),
            format!("{}\n", "kaffee ".repeat(20 - i)).as_bytes(),
        );
    }
    dir.write("p/x.md", b"kaffee\n");
    for i in 0..30 {
        dir.write(&format!("f/f{i:02}.md"), b"anderes thema\n");
    }
    let (hits, fallback) =
        search(dir.path(), "kaffee", &params(2, None, Some("pp"))).expect("readable");
    assert!(fallback);
    assert_eq!(
        hits.iter().map(|h| h.rel.as_str()).collect::<Vec<_>>(),
        ["p/x.md"]
    );
    assert_eq!(hits[0].space, "pp");
    let (plain, _) = search(dir.path(), "kaffee", &params(2, None, None)).expect("readable");
    assert_eq!(plain.len(), 2);
    assert!(plain.iter().all(|h| h.space == "kern"));
}

#[test]
fn vectors_of_paths_outside_the_corpus_count_for_nothing() {
    let dir = TempDir::new();
    dir.write("a.md", b"Garten Beet\n");
    dir.write("90-secrets/s.md", b"geheim\n");
    dir.write("HOT.md", b"heiss\n");
    let vectors = Vectors {
        query: vec![1.0, 0.0],
        notes: vec![
            ("90-secrets/s.md".into(), vec![1.0, 0.0]),
            ("HOT.md".into(), vec![1.0, 0.0]),
            ("nirgends.md".into(), vec![1.0, 0.0]),
            ("a.md".into(), vec![0.0, 1.0]),
        ],
    };
    let (hits, fallback) =
        search(dir.path(), "Beet", &params(5, Some(&vectors), None)).expect("readable");
    assert!(!fallback);
    assert_eq!(
        hits.iter().map(|h| h.rel.as_str()).collect::<Vec<_>>(),
        ["a.md"]
    );
    let only_relics = Vectors {
        query: vec![1.0],
        notes: vec![("90-secrets/s.md".into(), vec![1.0])],
    };
    let (none, fallback) =
        search(dir.path(), "zzzz", &params(5, Some(&only_relics), None)).expect("readable");
    assert!((none.len(), fallback) == (0, false));
}

#[test]
fn the_full_text_fallback_lists_corpus_notes_only_sorted_and_cut_to_k() {
    let dir = TempDir::new();
    for rel in [
        "b.md",
        "a/y.md",
        "a.md",
        "90-secrets/s.md",
        "_meta/m.md",
        "c.md",
    ] {
        dir.write(rel, b"ritz\n");
    }
    let (hits, fallback) = search(dir.path(), "rit+z", &params(3, None, None)).expect("readable");
    assert!(fallback);
    assert_eq!(
        hits.iter().map(|h| h.rel.as_str()).collect::<Vec<_>>(),
        ["a.md", "a/y.md", "b.md"]
    );
    assert!(
        hits.iter()
            .all(|h| h.score == 0.0 && h.matched == "text" && h.snippet.is_empty())
    );
    let seen: HashSet<_> = hits.iter().map(|h| h.rel.clone()).collect();
    assert!(!seen.contains("90-secrets/s.md"));
}

#[test]
fn a_vault_that_cannot_be_listed_has_no_hits() {
    let dir = TempDir::new();
    let (hits, fallback) =
        search(&dir.path().join("fehlt"), "x", &params(5, None, None)).expect("empty");
    assert!(hits.is_empty() && fallback);
}

/// A vault of 120 notes: BM25 and the vector list disagree, some notes are derived pages, some are superseded.
fn prefix_vault() -> TempDir {
    let dir = TempDir::new();
    let mut state = 20_261_119_u64;
    for i in 0..120_u64 {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        let word =
            ["alpha", "beta", "gamma", "delta"][usize::try_from(state >> 62).expect("two bits")];
        let mut body = vec![word; usize::try_from(1 + i % 5).expect("small")];
        body.extend(std::iter::repeat_n(
            "alpha",
            usize::try_from(i % 4).expect("small"),
        ));
        let extra = if i % 17 == 3 {
            "class: derived\n".to_string()
        } else if i % 7 == 2 {
            format!(
                "status: superseded\nsuperseded-by: [[Note {:03}]]\n",
                (i + 11) % 120
            )
        } else {
            String::new()
        };
        dir.write(
            &format!("10-global/n{i:03}.md"),
            format!(
                "---\ntitle: Note {i:03}\n{extra}---\n\n{}\n",
                body.join(" ")
            )
            .as_bytes(),
        );
    }
    dir
}

fn prefix_vectors() -> Vectors {
    let mut state = 7_u64;
    let notes = (0..120)
        .map(|i| {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            let angle =
                f64::from(u32::try_from(state >> 40).expect("24 bits")) / 16_777_216.0 * 3.0;
            (
                format!("10-global/n{i:03}.md"),
                vec![angle.cos(), angle.sin()],
            )
        })
        .collect();
    Vectors {
        query: vec![1.0, 0.0],
        notes,
    }
}

#[test]
fn the_list_for_a_larger_k_extends_the_list_for_a_smaller_k() {
    let dir = prefix_vault();
    let vectors = prefix_vectors();
    for (query, with_vectors) in [
        ("alpha beta", true),
        ("alpha gamma delta", true),
        ("beta", true),
        ("alpha beta", false),
        ("beta", false),
    ] {
        let vec = with_vectors.then_some(&vectors);
        let (long, fallback) = search(dir.path(), query, &params(50, vec, None)).expect("readable");
        assert_eq!(fallback, !with_vectors);
        assert!(long.len() >= 20, "{query:?}: {} hits", long.len());
        let long: Vec<&str> = long.iter().map(|h| h.rel.as_str()).collect();
        for k in [1, 2, 3, 5, 8, 10, 15, 20, 30]
            .into_iter()
            .filter(|k| *k <= long.len())
        {
            let (short, _) = search(dir.path(), query, &params(k, vec, None)).expect("readable");
            let short: Vec<&str> = short.iter().map(|h| h.rel.as_str()).collect();
            assert_eq!(short, long[..k], "{query:?} vectors {with_vectors} k {k}");
        }
    }
}

#[test]
fn a_small_space_is_found_although_its_notes_rank_deep() {
    let dir = prefix_vault();
    dir.write(
        ".brain/spaces.lock",
        br#"{"spaces": [{"space": "q", "mount": "20-projects/q"}]}"#,
    );
    let mut vectors = prefix_vectors();
    for i in 0..6 {
        let filler: Vec<String> = (0..120 + i).map(|j| format!("zusatz{j}")).collect();
        dir.write(
            &format!("20-projects/q/n{i}.md"),
            format!(
                "---\ntitle: Raumnotiz {i}\n---\n\nalpha {}\n",
                filler.join(" ")
            )
            .as_bytes(),
        );
        vectors.notes.push((
            format!("20-projects/q/n{i}.md"),
            vec![-1.0, 0.1 * f64::from(i)],
        ));
    }
    for with_vectors in [true, false] {
        let vec = with_vectors.then_some(&vectors);
        let (long, _) = search(dir.path(), "alpha", &params(50, vec, Some("q"))).expect("readable");
        assert_eq!(long.len(), 6, "vectors {with_vectors}");
        assert!(long.iter().all(|h| h.space == "q"));
        let long: Vec<&str> = long.iter().map(|h| h.rel.as_str()).collect();
        for k in [1, 2, 3, 5] {
            let (short, _) =
                search(dir.path(), "alpha", &params(k, vec, Some("q"))).expect("readable");
            let short: Vec<&str> = short.iter().map(|h| h.rel.as_str()).collect();
            assert_eq!(short, long[..k], "vectors {with_vectors} k {k}");
        }
    }
}
