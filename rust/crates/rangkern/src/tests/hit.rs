use super::{data, float, identical};
use crate::hit::{HYBRID_POOL, HYBRID_POOL_WIDE, fuse_hybrid, fuse_hybrid_with};
use crate::hit::{
    SearchHit, apply_nav_boost, cosine, demote_derived, hybrid_pool, is_navigational,
    max_normalize, pool_trim, rrf_fuse, semantic_ranking, sort_by_score,
};

fn hit_of(value: &serde_json::Value) -> SearchHit {
    let mut hit = SearchHit::new(value["rel"].as_str().expect("rel"), float(&value["score"]));
    hit.title = value["title"].as_str().expect("title").to_string();
    hit.matched = value["matched"].as_str().expect("match").to_string();
    hit.cosine = float(&value["cosine"]);
    hit.bm25_rang = usize::try_from(value["bm25_rang"].as_u64().expect("rank")).expect("fits");
    hit.bm25_norm = float(&value["bm25_norm"]);
    hit.vektor_rang = usize::try_from(value["vektor_rang"].as_u64().expect("rank")).expect("fits");
    hit.derived = value["derived"].as_bool().expect("derived");
    hit
}

#[test]
fn the_fusion_agrees_with_python_on_invented_lists() {
    let cases = data("fuse");
    let cases = cases.as_array().expect("cases");
    assert!(cases.len() >= 60);
    for case in cases {
        let semantic: Vec<SearchHit> = case["semantic"]
            .as_array()
            .expect("list")
            .iter()
            .map(hit_of)
            .collect();
        let text: Vec<SearchHit> = case["text"]
            .as_array()
            .expect("list")
            .iter()
            .map(hit_of)
            .collect();
        let query = case["query"].as_str().expect("query");
        let k = usize::try_from(case["k"].as_u64().expect("k")).expect("fits");
        let got = fuse_hybrid(&semantic, &text, query, k);
        let want = case["out"].as_array().expect("out");
        assert_eq!(got.len(), want.len(), "{query:?} k {k}");
        for (hit, want) in got.iter().zip(want) {
            let expected = hit_of(want);
            assert_eq!(hit.rel, expected.rel);
            assert!(
                identical(hit.score, expected.score),
                "{} vs {}",
                hit.score,
                expected.score
            );
            assert_eq!(hit.title, expected.title);
            assert_eq!(hit.matched, expected.matched, "{}", hit.rel);
            assert!(identical(hit.cosine, expected.cosine));
            assert_eq!(hit.bm25_rang, expected.bm25_rang);
            assert!(identical(hit.bm25_norm, expected.bm25_norm));
            assert_eq!(hit.vektor_rang, expected.vektor_rang);
            assert_eq!(hit.derived, expected.derived);
        }
    }
}

#[test]
fn the_cosine_is_the_same_to_the_last_bit() {
    let rows = data("cosine");
    let rows = rows.as_array().expect("rows");
    assert!(rows.len() >= 120);
    for row in rows {
        let parse = |key: &str| -> Vec<f64> {
            row[key]
                .as_array()
                .expect("list")
                .iter()
                .map(float)
                .collect()
        };
        assert_eq!(
            cosine(&parse("a"), &parse("b")),
            float(&row["cosine"]),
            "{row}"
        );
    }
    assert_eq!(cosine(&[], &[1.0]), 0.0);
    assert_eq!(cosine(&[0.0, 0.0], &[1.0, 1.0]), 0.0);
    assert_eq!(cosine(&[1.0, 0.0], &[0.0, 0.0]), 0.0);
}

fn view(list: &[(String, Vec<f64>)]) -> Vec<(&str, &[f64])> {
    list.iter()
        .map(|(rel, vector)| (rel.as_str(), vector.as_slice()))
        .collect()
}

#[test]
fn the_semantic_list_keeps_k_plus_fifty_and_numbers_the_ranks() {
    let entries: Vec<(String, Vec<f64>)> = (0..80)
        .map(|i| (format!("n{i}.md"), vec![1.0, f64::from(i) / 100.0]))
        .collect();
    let hits = semantic_ranking(&[1.0, 0.0], view(&entries), 3);
    assert_eq!(hits.len(), 53);
    assert_eq!(hits[0].rel, "n0.md");
    assert_eq!(hits[1].rel, "n1.md");
    assert_eq!((hits[0].vektor_rang, hits[52].vektor_rang), (1, 53));
    assert!(
        hits.iter()
            .all(|hit| hit.cosine == hit.score && hit.matched == "semantic")
    );
    assert_eq!(semantic_ranking(&[1.0], view(&entries[..2]), 5).len(), 2);
    assert_eq!(
        semantic_ranking(&[1.0], view(&entries[..2]), usize::MAX).len(),
        2
    );
}

#[test]
fn equal_scores_keep_their_order() {
    let mut hits: Vec<SearchHit> = ["a", "b", "c", "d"]
        .iter()
        .zip([1.0, 2.0, 1.0, 2.0])
        .map(|(rel, score)| SearchHit::new(rel, score))
        .collect();
    sort_by_score(&mut hits);
    let order: Vec<&str> = hits.iter().map(|hit| hit.rel.as_str()).collect();
    assert_eq!(order, ["b", "d", "a", "c"]);
}

