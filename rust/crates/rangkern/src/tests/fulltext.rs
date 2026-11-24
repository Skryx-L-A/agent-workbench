use super::TempDir;
use crate::fulltext::{fulltext_rels, lines, searchable};
use crate::notes::note_rec;

fn found(files: &[(&str, &[u8])], query: &str) -> Vec<String> {
    let dir = TempDir::new();
    let notes: Vec<_> = files
        .iter()
        .map(|(rel, bytes)| {
            dir.write(rel, bytes);
            note_rec(rel, String::new())
        })
        .collect();
    fulltext_rels(dir.path(), &notes, query)
}

#[test]
fn a_question_is_a_case_insensitive_regular_expression() {
    let files: [(&str, &[u8]); 3] = [
        ("a.md", b"Hallo Welt\n"),
        ("b.md", b"nichts\n"),
        ("c.md", b"HALLO\n"),
    ];
    assert_eq!(found(&files, "hallo"), ["a.md", "c.md"]);
    assert_eq!(found(&files, "^hal+o$"), ["c.md"]);
    assert_eq!(found(&files, "w.lt"), ["a.md"]);
    assert_eq!(found(&files, "ä"), Vec::<String>::new());
    assert_eq!(found(&[("u.md", "Ärger\n".as_bytes())], "ÄRGER"), ["u.md"]);
    assert_eq!(found(&[("u.md", "Ärger\n".as_bytes())], "ärger"), ["u.md"]);
    assert!(found(&[("s.md", "Straße\n".as_bytes())], "STRASSE").is_empty());
}

#[test]
fn an_invalid_expression_finds_nothing() {
    let files: [(&str, &[u8]); 1] = [("a.md", b"a[b\n")];
    assert!(found(&files, "[").is_empty());
    assert!(found(&files, "(").is_empty());
    assert!(found(&files, "a\\nb").is_empty());
}

#[test]
fn lines_end_at_a_newline_only() {
    assert_eq!(found(&[("a.md", b"foo\r\n")], "foo$"), Vec::<String>::new());
    assert_eq!(found(&[("a.md", b"foo\r\n")], "foo\\r$"), ["a.md"]);
    assert!(found(&[("a.md", b"ab\ncd\n")], "ab\\scd").is_empty());
    assert!(found(&[("a.md", b"ab\ncd\n")], "(?s)ab.cd").is_empty());
    assert!(found(&[("a.md", b"ab\ncd\n")], "ab[^x]cd").is_empty());
    assert_eq!(found(&[("a.md", b"ab\ncd\n")], "^cd$"), ["a.md"]);
    assert_eq!(found(&[("a.md", b"x\n\ny")], "^$"), ["a.md"]);
}

#[test]
fn the_empty_question_lists_every_file_with_a_line() {
    let files: [(&str, &[u8]); 4] = [
        ("a.md", b"x"),
        ("b.md", b""),
        ("c.md", b"\n"),
        ("d.md", b"y\n"),
    ];
    assert_eq!(found(&files, ""), ["a.md", "c.md", "d.md"]);
}

#[test]
fn binary_files_are_not_listed_but_bad_utf8_and_hidden_names_are() {
    let files: [(&str, &[u8]); 6] = [
        ("nul-vor.md", b"x\0y\nhallo\n"),
        ("nul-nach.md", b"hallo\nx\0y\n"),
        ("schlecht.md", b"ab\xffcd hallo\n"),
        (".versteckt.md", b"hallo\n"),
        ("ok.md", b"hallo\n"),
        ("d/.auch.md", b"hallo\n"),
    ];
    assert_eq!(
        found(&files, "hallo"),
        [".versteckt.md", "d/.auch.md", "ok.md", "schlecht.md"]
    );
}

#[test]
fn byte_order_marks_are_handled_before_the_search() {
    let le_file: Vec<u8> = [
        &[0xff_u8, 0xfe][..],
        &"hallo\n"
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>(),
    ]
    .concat();
    let be_file: Vec<u8> = [
        &[0xfe_u8, 0xff][..],
        &"hallo\n"
            .encode_utf16()
            .flat_map(u16::to_be_bytes)
            .collect::<Vec<_>>(),
    ]
    .concat();
    let files: [(&str, &[u8]); 3] = [
        ("le.md", &le_file),
        ("be.md", &be_file),
        ("bom.md", b"\xef\xbb\xbfhallo\n"),
    ];
    assert_eq!(found(&files, "hallo"), ["be.md", "bom.md", "le.md"]);
    assert_eq!(found(&files, "^hallo$"), ["be.md", "bom.md", "le.md"]);
    assert_eq!(searchable(b"\xef\xbb\xbfa".to_vec()), b"a");
    assert_eq!(
        searchable(vec![0xff, 0xfe, b'a', 0, 0x00, 0xd8]),
        "a\u{fffd}".as_bytes()
    );
    assert_eq!(searchable(vec![0xff, 0xfe, b'a']), "a".as_bytes());
    assert_eq!(searchable(b"abc".to_vec()), b"abc");
}

#[test]
fn the_files_are_sorted_by_the_bytes_of_the_path() {
    let files: [(&str, &[u8]); 4] = [
        ("a/b.md", b"x\n"),
        ("a.md", b"x\n"),
        ("B.md", b"x\n"),
        ("ä.md", b"x\n"),
    ];
    assert_eq!(found(&files, "x"), ["B.md", "a.md", "a/b.md", "ä.md"]);
}

#[test]
fn a_file_that_cannot_be_read_is_skipped() {
    let dir = TempDir::new();
    let notes = vec![note_rec("fehlt.md", String::new())];
    assert!(fulltext_rels(dir.path(), &notes, "x").is_empty());
}

#[test]
fn the_last_empty_line_is_no_line() {
    assert_eq!(lines(b"a\n").count(), 1);
    assert_eq!(lines(b"a\n\n").count(), 2);
    assert_eq!(lines(b"a").count(), 1);
    assert_eq!(lines(b"").count(), 0);
    assert_eq!(lines(b"\n").count(), 1);
}
