use crate::stem::stem;

#[test]
fn a_sample_of_the_oracle_gives_the_same_stems() {
    let text = include_str!("../../tests/data/stem_orakel.tsv");
    let mut checked = 0;
    for line in text.lines() {
        let (word, expected) = line.split_once('\t').expect("a line has a tab");
        assert_eq!(stem(word), expected, "stem of {word:?}");
        checked += 1;
    }
    assert!(checked > 3500);
}

#[test]
fn single_words_with_their_rules() {
    assert_eq!(stem(""), "");
    assert_eq!(stem("ß"), "ss");
    assert_eq!(stem("aeußerst"), "ausserst");
    assert_eq!(stem("einbettungen"), "einbett");
    assert_eq!(stem("suchindex"), "suchindex");
    assert_eq!(stem("kenntnisse"), "kenntnis");
    assert_eq!(stem("straße"), "strass");
    assert_eq!(stem("quelle"), "quell");
    assert_eq!(stem("ab's"), "ab");
    assert_eq!(stem("l'sch"), "l'sch");
}