#[test]
fn normalising_divides_by_the_first_positive_score_only() {
    let mut hits = vec![SearchHit::new("a", 4.0), SearchHit::new("b", 2.0)];
    max_normalize(&mut hits);
    assert_eq!((hits[0].score, hits[1].score), (1.0, 0.5));
    let mut zero = vec![SearchHit::new("a", 0.0), SearchHit::new("b", 2.0)];
    max_normalize(&mut zero);
    assert_eq!((zero[0].score, zero[1].score), (0.0, 2.0));
    let mut negative = vec![SearchHit::new("a", -1.0)];
    max_normalize(&mut negative);
    assert_eq!(negative[0].score, -1.0);
    max_normalize(&mut []);
}

#[test]
fn the_pool_has_k_source_hits_and_the_derived_pages_between_them() {
    let make = |rel: &str, derived: bool| {
        let mut hit = SearchHit::new(rel, 1.0);
        hit.derived = derived;
        hit
    };
    let hits = vec![
        make("d1", true),
        make("s1", false),
        make("d2", true),
        make("s2", false),
        make("s3", false),
        make("d3", true),
    ];
    let kept: Vec<String> = pool_trim(hits.clone(), 2)
        .into_iter()
        .map(|hit| hit.rel)
        .collect();
    assert_eq!(kept, ["d1", "s1", "d2", "s2"]);
    assert_eq!(pool_trim(hits.clone(), 10).len(), 6);
    assert!(pool_trim(hits, 0).is_empty());
}

#[test]
fn the_pool_does_not_depend_on_k() {
    for k in [0, 1, 5, 6, 10, 20, 40, 50, 200, usize::MAX] {
        assert_eq!(hybrid_pool(k, false), HYBRID_POOL, "k {k}");
        assert_eq!(hybrid_pool(k, true), HYBRID_POOL_WIDE, "k {k}");
    }
    assert_eq!(HYBRID_POOL, 40);
}

/// A deterministic pseudo random list of `n` hits over `names`, as a candidate list of one ranker.
fn invented_list(names: &[String], n: usize, seed: u64, matched: &str) -> Vec<SearchHit> {
    let mut state = seed;
    let mut order: Vec<usize> = (0..names.len()).collect();
    for i in (1..order.len()).rev() {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        let j = usize::try_from(state >> 33).expect("fits") % (i + 1);
        order.swap(i, j);
    }
    order
        .into_iter()
        .take(n)
        .enumerate()
        .map(|(i, name)| {
            let mut hit = SearchHit::new(
                &names[name],
                1.0 - 0.001 * f64::from(u32::try_from(i).expect("small")),
            );
            hit.matched = matched.to_string();
            hit.derived = name % 5 == 0;
            hit.bm25_rang = i + 1;
            hit.vektor_rang = i + 1;
            hit
        })
        .collect()
}

fn rels(hits: &[SearchHit]) -> Vec<&str> {
    hits.iter().map(|hit| hit.rel.as_str()).collect()
}

#[test]
fn a_longer_hybrid_list_extends_a_shorter_one() {
    let mut names: Vec<String> = (0..150)
        .map(|i| format!("20-projects/p/n{i:03}.md"))
        .collect();
    names.push("20-projects/p/STATUS.md".to_string());
    names.push("20-projects/p/MOC.md".to_string());
    for (query, seed) in [("Stand von p", 1_u64), ("alpha beta", 2)] {
        let semantic = invented_list(&names, 150, seed, "semantic");
        let text = invented_list(&names, 150, seed + 10, "text");
        let long = fuse_hybrid(&semantic, &text, query, 50);
        for k in [1, 2, 3, 5, 8, 10, 20, 30, 40] {
            let short = fuse_hybrid(&semantic, &text, query, k);
            assert_eq!(rels(&short), rels(&long)[..k], "{query:?} k {k}");
        }
    }
}

#[test]
fn the_validity_acts_on_the_whole_fused_list_before_the_cut() {
    let names: Vec<String> = (0..120).map(|i| format!("n{i:03}.md")).collect();
    let semantic = invented_list(&names, 120, 3, "semantic");
    let text = invented_list(&names, 120, 4, "text");
    // A stand-in for the validity: the first six hits of the list lose almost all their score.
    let demote = |hits: &mut Vec<SearchHit>| {
        for hit in hits.iter_mut().take(6) {
            hit.score *= 0.05;
        }
        hits.sort_by(|a, b| b.score.total_cmp(&a.score));
    };
    let long = fuse_hybrid_with(&semantic, &text, "alpha", 50, demote);
    for k in [1, 3, 5, 8, 10, 20, 30] {
        let seen = std::cell::Cell::new(0);
        let short = fuse_hybrid_with(&semantic, &text, "alpha", k, |hits| {
            seen.set(hits.len());
            demote(hits);
        });
        assert!(
            seen.get() > 50,
            "the validity sees the whole list, not the first {k}"
        );
        assert_eq!(rels(&short), rels(&long)[..k], "k {k}");
    }
}

