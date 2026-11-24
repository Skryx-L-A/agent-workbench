//! TR12: which program a start runs, found the way `portable-pty` finds it, so that a program that is missing is an error of
//! `Kind::starten` even though the child starts through `env -u SHELL` (see `kind.rs`).

use std::ffi::OsStr;
use std::path::{Component, Path, PathBuf};

/// `true` if the path is a file with an execute bit.
fn ausfuehrbar(pfad: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(pfad).is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
}

/// The absolute path of the program: an absolute name as it is; `./x` and `../x` from the working folder; any other name
/// by `PATH` of the child (taken from `env`), each folder of it from the working folder on.
///
/// # Errors
/// The program is not there, is not a file or has no execute bit; the text says which name.
pub(crate) fn finden(
    programm: &str,
    env: &[(String, String)],
    cwd: &Path,
) -> Result<PathBuf, String> {
    let name = Path::new(programm);
    let kandidaten: Vec<PathBuf> = if name.is_absolute() {
        vec![name.to_path_buf()]
    } else if matches!(
        name.components().next(),
        Some(Component::CurDir | Component::ParentDir)
    ) {
        vec![cwd.join(name)]
    } else {
        let Some((_, pfad)) = env.iter().find(|(n, _)| n == "PATH") else {
            return Err(format!(
                "Unable to spawn {programm} because the PATH is not set"
            ));
        };
        std::env::split_paths(OsStr::new(pfad))
            .map(|ordner| cwd.join(ordner).join(name))
            .collect()
    };
    for kandidat in kandidaten {
        if ausfuehrbar(&kandidat) {
            return std::path::absolute(&kandidat).map_err(|e| format!("{programm}: {e}"));
        }
    }
    Err(format!(
        "Unable to spawn {programm}: no executable file found"
    ))
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::finden;

    fn env(pfad: &str) -> Vec<(String, String)> {
        vec![("PATH".to_owned(), pfad.to_owned())]
    }

    #[test]
    fn an_absolute_name_is_taken_as_it_is_if_it_runs() {
        assert_eq!(
            finden("/bin/sh", &[], Path::new("/")),
            Ok(PathBuf::from("/bin/sh"))
        );
        assert!(finden("/nonexistent-program-of-traeger", &[], Path::new("/")).is_err());
        assert!(finden("/usr", &[], Path::new("/")).is_err(), "a folder");
        assert!(
            finden("/etc/passwd", &[], Path::new("/")).is_err(),
            "no execute bit"
        );
    }

    #[test]
    fn a_plain_name_is_searched_in_the_path_of_the_child() {
        let gefunden = finden("sh", &env("/nonexistent:/usr/bin:/bin"), Path::new("/"))
            .expect("sh is in /usr/bin or /bin");
        assert!(gefunden == Path::new("/usr/bin/sh") || gefunden == Path::new("/bin/sh"));
        assert!(finden("sh", &env("/nonexistent"), Path::new("/")).is_err());
        assert!(
            finden("sh", &[], Path::new("/")).is_err(),
            "without PATH nothing is found"
        );
    }

    #[test]
    fn a_name_with_a_dot_is_taken_from_the_working_folder_only() {
        assert_eq!(
            finden("./sh", &env("/nonexistent"), Path::new("/bin")),
            Ok(PathBuf::from("/bin/sh"))
        );
        assert_eq!(
            finden("../../bin/sh", &env("/nonexistent"), Path::new("/usr/lib")),
            Ok(PathBuf::from("/usr/lib/../../bin/sh"))
        );
        assert!(finden("./sh", &env("/bin"), Path::new("/usr/lib")).is_err());
    }

    #[test]
    fn the_folders_of_the_path_count_from_the_working_folder_on() {
        assert_eq!(
            finden("sh", &env("bin"), Path::new("/")),
            Ok(PathBuf::from("/bin/sh"))
        );
    }
}
