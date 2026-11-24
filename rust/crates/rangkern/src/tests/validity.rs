use std::collections::BTreeMap;

use notiz_kern::frontmatter::{Field, Fields};
use serde_json::Value;

use super::data;
use crate::hit::SearchHit;
use crate::notes::note_rec;
use crate::validity::{
    Lookup, Validity, apply_validity, frontmatter_validity, is_date_prefix, link_targets,
    wikilink_targets,
};

fn field_of(value: &Value) -> Field {
    match value {
        Value::String(text) => Field::Text(text.clone()),
        Value::Array(items) => Field::List(
            items
                .iter()
                .map(|i| i.as_str().expect("text").to_string())
                .collect(),
        ),
        other => panic!("unexpected field value {other}"),
    }
}

fn fields_of(value: &Value) -> Fields {
    value
        .as_object()
        .expect("fields")
        .iter()
        .map(|(key, value)| (key.clone(), field_of(value)))
        .collect::<BTreeMap<_, _>>()
}

#[test]
fn the_validity_of_the_frontmatter_agrees_with_python() {
    let rows = data("validity");
    let rows = rows.as_array().expect("rows");
    assert!(rows.len() >= 200);
    let mut some = 0;
    for row in rows {
        let fields = fields_of(&row["fields"]);
        let today = row["today"].as_str().expect("today");
        let got = frontmatter_validity(&fields, today);
        let want = if row["out"].is_null() {
            None
        } else {
            some += 1;
            Some(Validity {
                status: row["out"][0].as_str().expect("status").to_string(),
                targets: row["out"][1]
                    .as_array()
                    .expect("targets")
                    .iter()
                    .map(|t| t.as_str().expect("text").to_string())
                    .collect(),
            })
        };
        assert_eq!(got, want, "{} on {today}", row["fields"]);
    }
    assert!(some > 100);
}

#[test]
fn link_targets_agree_with_python() {
    for row in data("links").as_array().expect("rows") {
        let field = (!row["value"].is_null()).then(|| field_of(&row["value"]));
        let want: Vec<String> = row["out"]
            .as_array()
            .expect("out")
            .iter()
            .map(|t| t.as_str().expect("text").to_string())
            .collect();
        assert_eq!(link_targets(field.as_ref()), want, "{}", row["value"]);
    }
}

#[test]
fn wikilinks_end_at_a_closing_bracket_pipe_hash_or_newline() {
    assert_eq!(
        wikilink_targets("[[a]] [[b|c]] [[d#e]] [[f\ng]]"),
        ["a", "b", "d", "f"]
    );
    assert_eq!(wikilink_targets("[[[x]]"), ["[x"]);
    assert_eq!(wikilink_targets("[[]] [[ok]]"), ["ok"]);
    assert!(wikilink_targets("[[").is_empty());
    assert!(wikilink_targets("kein link").is_empty());
}

#[test]
fn a_date_prefix_is_year_month_or_day_with_ascii_digits() {
    for good in ["2026", "2026-09", "2026-09-30"] {
        assert!(is_date_prefix(good), "{good}");
    }
    for bad in [
        "",
        "26",
        "2026-9",
        "2026-09-3",
        "2026-09-30-1",
        "2026/09",
        "20x6",
        "٢٠٢٦",
        "2026-09-",
    ] {
        assert!(!is_date_prefix(bad), "{bad}");
    }
}

fn hit(rel: &str, score: f64) -> SearchHit {
    SearchHit::new(rel, score)
}

