//! The deny lists of the enclosure: secrets, login stores of the account and system paths. They are the lists of the
//! old Linux door of the earlier Python version (`TUER_*`, `ANMELDE_*`, `_SECRET_PARTS`, `_SYSTEM_DENY`), taken over as a template
//! and not called. No path of these lists is ever mounted, whoever supplied it.

use std::path::{Path, PathBuf};

use crate::pfad::innerhalb;

/// A path with one of these segments is never mounted (secrets and settings of tools).
pub const GEHEIMNIS_TEILE: [&str; 9] = [
    ".aws",
    ".config",
    ".docker",
    ".gnupg",
    ".kube",
    ".secrets-sync",
    ".ssh",
    "90-secrets",
    ".env",
];

/// The working folder is none of these paths, and none of them lies below it.
pub const SYSTEM_VERBOTEN: [&str; 11] = [
    "/",
    "/dev",
    "/etc",
    "/home",
    "/proc",
    "/root",
    "/run",
    "/sys",
    "/tmp",
    "/var/run",
    "/var/lib/docker",
];

/// The working folder does not lie below these paths.
pub const SYSTEM_UNTER_VERBOTEN: [&str; 7] =
    ["/usr", "/run", "/proc", "/sys", "/dev", "/etc", "/opt"];

/// Credential stores of the account, relative to the home: browser profiles, key rings, `ssh`, `gpg`, cloud tools,
/// password managers, and the store of the holder (`.local/state/agent-workbench/halter`, which keeps the login of
/// the Copilot CLI). No harness is exempt from the last one: not even its own harness may see it.
pub const ANMELDE_ORDNER: [&str; 39] = [
    ".config/google-chrome",
    ".config/chromium",
    ".mozilla",
    ".config/microsoft-edge",
    ".config/BraveSoftware",
    ".config/vivaldi",
    ".config/opera",
    "snap/firefox",
    "snap/chromium",
    "snap/brave",
    "snap/opera",
    "snap/vivaldi",
    ".var/app/org.mozilla.firefox",
    ".var/app/com.google.Chrome",
    ".var/app/org.chromium.Chromium",
    ".var/app/com.microsoft.Edge",
    ".var/app/com.brave.Browser",
    ".var/app/com.opera.Opera",
    ".var/app/com.vivaldi.Vivaldi",
    ".var/app/io.gitlab.librewolf-community",
    ".local/share/keyrings",
    ".local/share/kwalletd",
    ".gnome2/keyrings",
    ".ssh",
    ".gnupg",
    ".aws",
    ".config/gcloud",
    ".azure",
    ".kube",
    ".config/rclone",
    ".config/1Password",
    ".config/Bitwarden",
    ".config/KeePassXC",
    ".config/keepassxc",
    ".password-store",
    ".config/op",
    ".var/app/com.bitwarden.desktop",
    ".var/app/org.keepassxc.KeePassXC",
    ".local/state/agent-workbench/halter",
];

/// File stores of tokens and passwords of tools, relative to the home.
pub const ANMELDE_DATEIEN: [&str; 13] = [
    ".config/gh/hosts.yml",
    ".netrc",
    ".git-credentials",
    ".config/git/credentials",
    ".npmrc",
    ".yarnrc.yml",
    ".pypirc",
    ".docker/config.json",
    ".cargo/credentials",
    ".cargo/credentials.toml",
    ".gem/credentials",
    ".m2/settings.xml",
    ".terraform.d/credentials.tfrc.json",
];

/// The login files of each harness, relative to the home. The own harness reads its own, every other is closed.
pub const HARNESS_ZUGANG: [(&str, &[&str]); 6] = [
    ("claude", &[".claude/.credentials.json", ".claude.json"]),
    ("codex", &[".codex/auth.json"]),
    ("pi", &[".pi/agent/auth.json"]),
    ("opencode", &[".local/share/opencode/auth.json"]),
    ("qwen", &[".qwen/oauth_creds.json"]),
    ("gemini", &[".gemini/oauth_creds.json"]),
];

/// Channels of the secret service below `/run/user/<uid>`, closed by the private `/run`.
pub const ANMELDE_LAUFZEIT: [&str; 3] = ["bus", "keyring", "gnupg"];

/// Folders that exist besides a login store: other places a path must never cover.
fn stellen(heim: &Path, harness: &str) -> Vec<PathBuf> {
    let mut alle: Vec<PathBuf> = ANMELDE_ORDNER
        .iter()
        .chain(ANMELDE_DATEIEN.iter())
        .map(|rel| heim.join(rel))
        .collect();
    for (name, dateien) in HARNESS_ZUGANG {
        if name != harness {
            alle.extend(dateien.iter().map(|rel| heim.join(rel)));
        }
    }
    alle
}

/// `true` when a segment of `pfad` is a secret name.
#[must_use]
pub fn hat_geheimnisteil(pfad: &Path) -> bool {
    pfad.components()
        .any(|c| GEHEIMNIS_TEILE.iter().any(|g| c.as_os_str() == *g))
}

/// `true` when `pfad` is, lies in or covers a login store of the account at `heim`, or a login file of a harness
/// other than `harness` (an unknown harness owns none: all are foreign).
#[must_use]
pub fn beruehrt_anmeldespeicher(pfad: &Path, heim: &Path, harness: &str) -> bool {
    stellen(heim, harness)
        .iter()
        .any(|stelle| innerhalb(pfad, stelle) || innerhalb(stelle, pfad))
}

