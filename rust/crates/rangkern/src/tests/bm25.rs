use super::{data, float, identical};
use crate::bm25::{LexKorpus, Origin};
use crate::notes::note_rec;
use crate::token::Analyzer;

#[test]
fn scores_and_lists_agree_with_python_on_invented_corpora() {
    let corpora = data("bm25");
    let corpora = corpora.as_array().expect("corpora");
    assert!(corpora.len() >= 40);
    let mut checked = 0;
    for corpus in corpora {
        let files = corpus["files"].as_object().expect("files");
        let notes = corpus["rels"]
            .as_array()
            .expect("rels")
            .iter()
            .map(|rel| {
                let rel = rel.as_str().expect("rel");
                note_rec(rel, files[rel].as_str().expect("text").to_string())
            })
            .collect();
        let mut analyzer = Analyzer::new();
        let korpus = LexKorpus::new(notes, &mut analyzer);
        for query in corpus["queries"].as_array().expect("queries") {
            let text = query["query"].as_str().expect("query");
            let scores = korpus.scores(&mut analyzer, text);
            let expected = query["scores"].as_array().expect("scores");
            assert_eq!(scores.len(), expected.len());
            for (got, want) in scores.iter().zip(expected) {
                assert!(
                    identical(*got, float(want)),
                    "score {got} against {want} for {text:?}"
                );
            }
            let k = usize::try_from(query["k"].as_u64().expect("k")).expect("fits");
            let hits = korpus.hits(&mut analyzer, text, k);
            let wanted = query["hits"].as_array().expect("hits");
            assert_eq!(hits.len(), wanted.len(), "hits of {text:?} with k {k}");
            for (hit, want) in hits.iter().zip(wanted) {
                assert_eq!(hit.rel, want[0].as_str().expect("rel"));
                assert!(identical(hit.score, float(&want[1])));
                assert_eq!(
                    hit.bm25_rang,
                    usize::try_from(want[2].as_u64().expect("rank")).expect("fits")
                );
                assert!(identical(hit.bm25_norm, float(&want[3])));
                assert_eq!(hit.derived, want[4].as_bool().expect("derived"));
                assert_eq!(hit.title, want[5].as_str().expect("title"));
                assert_eq!(hit.matched, "text");
                assert!(hit.snippet.is_empty() && hit.lines.is_none());
            }
            checked += 1;
        }
    }
    assert!(checked >= 400);
}

#[test]
fn an_empty_corpus_has_no_scores_and_no_hits() {
    let mut analyzer = Analyzer::new();
    let korpus = LexKorpus::new(Vec::new(), &mut analyzer);
    assert_eq!(korpus.notes().count(), 0);
    assert!(korpus.scores(&mut analyzer, "x").is_empty());
    assert!(korpus.hits(&mut analyzer, "x", 5).is_empty());
}

#[test]
fn the_title_bonus_counts_stems_the_query_and_the_title_share() {
    let mut analyzer = Analyzer::new();
    let a = note_rec("a.md", "---\ntitle: Garten Beet\n---\nnichts\n".to_string());
    let b = note_rec("b.md", "---\ntitle: Anderes\n---\ngarten\n".to_string());
    let korpus = LexKorpus::new(vec![a, b], &mut analyzer);
    let scores = korpus.scores(&mut analyzer, "Gärten Beeten");
    assert!(scores[0] > scores[1] + 3.0, "{scores:?}");
    assert!(korpus.scores(&mut analyzer, "")[0] == 0.0);
}

/// Notes with the same words in different amounts, titles, derived pages and links.
fn sample() -> Vec<crate::notes::NoteRec> {
    let texts = [
        (
            "a.md",
            "---\ntitle: Garten\n---\ngarten beet kaffee garten\n",
        ),
        (
            "b.md",
            "---\nclass: derived\n---\nkaffee wasserfall bergwanderung\n",
        ),
        ("c.md", "kaffee kaffee beet der die das\n"),
        (
            "d.md",
            "---\ntitle: Beet\n---\nder die das bergwanderung entscheidung\n",
        ),
        ("e.md", "---\nclass: derived\n---\ngarten der die\n"),
        ("f.md", "wasserfall der die das und ist\n"),
    ];
    texts
        .iter()
        .map(|(rel, text)| note_rec(rel, (*text).to_string()))
        .collect()
}

const ASKS: [&str; 7] = [
    "garten",
    "kaffee beet",
    "der die das",
    "bergwanderung entscheidung",
    "Gärten",
    "xyzzy",
    "",
];

fn same_answers(a: &LexKorpus, b: &LexKorpus, analyzer: &mut Analyzer) {
    for ask in ASKS {
        assert_eq!(a.scores(analyzer, ask), b.scores(analyzer, ask), "{ask:?}");
        for k in [1, 2, 5] {
            assert_eq!(
                a.hits(analyzer, ask, k),
                b.hits(analyzer, ask, k),
                "{ask:?}"
            );
        }
    }
    assert_eq!(
        a.notes().map(|n| &n.rel).collect::<Vec<_>>(),
        b.notes().map(|n| &n.rel).collect::<Vec<_>>()
    );
}