#[test]
fn a_superseded_hit_is_demoted_and_its_successor_takes_its_old_score() {
    let old = note_rec(
        "old.md",
        "---\ntitle: Alt\nstatus: superseded\nsuperseded-by: \"[[neu]]\"\n---\ntext\n".into(),
    );
    let new = note_rec("neu.md", "---\ntitle: Neu\n---\ntext\n".into());
    let other = note_rec("x.md", "text\n".into());
    let notes = vec![old, new, other];
    let lookup = Lookup::new(&notes);
    let mut hits = vec![hit("old.md", 1.0), hit("x.md", 0.6)];
    apply_validity(&mut hits, &lookup, "2026-11-18");
    let order: Vec<(&str, &str)> = hits
        .iter()
        .map(|h| (h.rel.as_str(), h.matched.as_str()))
        .collect();
    assert_eq!(
        order,
        [("neu.md", "nachfolger"), ("x.md", ""), ("old.md", "")]
    );
    let old_hit = hits.iter().find(|h| h.rel == "old.md").expect("old");
    assert_eq!(old_hit.status, "superseded");
    assert_eq!(old_hit.superseded_by, ["neu"]);
    assert_eq!(old_hit.superseded_by_rel, ["neu.md"]);
    assert_eq!(old_hit.score, 0.49);
    let successor = hits.iter().find(|h| h.rel == "neu.md").expect("successor");
    assert_eq!((successor.score, successor.title.as_str()), (1.0, "Neu"));
}

#[test]
fn a_present_successor_below_the_old_score_is_lifted_but_never_lowered() {
    let notes = vec![
        note_rec("old.md", "---\nsuperseded-by: \"[[neu]]\"\n---\n".into()),
        note_rec("neu.md", "text\n".into()),
        note_rec("old2.md", "---\nsuperseded-by: \"[[neu2]]\"\n---\n".into()),
        note_rec("neu2.md", "text\n".into()),
    ];
    let lookup = Lookup::new(&notes);
    let mut hits = vec![
        hit("old.md", 1.0),
        hit("neu.md", 0.3),
        hit("old2.md", 0.5),
        hit("neu2.md", 0.9),
    ];
    apply_validity(&mut hits, &lookup, "2026-11-18");
    let score = |rel: &str| hits.iter().find(|h| h.rel == rel).expect("hit").score;
    assert_eq!(score("neu.md"), 1.0);
    assert_eq!(score("neu2.md"), 0.9);
    assert_eq!(hits.len(), 4);
}

#[test]
fn successor_chains_stop_after_four_steps_and_at_a_cycle() {
    let notes: Vec<_> = (0..7)
        .map(|i| {
            let text = if i < 6 {
                format!("---\nsuperseded-by: \"[[n{}]]\"\n---\n", i + 1)
            } else {
                "x\n".to_string()
            };
            note_rec(&format!("n{i}.md"), text)
        })
        .collect();
    let lookup = Lookup::new(&notes);
    let mut hits = vec![hit("n0.md", 1.0)];
    apply_validity(&mut hits, &lookup, "2026-11-18");
    let first = hits.iter().find(|h| h.rel == "n0.md").expect("n0");
    assert_eq!(first.status, "superseded");
    assert_eq!(first.superseded_by, ["n1"]);
    assert_eq!(first.superseded_by_rel, ["n5.md"].map(String::from));
    let cycle = vec![
        note_rec("a.md", "---\nsuperseded-by: \"[[b]]\"\n---\n".into()),
        note_rec("b.md", "---\nsuperseded-by: \"[[a]]\"\n---\n".into()),
    ];
    let lookup = Lookup::new(&cycle);
    let mut hits = vec![hit("a.md", 1.0)];
    apply_validity(&mut hits, &lookup, "2026-11-18");
    let first = hits.iter().find(|h| h.rel == "a.md").expect("a");
    assert_eq!(first.superseded_by_rel, ["b.md"].map(String::from));
    let mut unknown = vec![hit("a.md", 1.0)];
    let lonely = vec![note_rec(
        "a.md",
        "---\nstatus: archived\nsuperseded-by: \"[[nichts]]\"\n---\n".into(),
    )];
    apply_validity(&mut unknown, &Lookup::new(&lonely), "2026-11-18");
    assert!(unknown[0].superseded_by_rel.is_empty());
    assert_eq!(unknown[0].superseded_by, ["nichts"]);
}

