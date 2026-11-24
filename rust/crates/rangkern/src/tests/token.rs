use super::{data, strings};
use crate::token::{Analyzer, NGRAM_MARK, tokenize};

#[test]
fn the_token_functions_agree_with_python_on_every_text() {
    let rows = data("tokens");
    let rows = rows.as_array().expect("rows");
    assert!(rows.len() > 140);
    let mut analyzer = Analyzer::new();
    for row in rows {
        let text = row["text"].as_str().expect("text");
        assert_eq!(tokenize(text), strings(&row["words"]), "words of {text:?}");
        assert_eq!(
            analyzer.stem_tokens(text),
            strings(&row["stems"]),
            "stems of {text:?}"
        );
        assert_eq!(
            analyzer.index_tokens(text),
            strings(&row["index"]),
            "index of {text:?}"
        );
        let counts: Vec<(String, u32)> = row["counts"]
            .as_array()
            .expect("counts")
            .iter()
            .map(|pair| {
                (
                    pair[0].as_str().expect("term").to_string(),
                    u32::try_from(pair[1].as_u64().expect("count")).expect("fits"),
                )
            })
            .collect();
        assert_eq!(analyzer.index_counts(text), counts, "counts of {text:?}");
    }
}

#[test]
fn a_fresh_analyzer_gives_the_same_stems_as_a_warm_one() {
    let mut warm = Analyzer::new();
    let first = warm.stem_word("einbettungen");
    assert_eq!(first, "einbett");
    assert_eq!(warm.stem_word("einbettungen"), first);
    assert_eq!(Analyzer::new().stem_word("einbettungen"), first);
    assert_eq!(warm.stem_word("v2"), "v2");
    assert_eq!(warm.stem_word("a'b"), "a'b");
}

#[test]
fn grams_start_at_six_code_points() {
    let mut analyzer = Analyzer::new();
    assert_eq!(analyzer.index_forms("hallo"), ["hallo"]);
    let forms = analyzer.index_forms("suchen");
    assert_eq!(forms, ["such"]);
    let long = analyzer.index_forms("suchindex");
    assert_eq!(long[0], "suchindex");
    assert_eq!(long.len(), 1 + 6);
    assert!(
        long[1..]
            .iter()
            .all(|gram| gram.starts_with(NGRAM_MARK) && gram.chars().count() == 5)
    );
    assert_eq!(long[1], "#such");
    assert_eq!(long[6], "#ndex");
    assert_eq!(
        analyzer.index_forms("12345678"),
        ["12345678", "#1234", "#2345", "#3456", "#4567", "#5678"]
    );
}

#[test]
fn counts_keep_the_order_of_first_occurrence_and_add_repeated_grams() {
    let mut analyzer = Analyzer::new();
    let counts = analyzer.index_counts("b a b");
    assert_eq!(counts, [("b".to_string(), 2), ("a".to_string(), 1)]);
    let repeated = analyzer.index_counts("aaaaaaa");
    assert_eq!(
        repeated,
        [("aaaaaaa".to_string(), 1), ("#aaaa".to_string(), 4)]
    );
}
