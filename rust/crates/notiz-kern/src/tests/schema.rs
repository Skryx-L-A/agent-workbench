//! Tests of the schema 4 lint.

use crate::schema::{Finding, findings, in_scope, lint, schema_fields};

const ID: &str = "01N7QWB2XHD4RT9ZKMA3VC5EPY";

fn note(extra: &str) -> String {
    format!("---\nid: {ID}\nschema: 4\nclass: knowledge\ntype: note\n{extra}---\nbody\n")
}

fn kinds(rel: &str, text: &str) -> Vec<&'static str> {
    findings(rel, text).iter().map(|f| f.kind).collect()
}

fn finding(kind: &'static str, detail: &str) -> Finding {
    Finding {
        kind,
        detail: detail.to_string(),
    }
}

#[test]
fn scope_needs_md_suffix_and_a_known_branch() {
    for rel in [
        "00-sources/a.md",
        "10-global/a.md",
        "20-projects/p/a.md",
        "30-topics/t/sub/a.md",
        "40-people/a.md",
    ] {
        assert!(in_scope(rel), "{rel}");
    }
    for rel in [
        "a.md",
        "10-global/a.txt",
        "10-global/a.MD",
        "50-other/a.md",
        "10-globalx/a.md",
        "10-global",
        "x/10-global/a.md",
        "",
    ] {
        assert!(!in_scope(rel), "{rel}");
    }
}

#[test]
fn scope_excludes_assets_dot_parts_and_generated_names() {
    for rel in [
        "10-global/_assets/a.md",
        "10-global/.hidden/a.md",
        "10-global/.a.md",
        "10-global/.md",
        "10-global/_assets",
        "10-global/MOC.md",
        "10-global/DECISIONS.md",
        "10-global/README.md",
        "10-global/brain-vorschlaege.md",
        "10-global/review-queue.md",
        "10-global/gardener-report-1.md",
        "10-global/brain-health-1.md",
        "10-global/dream-report-1.md",
        "10-global/brain-run-1.md",
        "10-global/brain-vorschlaege-x.md",
    ] {
        assert!(!in_scope(rel), "{rel}");
    }
    for rel in [
        "10-global/_assets2/a.md",
        "10-global/moc.md",
        "10-global/a-gardener-report-.md",
        "10-global/readme.md",
    ] {
        assert!(in_scope(rel), "{rel}");
    }
}

#[test]
fn schema_fields_cuts_comments_after_one_whitespace_and_strips_quotes() {
    let text = "---\na: b #c\nd: e#f\ng: 'h'\ni: \"j\"\nk: \"'l'\"\nm:\nn:\tx\n o: p\n- q\nr: s # t u\n---\n";
    let (fields, blocks) = schema_fields(text);
    assert_eq!(blocks, 1);
    let get = |k: &str| fields.get(k).map(String::as_str);
    assert_eq!(get("a"), Some("b"));
    assert_eq!(get("d"), Some("e#f"));
    assert_eq!(get("g"), Some("h"));
    assert_eq!(get("i"), Some("j"));
    assert_eq!(get("k"), Some("l"));
    assert_eq!(get("m"), Some(""));
    assert_eq!(get("n"), Some("x"));
    assert_eq!(get("r"), Some("s"));
    assert_eq!(fields.len(), 8);
}

#[test]
fn schema_fields_later_blocks_win_and_a_bom_is_dropped() {
    let (fields, blocks) = schema_fields("\u{feff}---\na: 1\nb: 2\n---\n\n---\na: 3\n---\nbody");
    assert_eq!(blocks, 2);
    assert_eq!(fields["a"], "3");
    assert_eq!(fields["b"], "2");
}

#[test]
fn schema_fields_without_frontmatter_is_empty() {
    let (fields, blocks) = schema_fields("no frontmatter\na: 1\n");
    assert_eq!(fields.len(), 0);
    assert_eq!(blocks, 0);
}

#[test]
fn findings_without_frontmatter() {
    assert_eq!(
        findings("10-global/a.md", "plain"),
        [finding("pflichtfeld", "kein Frontmatter")]
    );
}

#[test]
fn findings_a_valid_note_has_none() {
    assert_eq!(findings("10-global/a.md", &note("")).len(), 0);
    assert_eq!(
        findings("10-global/a.md", &note("created: 2026-10-01\ntitle: x\n")).len(),
        0
    );
}

#[test]
fn findings_generated_pages_are_exempt() {
    assert_eq!(
        findings("10-global/a.md", "---\nclass: derived\n---\n").len(),
        0
    );
    assert_eq!(
        findings(
            "10-global/a.md",
            "---\ngardener-generated: true\nid: bad\n---\n"
        )
        .len(),
        0
    );
    assert_eq!(
        kinds("10-global/a.md", "---\nclass: knowledge\n---\n"),
        ["pflichtfeld"]
    );
}