/// `true` when `arbeit` may not be a working folder: a system path, one above the home, or the home.
#[must_use]
pub fn arbeitsordner_verboten(arbeit: &Path, heim: &Path) -> bool {
    innerhalb(heim, arbeit)
        || SYSTEM_VERBOTEN
            .iter()
            .any(|p| innerhalb(Path::new(p), arbeit))
        || SYSTEM_UNTER_VERBOTEN
            .iter()
            .any(|p| innerhalb(arbeit, Path::new(p)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(s: &str) -> &Path {
        Path::new(s)
    }

    #[test]
    fn secret_segments_are_found_anywhere_in_the_path() {
        for pfad in [
            "/a/.ssh/x",
            "/a/b/.config",
            "/90-secrets",
            "/x/.env",
            "/x/.aws/y",
            "/.gnupg",
            "/.docker",
            "/.kube/c",
            "/.secrets-sync",
        ] {
            assert!(hat_geheimnisteil(p(pfad)), "{pfad}");
        }
        for pfad in ["/a/b", "/a/.ssh2", "/a/config", "/a/x.env", "/a/.envx"] {
            assert!(!hat_geheimnisteil(p(pfad)), "{pfad}");
        }
    }

    #[test]
    fn a_login_store_is_touched_from_above_below_and_equal() {
        let heim = p("/home/u");
        for pfad in [
            "/home/u/.ssh",
            "/home/u/.ssh/id",
            "/home/u",
            "/home",
            "/home/u/.mozilla/x",
            "/home/u/.config/gh/hosts.yml",
            "/home/u/.config/gh",
            "/home/u/.cargo/credentials.toml",
            "/home/u/.local/share/keyrings",
            "/home/u/.local/share",
            "/home/u/.local",
        ] {
            assert!(beruehrt_anmeldespeicher(p(pfad), heim, "claude"), "{pfad}");
        }
        for pfad in [
            "/home/u/proj",
            "/home/u/.local/bin",
            "/home/u/.local/share/claude",
            "/srv/x",
            "/home/v/.ssh",
        ] {
            assert!(!beruehrt_anmeldespeicher(p(pfad), heim, "claude"), "{pfad}");
        }
    }

    #[test]
    fn the_store_of_the_holder_is_closed_like_a_login_store_for_every_harness() {
        let heim = p("/home/u");
        let harnesse: Vec<&str> = HARNESS_ZUGANG
            .iter()
            .map(|(name, _)| *name)
            .chain(["attrappe"])
            .collect();
        for harness in harnesse {
            for pfad in [
                "/home/u/.local/state/agent-workbench/halter/geheim/anmeldung.copilot",
                "/home/u/.local/state/agent-workbench/halter",
                "/home/u/.local/state/agent-workbench",
                "/home/u/.local/state",
            ] {
                assert!(
                    beruehrt_anmeldespeicher(p(pfad), heim, harness),
                    "{harness} {pfad}"
                );
            }
            for pfad in [
                "/home/u/.local/state/andere",
                "/home/u/.local/state/agent-workbench/andere",
            ] {
                assert!(
                    !beruehrt_anmeldespeicher(p(pfad), heim, harness),
                    "{harness} {pfad}"
                );
            }
        }
    }

    #[test]
    fn the_own_harness_keeps_its_login_files_and_the_others_do_not() {
        let heim = p("/home/u");
        assert!(!beruehrt_anmeldespeicher(
            p("/home/u/.claude/.credentials.json"),
            heim,
            "claude"
        ));
        assert!(beruehrt_anmeldespeicher(
            p("/home/u/.claude/.credentials.json"),
            heim,
            "pi"
        ));
        assert!(beruehrt_anmeldespeicher(
            p("/home/u/.claude.json"),
            heim,
            "pi"
        ));
        assert!(beruehrt_anmeldespeicher(
            p("/home/u/.pi/agent/auth.json"),
            heim,
            "claude"
        ));
        assert!(!beruehrt_anmeldespeicher(
            p("/home/u/.pi/agent/auth.json"),
            heim,
            "pi"
        ));
        // an unknown harness owns none: every login file is foreign
        for datei in [
            ".claude/.credentials.json",
            ".pi/agent/auth.json",
            ".codex/auth.json",
            ".qwen/oauth_creds.json",
        ] {
            let pfad = heim.join(datei);
            assert!(beruehrt_anmeldespeicher(&pfad, heim, "attrappe"), "{datei}");
        }
        for (name, dateien) in HARNESS_ZUGANG {
            for datei in dateien {
                assert!(
                    !beruehrt_anmeldespeicher(&heim.join(datei), heim, name),
                    "{name} {datei}"
                );
            }
        }
    }

    #[test]
    fn a_working_folder_is_not_a_system_path_nor_the_home_nor_above_it() {
        let heim = p("/home/u");
        for arbeit in [
            "/home/u",
            "/home",
            "/usr",
            "/usr/share",
            "/etc",
            "/etc/x",
            "/run/x",
            "/proc",
            "/sys/x",
            "/dev/shm",
            "/opt",
            "/opt/x",
            "/tmp",
            "/root",
            "/var/run",
            "/var/lib/docker",
        ] {
            assert!(arbeitsordner_verboten(p(arbeit), heim), "{arbeit}");
        }
        for arbeit in [
            "/home/u/proj",
            "/srv/arbeit",
            "/var/tmp/x",
            "/tmp/x",
            "/home/v/proj",
        ] {
            assert!(!arbeitsordner_verboten(p(arbeit), heim), "{arbeit}");
        }
    }

    #[test]
    fn the_lists_have_the_sizes_the_arrays_say() {
        assert_eq!(ANMELDE_LAUFZEIT.len(), 3);
        assert!(
            ANMELDE_ORDNER
                .iter()
                .all(|s| !s.starts_with('/') && !s.ends_with('/'))
        );
        assert!(
            ANMELDE_DATEIEN
                .iter()
                .all(|s| !s.starts_with('/') && !s.ends_with('/'))
        );
    }
}
