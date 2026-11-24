//! Under Windows `rangkern sitzung` has no socket yet (the named pipe follows with its own part of contract 4): it ends
//! with exit 64 and exactly one line on the error output, and writes nothing to the standard output. These tests run only
//! for a Windows target; the cross build compiles them.

use crate::cli::run;

#[test]
fn the_command_ends_with_exit_64_and_one_line() {
    let mut out = Vec::new();
    let mut err = Vec::new();
    let args: Vec<String> = ["sitzung", "vault", "--socket", "s", "--route", "r"]
        .iter()
        .map(ToString::to_string)
        .collect();
    assert_eq!(run(&args, &mut out, &mut err), 64);
    assert!(out.is_empty());
    assert_eq!(
        String::from_utf8(err).expect("text"),
        "rangkern sitzung: unter Windows noch nicht unterstützt\n"
    );
}

#[test]
fn the_command_without_arguments_ends_the_same_way() {
    let mut out = Vec::new();
    let mut err = Vec::new();
    assert_eq!(run(&["sitzung".to_string()], &mut out, &mut err), 64);
    assert!(out.is_empty());
    let text = String::from_utf8(err).expect("text");
    assert!(text.ends_with('\n'));
    assert_eq!(text.lines().count(), 1);
}