#[test]
fn findings_name_the_missing_required_fields_in_order() {
    let found = findings("10-global/a.md", "---\ntitle: x\n---\n");
    assert_eq!(
        found,
        [finding(
            "pflichtfeld",
            "Pflichtfeld fehlt: id, schema, class, type"
        )]
    );
    let found = findings(
        "10-global/a.md",
        &format!("---\nid: {ID}\nclass: knowledge\n---\n"),
    );
    assert_eq!(
        found,
        [finding("pflichtfeld", "Pflichtfeld fehlt: schema, type")]
    );
    let found = findings(
        "10-global/a.md",
        &format!("---\nid: {ID}\nschema:\nclass: knowledge\ntype: note\n---\n"),
    );
    assert_eq!(found, [finding("pflichtfeld", "Pflichtfeld fehlt: schema")]);
}

#[test]
fn findings_check_the_ulid_character_set_and_length() {
    for id in ["0123456789ABCDEFGHJKMNPQRS", "TVWXYZ0123456789ABCDEFGHJK"] {
        let text = format!("---\nid: {id}\nschema: 4\nclass: knowledge\ntype: note\n---\n");
        assert!(findings("10-global/a.md", &text).is_empty(), "{id}");
    }
    for bad in ['I', 'L', 'O', 'U', 'a', 'z', '@', '[', '/', ':', '\u{e4}'] {
        let id = format!("{bad}123456789ABCDEFGHJKMNPQRS");
        let text = format!("---\nid: {id}\nschema: 4\nclass: knowledge\ntype: note\n---\n");
        let found = findings("10-global/a.md", &text);
        assert_eq!(
            found,
            [finding("id-format", &format!("id '{id}' ist keine ULID"))],
            "{bad:?}"
        );
    }
    for id in ["0123456789ABCDEFGHJKMNPQR", "0123456789ABCDEFGHJKMNPQRST"] {
        let text = format!("---\nid: {id}\nschema: 4\nclass: knowledge\ntype: note\n---\n");
        assert_eq!(kinds("10-global/a.md", &text), ["id-format"], "{id}");
    }
}

#[test]
fn findings_shorten_a_long_id_to_thirty_characters() {
    let id = "0123456789ABCDEFGHJKMNPQRSTVWXYZ0123456789";
    let text = format!("---\nid: {id}\nschema: 4\nclass: knowledge\ntype: note\n---\n");
    assert_eq!(
        findings("10-global/a.md", &text),
        [finding(
            "id-format",
            "id '0123456789ABCDEFGHJKMNPQRSTVWX' ist keine ULID"
        )]
    );
}

#[test]
fn findings_check_type_and_class_vocabulary() {
    let text = format!("---\nid: {ID}\nschema: 4\nclass: weird\ntype: bogus\n---\n");
    assert_eq!(
        findings("10-global/a.md", &text),
        [
            finding(
                "typ",
                "type 'bogus' nicht im Vokabular (asset|decision|incident|note|person|pitfall|reference|report|secret|session|system|term)"
            ),
            finding(
                "klasse",
                "class 'weird' unbekannt (derived|knowledge|meta|source)"
            ),
        ]
    );
    for kind in [
        "asset",
        "decision",
        "incident",
        "note",
        "person",
        "pitfall",
        "reference",
        "report",
        "secret",
        "session",
        "system",
        "term",
    ] {
        let class = if kind == "session" {
            "source"
        } else {
            "knowledge"
        };
        let text = format!("---\nid: {ID}\nschema: 4\nclass: {class}\ntype: {kind}\n---\n");
        assert!(findings("10-global/a.md", &text).is_empty(), "{kind}");
    }
    for class in ["knowledge", "meta", "source"] {
        let text = format!("---\nid: {ID}\nschema: 4\nclass: {class}\ntype: note\n---\n");
        assert!(findings("10-global/a.md", &text).is_empty(), "{class}");
    }
}

#[test]
fn findings_class_must_match_sessions_and_sources() {
    let with = |class: &str, kind: &str| {
        format!("---\nid: {ID}\nschema: 4\nclass: {class}\ntype: {kind}\n---\n")
    };
    assert_eq!(
        findings("20-projects/p/sessions/x.md", &with("knowledge", "note")),
        [finding(
            "klasse-pfad",
            "Pfad sessions/ oder 00-sources/ verlangt class: source, steht class: knowledge"
        )]
    );
    assert_eq!(
        findings("00-sources/x.md", &with("meta", "note")),
        [finding(
            "klasse-pfad",
            "Pfad sessions/ oder 00-sources/ verlangt class: source, steht class: meta"
        )]
    );
    assert_eq!(
        findings("10-global/x.md", &with("knowledge", "session")),
        [finding(
            "klasse-pfad",
            "type: session verlangt class: source, steht class: knowledge"
        )]
    );
    assert_eq!(
        findings("00-sources/x.md", &with("knowledge", "session")),
        [finding(
            "klasse-pfad",
            "type: session verlangt class: source, steht class: knowledge"
        )]
    );
}

