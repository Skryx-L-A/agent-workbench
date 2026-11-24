//! Path rules shared by the checks. A path of the description is absolute, normalised and plain text.

use std::path::{Path, PathBuf};

/// `true` when `pfad` is absolute, is not the root, has no `.` or `..` segment, no empty segment (`//`), no trailing
/// slash, no control character (U+0000 to U+001F, U+007F) and is valid UTF-8. These are the rules of the contract for
/// `arbeitsordner`, applied to every path of the description.
#[must_use]
pub fn normal(pfad: &Path) -> bool {
    let Some(text) = pfad.to_str() else {
        return false;
    };
    // an empty segment is `//` or a trailing slash; the root alone has no segment and is refused
    let Some(rest) = text.strip_prefix('/') else {
        return false;
    };
    !text.chars().any(char::is_control)
        && rest
            .split('/')
            .all(|s| !s.is_empty() && s != "." && s != "..")
}

/// `true` when `pfad` is `wurzel` or lies below it (by whole segments, not by text prefix).
#[must_use]
pub fn innerhalb(pfad: &Path, wurzel: &Path) -> bool {
    pfad.starts_with(wurzel)
}

/// `true` when one path is the other or lies below it.
#[must_use]
pub fn ueberlappt(a: &Path, b: &Path) -> bool {
    innerhalb(a, b) || innerhalb(b, a)
}

/// The place `rel` below `basis` inside the Linux enclosure. The enclosure is Linux, so the place is a POSIX path on
/// every host: the parts are joined with `/`, also on Windows, where `Path::join` would put `\\` between them. On Unix
/// the result is the same as `basis.join(rel)`; an absolute `rel` replaces `basis` as there.
#[must_use]
pub fn gast_pfad(basis: &Path, rel: &str) -> PathBuf {
    if rel.starts_with('/') {
        return PathBuf::from(rel);
    }
    let basis = basis.to_string_lossy();
    PathBuf::from(format!("{}/{rel}", basis.trim_end_matches('/')))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(s: &str) -> &Path {
        Path::new(s)
    }

    #[test]
    fn a_place_inside_is_joined_with_a_slash_on_every_host() {
        assert_eq!(
            gast_pfad(p("/srv/beispiel/heim"), ".cache/copilot"),
            Path::new("/srv/beispiel/heim/.cache/copilot")
        );
        assert_eq!(
            gast_pfad(p("/srv/beispiel/heim"), ".pi").to_str(),
            Some("/srv/beispiel/heim/.pi")
        );
        assert_eq!(gast_pfad(p("/"), "a").to_str(), Some("/a"));
        assert_eq!(
            gast_pfad(p("/srv/beispiel/heim"), "/x").to_str(),
            Some("/x")
        );
        #[cfg(unix)]
        for rel in [".cache/copilot", "a", "/x"] {
            assert_eq!(
                gast_pfad(p("/srv/beispiel/heim"), rel),
                p("/srv/beispiel/heim").join(rel)
            );
        }
    }

    #[test]
    fn normal_takes_plain_absolute_paths() {
        assert!(normal(p("/a")));
        assert!(normal(p("/home/beispiel/projekt")));
        assert!(normal(p("/a/.versteckt")));
        assert!(normal(p("/a/...x")));
    }

    #[test]
    fn normal_refuses_every_other_shape() {
        for schlecht in [
            "",
            "/",
            "a",
            "a/b",
            "/a/",
            "//a",
            "/a//b",
            "/a/./b",
            "/a/../b",
            "/..",
            "/.",
            "/a/.",
            "/a\nb",
            "/a\u{7f}b",
            "/a\u{0}b",
            "/a\tb",
        ] {
            assert!(!normal(p(schlecht)), "{schlecht:?}");
        }
    }

    #[cfg(unix)]
    #[test]
    fn normal_refuses_text_that_is_not_utf8() {
        use std::ffi::OsStr;
        use std::os::unix::ffi::OsStrExt;
        assert!(!normal(Path::new(OsStr::from_bytes(b"/a/\xff"))));
    }

    #[test]
    fn innerhalb_counts_whole_segments() {
        assert!(innerhalb(p("/a/b"), p("/a")));
        assert!(innerhalb(p("/a"), p("/a")));
        assert!(!innerhalb(p("/ab"), p("/a")));
        assert!(!innerhalb(p("/a"), p("/a/b")));
    }

    #[test]
    fn ueberlappt_goes_both_ways() {
        assert!(ueberlappt(p("/a/b"), p("/a")));
        assert!(ueberlappt(p("/a"), p("/a/b")));
        assert!(ueberlappt(p("/a"), p("/a")));
        assert!(!ueberlappt(p("/ab"), p("/a")));
        assert!(!ueberlappt(p("/a/x"), p("/a/y")));
    }
}