#[test]
fn a_rebuilt_index_gives_the_numbers_of_one_built_from_scratch() {
    let mut analyzer = Analyzer::new();
    let fresh = LexKorpus::new(sample(), &mut analyzer);
    let mut notes = sample();
    let late = notes.remove(2);
    let mut korpus = LexKorpus::new(notes, &mut analyzer);
    assert_eq!(korpus.entry_count(), 5);
    let plan = vec![
        (Origin::Kept(0), true),
        (Origin::Kept(1), true),
        (Origin::Fresh(late), true),
        (Origin::Kept(2), true),
        (Origin::Kept(3), true),
        (Origin::Kept(4), true),
    ];
    korpus.rebuild(plan, &mut analyzer);
    same_answers(&fresh, &korpus, &mut analyzer);
    assert_eq!(korpus.entry_count(), 6);
}

#[test]
fn a_rebuilt_index_after_a_changed_note_equals_a_fresh_one() {
    let mut analyzer = Analyzer::new();
    let mut korpus = LexKorpus::new(sample(), &mut analyzer);
    let changed = note_rec(
        "c.md",
        "---\ntitle: Kaffee Garten\n---\nneue wörter wasserfall zukunftsmusik\n".to_string(),
    );
    let plan = (0..6)
        .map(|at| {
            if at == 2 {
                (Origin::Fresh(note_rec("c.md", changed.text.clone())), true)
            } else {
                (Origin::Kept(at), true)
            }
        })
        .collect();
    korpus.rebuild(plan, &mut analyzer);
    let mut notes = sample();
    notes[2] = changed;
    let fresh = LexKorpus::new(notes, &mut analyzer);
    same_answers(&fresh, &korpus, &mut analyzer);
}

#[test]
fn a_note_left_out_of_the_corpus_is_kept_for_later_and_neither_counted_nor_found() {
    let mut analyzer = Analyzer::new();
    let mut korpus = LexKorpus::new(sample(), &mut analyzer);
    let plan = (0..6)
        .map(|at| (Origin::Kept(at), at != 2 && at != 4))
        .collect();
    korpus.rebuild(plan, &mut analyzer);
    let mut notes = sample();
    notes.remove(4);
    notes.remove(2);
    let fresh = LexKorpus::new(notes, &mut analyzer);
    same_answers(&fresh, &korpus, &mut analyzer);
    assert_eq!(korpus.entry_count(), 6);
    let all: Vec<&str> = korpus.entry_notes().map(|n| n.rel.as_str()).collect();
    assert_eq!(all, ["a.md", "b.md", "c.md", "d.md", "e.md", "f.md"]);
    assert_eq!(korpus.entry_note(2).map(|n| n.rel.as_str()), Some("c.md"));
    assert!(korpus.entry_note(6).is_none());
    let plan = (0..6).map(|at| (Origin::Kept(at), true)).collect();
    korpus.rebuild(plan, &mut analyzer);
    let again = LexKorpus::new(sample(), &mut analyzer);
    same_answers(&again, &korpus, &mut analyzer);
}

#[test]
fn removing_every_note_empties_the_index_and_a_kept_entry_that_is_not_there_is_skipped() {
    let mut analyzer = Analyzer::new();
    let mut korpus = LexKorpus::new(sample(), &mut analyzer);
    korpus.rebuild(
        vec![(Origin::Kept(99), true), (Origin::Kept(1), true)],
        &mut analyzer,
    );
    assert_eq!(korpus.entry_count(), 1);
    assert_eq!(korpus.notes().count(), 1);
    korpus.rebuild(Vec::new(), &mut analyzer);
    assert_eq!(korpus.entry_count(), 0);
    assert!(korpus.scores(&mut analyzer, "garten").is_empty());
    assert!(korpus.hits(&mut analyzer, "garten", 5).is_empty());
    korpus.rebuild(
        sample()
            .into_iter()
            .map(|n| (Origin::Fresh(n), true))
            .collect(),
        &mut analyzer,
    );
    let fresh = LexKorpus::new(sample(), &mut analyzer);
    same_answers(&fresh, &korpus, &mut analyzer);
}

#[test]
fn terms_of_a_removed_note_do_not_change_the_numbers() {
    let mut analyzer = Analyzer::new();
    let mut notes = sample();
    notes.push(note_rec(
        "g.md",
        "zukunftsmusik zukunftsmusik garten\n".to_string(),
    ));
    let mut korpus = LexKorpus::new(notes, &mut analyzer);
    let plan = (0..6).map(|at| (Origin::Kept(at), true)).collect();
    korpus.rebuild(plan, &mut analyzer);
    assert_eq!(korpus.entry_count(), 6);
    let fresh = LexKorpus::new(sample(), &mut analyzer);
    same_answers(&fresh, &korpus, &mut analyzer);
    assert!(
        korpus
            .scores(&mut analyzer, "zukunftsmusik")
            .iter()
            .all(|s| *s == 0.0)
    );
}