#[test]
fn findings_class_against_path_has_exceptions() {
    let with = |class: &str, kind: &str| {
        format!("---\nid: {ID}\nschema: 4\nclass: {class}\ntype: {kind}\n---\n")
    };
    assert_eq!(
        findings("20-projects/p/sessions/x.md", &with("source", "note")).len(),
        0
    );
    assert_eq!(
        findings("20-projects/sessions", &with("knowledge", "note")).len(),
        0
    );
    assert_eq!(
        findings("20-projects/sessions2/x.md", &with("knowledge", "note")).len(),
        0
    );
    assert_eq!(
        findings("00-sourcesx/x.md", &with("knowledge", "note")).len(),
        0
    );
    assert_eq!(kinds("00-sources/x.md", &with("weird", "note")), ["klasse"]);
    assert_eq!(
        kinds("00-sources/x.md", "---\nid: bad\ntype: note\n---\n"),
        ["pflichtfeld", "id-format"]
    );
}

#[test]
fn findings_several_blocks_and_memory_leftovers() {
    let text = format!(
        "---\nname: x\n---\n\n---\nid: {ID}\nschema: 4\nclass: knowledge\ntype: note\nmetadata: y\n---\n"
    );
    assert_eq!(
        findings("10-global/a.md", &text),
        [
            finding(
                "mehrfach-frontmatter",
                "2 Frontmatter-Bloecke (Basic-Memory-Rest)"
            ),
            finding("memory-rest", "Claude-Memory-Felder: name:, metadata:"),
        ]
    );
    assert_eq!(
        findings("10-global/a.md", &note("name: x\n")),
        [finding("memory-rest", "Claude-Memory-Felder: name:")]
    );
    assert_eq!(
        findings("10-global/a.md", &note("metadata: x\n")),
        [finding("memory-rest", "Claude-Memory-Felder: metadata:")]
    );
}

#[test]
fn findings_come_in_a_fixed_order() {
    let text = "---\nname: x\nid: bad\nclass: weird\ntype: bogus\n---\n\n---\na: 1\n---\n";
    assert_eq!(
        kinds("00-sources/x.md", text),
        [
            "pflichtfeld",
            "id-format",
            "typ",
            "klasse",
            "mehrfach-frontmatter",
            "memory-rest"
        ]
    );
    let text =
        "---\nid: bad\nschema: 4\nclass: knowledge\ntype: session\n---\n\n---\nname: x\n---\n";
    assert_eq!(
        kinds("10-global/x.md", text),
        [
            "id-format",
            "klasse-pfad",
            "mehrfach-frontmatter",
            "memory-rest"
        ]
    );
}

#[test]
fn lint_reports_scope_fields_blocks_findings_and_newness() {
    let report = lint("10-global/a.md", &note("created: 2026-10-01\n"));
    assert!(report.in_scope);
    assert_eq!(report.blocks, 1);
    assert_eq!(report.fields["created"], "2026-10-01");
    assert_eq!(report.fields["id"], ID);
    assert_eq!(report.findings.len(), 0);
    assert!(report.is_new);
    let report = lint("a.md", "plain");
    assert!(!report.in_scope);
    assert_eq!(report.blocks, 0);
    assert!(!report.is_new);
    assert_eq!(report.findings.len(), 1);
}

#[test]
fn lint_new_starts_on_the_cutoff_day() {
    let is_new =
        |created: &str| lint("10-global/a.md", &format!("---\ncreated: {created}\n---\n")).is_new;
    for created in [
        "2026-09-28",
        "2026-09-28T10:00",
        "2026-09-29",
        "2026-10-01",
        "2027-01-01",
        "2026-09-28 x",
        "'2026-10-02'",
        "\"2026-10-02\"",
    ] {
        assert!(is_new(created), "{created}");
    }
    for created in [
        "2026-09-27",
        "2026-09-27T23:59",
        "2025-12-31",
        "2026-9-28",
        "20260928xx",
        "",
        "2026-09",
        "x",
        "\u{e4}026-10-01",
        "2026-09-2",
    ] {
        assert!(!is_new(created), "{created}");
    }
    assert!(!lint("10-global/a.md", "---\ntitle: x\n---\n").is_new);
}

#[test]
fn lint_new_checks_every_position_of_the_date() {
    for position in 0..10 {
        let mut chars: Vec<char> = "2026-10-01".chars().collect();
        chars[position] = 'x';
        let created: String = chars.into_iter().collect();
        let text = format!("---\ncreated: {created}\n---\n");
        assert!(!lint("10-global/a.md", &text).is_new, "{created}");
    }
    let text = "---\ncreated: 2026+10+01\n---\n";
    assert!(!lint("10-global/a.md", text).is_new);
}