#[test]
fn valid_and_unknown_notes_are_left_alone() {
    let notes = vec![
        note_rec("a.md", "---\nstatus: active\n---\n".into()),
        note_rec("b.md", "---\nvalid-until: 2026-12\n---\n".into()),
    ];
    let lookup = Lookup::new(&notes);
    let mut hits = vec![hit("a.md", 1.0), hit("b.md", 0.5), hit("fremd.md", 0.2)];
    apply_validity(&mut hits, &lookup, "2026-11-18");
    assert_eq!(
        hits.iter().map(|h| h.score).collect::<Vec<_>>(),
        [1.0, 0.5, 0.2]
    );
    assert!(hits.iter().all(|h| h.status.is_empty()));
    let mut expired = vec![hit("b.md", 1.0)];
    apply_validity(&mut expired, &lookup, "2027-01-01");
    assert_eq!(expired[0].status, "expired");
    assert!(expired[0].superseded_by.is_empty());
}

#[test]
fn the_lookup_resolves_stem_title_and_alias_and_the_first_note_wins() {
    let notes = vec![
        note_rec(
            "d/Erste Notiz.md",
            "---\ntitle: Titel Eins\naliases: [Kurz, Lang]\n---\n".into(),
        ),
        note_rec("e/kurz.md", "x\n".into()),
        note_rec("f/n.md", "---\naliases: Einzel\n---\n".into()),
    ];
    let lookup = Lookup::new(&notes);
    assert_eq!(lookup.resolve("erste notiz"), Some(0));
    assert_eq!(lookup.resolve("  TITEL eins "), Some(0));
    assert_eq!(lookup.resolve("lang"), Some(0));
    assert_eq!(lookup.resolve("Kurz"), Some(0));
    assert_eq!(lookup.resolve("nirgends"), None);
    assert_eq!(lookup.resolve("einzel"), Some(2));
    assert!(lookup.note("e/kurz.md").is_some());
    assert!(lookup.note("e/nein.md").is_none());
}

#[test]
fn a_valid_until_that_is_no_date_never_expires() {
    for until in ["1999-xx", "abc", "2026-1-1", "20", "1999-01-01-01"] {
        let fields: Fields = [("valid-until".to_string(), Field::Text(until.to_string()))].into();
        let validity = frontmatter_validity(&fields, "2026-11-18").expect("a field is set");
        assert_eq!(validity.status, "active", "{until}");
    }
    let past: Fields = [(
        "valid-until".to_string(),
        Field::Text("1999-01".to_string()),
    )]
    .into();
    assert_eq!(
        frontmatter_validity(&past, "2026-11-18")
            .expect("set")
            .status,
        "expired"
    );
}

#[test]
fn the_chain_stops_at_a_note_that_names_no_successor_or_is_not_superseded() {
    let notes = vec![
        note_rec("a.md", "---\nsuperseded-by: \"[[b]]\"\n---\n".into()),
        note_rec("b.md", "---\nstatus: superseded\n---\n".into()),
        note_rec("c.md", "---\nsuperseded-by: \"[[d]]\"\n---\n".into()),
        note_rec(
            "d.md",
            "---\nstatus: archived\nsuperseded-by: \"[[e]]\"\n---\n".into(),
        ),
        note_rec("e.md", "x\n".into()),
    ];
    let lookup = Lookup::new(&notes);
    let mut first = vec![hit("a.md", 1.0)];
    apply_validity(&mut first, &lookup, "2026-11-18");
    assert_eq!(
        first
            .iter()
            .find(|h| h.rel == "a.md")
            .expect("a")
            .superseded_by_rel,
        ["b.md"]
    );
    let mut second = vec![hit("c.md", 1.0)];
    apply_validity(&mut second, &lookup, "2026-11-18");
    assert_eq!(
        second
            .iter()
            .find(|h| h.rel == "c.md")
            .expect("c")
            .superseded_by_rel,
        ["d.md"]
    );
}

#[test]
fn an_equal_successor_score_is_not_a_change_and_the_order_stays() {
    let notes = vec![
        note_rec("old.md", "---\nsuperseded-by: \"[[neu]]\"\n---\n".into()),
        note_rec("neu.md", "x\n".into()),
    ];
    let lookup = Lookup::new(&notes);
    let mut hits = vec![hit("neu.md", 1.0), hit("old.md", 1.0)];
    apply_validity(&mut hits, &lookup, "2026-11-18");
    assert_eq!(
        hits.iter().map(|h| h.rel.as_str()).collect::<Vec<_>>(),
        ["neu.md", "old.md"]
    );
    assert_eq!(hits[0].score, 1.0);
}
