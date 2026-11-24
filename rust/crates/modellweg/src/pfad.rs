//! The field `pfad` of the line (contract 8, section 8): at most the first two segments of the request path, without
//! query and fragment. A segment that is too long or carries a character outside `[A-Za-z0-9._~-]` (a percent encoding
//! too) stands as `*`; nothing is ever shortened and nothing is ever decoded, so no part of a name can slip in.
//! With the admin switch `--protokoll-pfad-voll` the whole path stands there, under the same character rule and a
//! length limit.

/// The value of a segment, or of the whole field, that does not keep the form.
pub const STERN: &str = "*";
/// Longest segment of the limited form; a longer one is [`STERN`].
pub const SEGMENT_MAX: usize = 32;
/// Longest path of the full form; a longer one makes the whole field [`STERN`].
pub const PFAD_VOLL_MAX: usize = 512;

/// A segment `[A-Za-z0-9._~-]{1,max}`.
fn segment_ok(teil: &str, max: usize) -> bool {
    !teil.is_empty()
        && teil.len() <= max
        && teil
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"._~-".contains(&c))
}

/// The field `pfad` for the request target of one request; `voll` only with `--protokoll-pfad-voll`.
///
/// Query and fragment fall away. A target that does not begin with `/` (the form `*` or an absolute URI) gives
/// [`STERN`], the root `/` gives `/`. Without `voll` at most the first two segments stand there, each at most
/// [`SEGMENT_MAX`] bytes; with `voll` every segment stands there, and a path over [`PFAD_VOLL_MAX`] bytes gives
/// [`STERN`]. A segment outside the form is [`STERN`], never a part of itself.
#[must_use]
pub fn pfad_feld(ziel: &str, voll: bool) -> String {
    let ohne = ziel.split(['?', '#']).next().unwrap_or_default();
    let Some(rest) = ohne.strip_prefix('/') else {
        return STERN.to_owned();
    };
    if rest.is_empty() {
        return "/".to_owned();
    }
    if voll && ohne.len() > PFAD_VOLL_MAX {
        return STERN.to_owned();
    }
    let max = if voll { PFAD_VOLL_MAX } else { SEGMENT_MAX };
    let segmente = rest
        .split('/')
        .map(|teil| if segment_ok(teil, max) { teil } else { STERN });
    let genommen: Vec<&str> = if voll {
        segmente.collect()
    } else {
        segmente.take(2).collect()
    };
    format!("/{}", genommen.join("/"))
}

/// Whether a value may stand in the field `pfad` of a line: [`STERN`], `/`, or `/` and segments that are each
/// [`STERN`] or of the form `[A-Za-z0-9._~-]+`, in all at most [`PFAD_VOLL_MAX`] bytes. So no query, no fragment and
/// no percent encoding can stand in the line, in either form.
#[must_use]
pub fn pfad_feld_ok(wert: &str) -> bool {
    if wert == STERN || wert == "/" {
        return true;
    }
    wert.len() <= PFAD_VOLL_MAX
        && wert.strip_prefix('/').is_some_and(|rest| {
            rest.split('/')
                .all(|teil| teil == STERN || segment_ok(teil, PFAD_VOLL_MAX))
        })
}

#[cfg(test)]
mod tests {
    use super::{PFAD_VOLL_MAX, pfad_feld, pfad_feld_ok};

    #[test]
    fn the_limited_field_keeps_at_most_two_segments() {
        assert_eq!(pfad_feld("/models", false), "/models");
        assert_eq!(pfad_feld("/models/session", false), "/models/session");
        assert_eq!(pfad_feld("/models/session/a/b", false), "/models/session");
        assert_eq!(pfad_feld("/", false), "/");
    }

    #[test]
    fn query_and_fragment_fall_away() {
        assert_eq!(pfad_feld("/models?per_page=100", false), "/models");
        assert_eq!(pfad_feld("/models#teil", false), "/models");
        assert_eq!(pfad_feld("/models?a=b#c", false), "/models");
        assert_eq!(pfad_feld("/?a=b", false), "/");
        assert_eq!(pfad_feld("/a/b?x=/c/d", false), "/a/b");
    }

    #[test]
    fn a_segment_outside_the_form_is_a_star_and_is_never_shortened() {
        let s32 = "a".repeat(32);
        let s33 = "a".repeat(33);
        assert_eq!(pfad_feld(&format!("/{s32}"), false), format!("/{s32}"));
        assert_eq!(pfad_feld(&format!("/{s33}"), false), "/*");
        assert_eq!(pfad_feld("/%2e%2e", false), "/*");
        assert_eq!(pfad_feld("/a b", false), "/*");
        assert_eq!(pfad_feld("/a:b/c", false), "/*/c");
        assert_eq!(pfad_feld("/a/b c", false), "/a/*");
        assert_eq!(pfad_feld("/a//b", false), "/a/*");
        assert_eq!(pfad_feld("/a/", false), "/a/*");
    }

    #[test]
    fn a_target_that_is_no_path_is_a_star() {
        assert_eq!(pfad_feld("*", false), "*");
        assert_eq!(pfad_feld("https://host.beispiel.invalid/a", false), "*");
        assert_eq!(pfad_feld("", false), "*");
        assert_eq!(pfad_feld("a/b", false), "*");
    }

    #[test]
    fn the_full_form_keeps_every_segment_up_to_the_limit() {
        assert_eq!(pfad_feld("/a/b/c/d", true), "/a/b/c/d");
        assert_eq!(pfad_feld("/a/b c/d", true), "/a/*/d");
        assert_eq!(pfad_feld("/a/b?x=y", true), "/a/b");
        let vierzig = format!("/{}", "a".repeat(40));
        assert_eq!(pfad_feld(&vierzig, true), vierzig);
        let lang = format!("/{}", "a".repeat(PFAD_VOLL_MAX - 1));
        assert_eq!(lang.len(), PFAD_VOLL_MAX);
        assert_eq!(pfad_feld(&lang, true), lang);
        assert_eq!(pfad_feld(&format!("{lang}a"), true), "*");
    }

    #[test]
    fn the_field_of_the_model_is_always_a_value_of_the_line() {
        for ziel in [
            "/",
            "/a",
            "/a/b",
            "/a/b/c",
            "/a b",
            "*",
            "/%2e",
            "/a/",
            "/a//b",
            "https://h.b.invalid/a",
            &format!("/{}", "a".repeat(600)),
        ] {
            for voll in [false, true] {
                let wert = pfad_feld(ziel, voll);
                assert!(pfad_feld_ok(&wert), "{ziel} {voll} -> {wert}");
            }
        }
    }

    #[test]
    fn a_value_of_the_line_carries_no_query_and_no_encoding() {
        assert!(pfad_feld_ok("*"));
        assert!(pfad_feld_ok("/"));
        assert!(pfad_feld_ok("/a/*"));
        assert!(pfad_feld_ok(&format!("/{}", "a".repeat(PFAD_VOLL_MAX - 1))));
        assert!(!pfad_feld_ok(&format!("/{}", "a".repeat(PFAD_VOLL_MAX))));
        assert!(!pfad_feld_ok(""));
        assert!(!pfad_feld_ok("a"));
        assert!(!pfad_feld_ok("/a?b=c"));
        assert!(!pfad_feld_ok("/a#b"));
        assert!(!pfad_feld_ok("/%2e"));
        assert!(!pfad_feld_ok("/a b"));
        assert!(!pfad_feld_ok("/a/"));
        assert!(!pfad_feld_ok("/a//b"));
        assert!(!pfad_feld_ok("**"));
    }
}