#[test]
fn navigational_questions_boost_the_hubs_and_nothing_else() {
    for question in [
        "Wo fängt es an",
        "wo ",
        "Stand von X",
        "stand vom x",
        "Stand beim Z",
        "aktueller Stand",
        "Übersicht",
        "UEBERSICHT",
        "einstieg",
        "welche Entscheidungen",
        "Entscheidungen zu",
        "entscheidungen gibt",
        "x MOC",
        "Status von a",
        "status vom a",
        "Themen-Hub",
        "themenhub",
        "wo faengt",
    ] {
        assert!(is_navigational(question), "{question}");
    }
    assert!(!is_navigational("garten") && !is_navigational("wo") && !is_navigational("moc"));
    let mut hits = vec![
        SearchHit::new("a/n.md", 1.0),
        SearchHit::new("a/MOC.md", 0.9),
        SearchHit::new("b/STATUS.md", 0.5),
        SearchHit::new("MOC.md.bak", 0.99),
    ];
    apply_nav_boost(&mut hits, "Stand von a");
    let order: Vec<(&str, f64)> = hits
        .iter()
        .map(|hit| (hit.rel.as_str(), hit.score))
        .collect();
    assert_eq!(order[0].0, "a/MOC.md");
    assert!(identical(order[0].1, 1.05));
    assert_eq!(order[1].0, "a/n.md");
    assert_eq!(order[2].0, "MOC.md.bak");
    assert!(identical(order[3].1, 0.65));
    let mut plain = vec![SearchHit::new("a/MOC.md", 0.5), SearchHit::new("b.md", 0.9)];
    apply_nav_boost(&mut plain, "garten");
    assert_eq!((plain[0].score, plain[1].score), (0.5, 0.9));
    apply_nav_boost(&mut [], "wo ");
}

#[test]
fn derived_pages_lose_half_their_score_and_the_list_is_normalised_again() {
    let mut hits = vec![SearchHit::new("d", 1.0), SearchHit::new("s", 0.6)];
    hits[0].derived = true;
    demote_derived(&mut hits);
    assert_eq!(hits[0].rel, "s");
    assert_eq!(hits[0].score, 1.0);
    assert!(identical(hits[1].score, 0.5 / 0.6));
    let mut top = vec![SearchHit::new("d", 1.0), SearchHit::new("s", 0.4)];
    top[0].derived = true;
    demote_derived(&mut top);
    assert_eq!(top[0].rel, "d");
    assert_eq!(top[0].score, 1.0);
    assert!(identical(top[1].score, 0.8));
}

#[test]
fn derived_demotion_changes_nothing_without_a_derived_page() {
    let mut hits = vec![SearchHit::new("a", 3.0), SearchHit::new("b", 1.0)];
    demote_derived(&mut hits);
    assert_eq!((hits[0].score, hits[1].score), (3.0, 1.0));
}

#[test]
fn a_hit_without_a_rank_gets_its_position_as_rank() {
    let semantic = vec![SearchHit::new("a.md", 0.9), SearchHit::new("b.md", 0.8)];
    let text = vec![SearchHit::new("c.md", 3.0), SearchHit::new("a.md", 2.0)];
    let fused = rrf_fuse(&semantic, &text, 10);
    let by = |rel: &str| fused.iter().find(|h| h.rel == rel).expect("hit").clone();
    assert_eq!((by("a.md").vektor_rang, by("a.md").bm25_rang), (1, 2));
    assert_eq!((by("b.md").vektor_rang, by("b.md").bm25_rang), (2, 0));
    assert_eq!((by("c.md").vektor_rang, by("c.md").bm25_rang), (0, 1));
    assert_eq!(by("a.md").matched, "hybrid");
    assert_eq!(by("b.md").matched, "semantic");
    assert_eq!(by("c.md").matched, "text");
}

#[test]
fn the_first_hit_with_a_title_gives_title_snippet_space_and_lines() {
    let mut first = SearchHit::new("a.md", 1.0);
    first.title = "Erster".into();
    first.snippet = "s1".into();
    first.space = "x".into();
    first.lines = Some((1, 2));
    let mut second = SearchHit::new("a.md", 1.0);
    second.title = "Zweiter".into();
    second.snippet = "s2".into();
    second.space = "y".into();
    second.lines = Some((3, 4));
    let fused = rrf_fuse(&[first.clone()], &[second.clone()], 5);
    assert_eq!(
        (
            fused[0].title.as_str(),
            fused[0].snippet.as_str(),
            fused[0].space.as_str(),
            fused[0].lines
        ),
        ("Erster", "s1", "x", Some((1, 2)))
    );
    let mut untitled = SearchHit::new("a.md", 1.0);
    untitled.snippet = "s0".into();
    let later = rrf_fuse(&[untitled], &[second], 5);
    assert_eq!(
        (
            later[0].title.as_str(),
            later[0].snippet.as_str(),
            later[0].space.as_str(),
            later[0].lines
        ),
        ("Zweiter", "s2", "y", Some((3, 4)))
    );
}
