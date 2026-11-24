//! The rule for the program folders of `lauf.json`: the one rule of the enclosure, called from the package scripts and from the check.
//!
//! The enclosure refuses a program folder when a segment of it is a secret name (`hat_geheimnisteil`) or when it is, lies in or covers a
//! login store of the account (`beruehrt_anmeldespeicher`); see `einschluss::profil` (`kopf_pruefen`). This module calls exactly these two
//! functions of the crate `einschluss`; there is no second version of the rule, neither in a shell script nor in the contract text. The
//! harness name is `copilot`, which owns no login file in the list `HARNESS_ZUGANG`: a folder passes only when it passes for every session.

use std::path::Path;

use einschluss::verbote::{beruehrt_anmeldespeicher, hat_geheimnisteil};

/// The name of the harness for the rule: one that owns no login file, so every login file of every harness counts as foreign.
const HARNESS_FREMD: &str = "copilot";

/// The sentence of `--programmordner` for a folder that the enclosure refuses.
pub const SATZ_VERBOTEN: &str = "pruef.programmordner_verboten: Der Ordner beruehrt den Anmeldespeicher oder ein Geheimnis; der Einschluss lehnt ihn ab (5 verboten).";

/// `true` when the enclosure refuses `ordner` as a program folder for the user with the home `heim`.
#[must_use]
pub fn verboten(ordner: &Path, heim: &Path) -> bool {
    hat_geheimnisteil(ordner) || beruehrt_anmeldespeicher(ordner, heim, HARNESS_FREMD)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(ordner: &str) -> bool {
        verboten(Path::new(ordner), Path::new("/srv/beispiel"))
    }

    #[test]
    fn the_prefix_of_a_user_install_is_refused_and_its_package_root_is_not() {
        assert!(v("/srv/beispiel/.local"));
        assert!(v("/srv/beispiel"));
        assert!(!v("/srv/beispiel/.local/bin"));
        assert!(!v("/srv/beispiel/.local/lib/node_modules/@scope/paket"));
        assert!(!v("/srv/beispiel/.npm-global/lib/node_modules/paket"));
        assert!(!v("/srv/beispiel/.nvm/versions/node/v22.1.0/bin"));
    }

    #[test]
    fn a_secret_segment_or_a_login_store_is_refused() {
        assert!(v("/srv/beispiel/.config/yarn/global/node_modules/paket"));
        assert!(v("/srv/beispiel/.local/share/keyrings"));
        assert!(v("/srv/beispiel/.ssh"));
        assert!(v("/srv/beispiel/.pi/agent/auth.json"));
        assert!(!v("/opt/beispiel/paket"));
    }
}
