//! The private group of the account in the program (decision of the operator, 28.11.2026, and its addendum 1; contract
//! 9, rules for the store, and the rule of the holder, point "Private Gruppe"). The program reads the test `passwd`, the
//! test `group` and the test `nsswitch.conf` through the seams `WB_ANMELDEN_PASSWD`, `WB_ANMELDEN_GROUP` and
//! `WB_ANMELDEN_NSSWITCH` of the test build. The stand-in of the CLI is missing on purpose: a run that gets past the
//! store ends with `cli_fehlt` (exit 10), a refused store with `ziel_nicht_schreibbar` (exit 14), before the CLI is
//! looked for; a refused group write names the remedy `chmod g-w` with the paths first.
#![cfg(all(unix, feature = "pruefbau"))] // the crate is Unix only (see `src/lib.rs`)

mod gemeinsam;

use std::fs;
use std::os::unix::fs::PermissionsExt;

use gemeinsam::{
    Aufbau, HINWEIS_GRUPPE, ablage_ebenen, aufbau, eigene_gid, eigene_uid, group_schreiben, laufen,
};

const ZIEL: &str =
    "werkbank-anmelden: anmelden.ziel_nicht_schreibbar: Der Zielordner ist nicht schreibbar.\n";
const CLI: &str = "werkbank-anmelden: anmelden.cli_fehlt: Die Copilot CLI wurde nicht gefunden.\n";

/// The home and the four levels above `geheim`.
fn oben(a: &Aufbau) -> Vec<std::path::PathBuf> {
    let mut oben = vec![a.heim.clone()];
    oben.extend(ablage_ebenen(&a.heim).into_iter().take(4));
    oben
}

/// The home and the four levels above `geheim` with `modus`, as umask 002 makes them.
fn oben_setzen(a: &Aufbau, modus: u32) {
    for ebene in oben(a) {
        fs::set_permissions(ebene, fs::Permissions::from_mode(modus)).expect("mode");
    }
}

/// The refusal with the note: `chmod g-w` and the home with the four levels.
fn abgelehnt(a: &Aufbau) -> (i32, String) {
    let pfade: Vec<String> = oben(a).iter().map(|p| p.display().to_string()).collect();
    (14, format!("{HINWEIS_GRUPPE} {}\n{ZIEL}", pfade.join(" ")))
}

fn lauf(a: &Aufbau) -> (i32, String) {
    let aus = laufen(a, &["copilot".to_owned(), "--neu".to_owned()]);
    (aus.exit, aus.stderr)
}

#[test]
fn the_private_group_with_0775_gets_past_the_store() {
    let a = aufbau();
    group_schreiben(&a, "");
    oben_setzen(&a, 0o775);
    assert_eq!(lauf(&a), (10, CLI.to_owned()));
    // files alone count as well
    fs::write(&a.nsswitch, "passwd: files\ngroup: files\n").expect("nsswitch");
    assert_eq!(lauf(&a), (10, CLI.to_owned()), "files alone");
    // counter-check: the same levels without the group file end at the store, with the note
    fs::remove_file(&a.group).expect("remove");
    assert_eq!(lauf(&a), abgelehnt(&a));
}

#[test]
fn every_other_case_refuses_the_store_as_before() {
    let a = aufbau();
    oben_setzen(&a, 0o775);
    let passwd = fs::read_to_string(&a.passwd).expect("passwd");
    // a second member of the group
    group_schreiben(&a, "andere");
    assert_eq!(lauf(&a), abgelehnt(&a), "second member");
    // a second account with the group as its primary group
    group_schreiben(&a, "");
    fs::write(
        &a.passwd,
        format!(
            "{passwd}andere:x:{}:{}::/srv/beispiel/andere:/bin/sh\n",
            eigene_uid(&a) + 1,
            eigene_gid(&a)
        ),
    )
    .expect("passwd");
    assert_eq!(lauf(&a), abgelehnt(&a), "second account");
    fs::write(&a.passwd, &passwd).expect("passwd");
    assert_eq!(lauf(&a), (10, CLI.to_owned()), "the good pair again");
    // the switch of the name services (addendum 1): another source, or no readable file
    let gut = fs::read_to_string(&a.nsswitch).expect("nsswitch");
    for (grund, text) in [
        ("sss", Some("passwd: files sss\ngroup: files sss\n")),
        ("ldap", Some("passwd: files ldap\ngroup: files ldap\n")),
        (
            "an action",
            Some("passwd: files\ngroup: files [NOTFOUND=return] systemd\n"),
        ),
        ("missing", None),
    ] {
        let _ = fs::remove_file(&a.nsswitch);
        if let Some(t) = text {
            fs::write(&a.nsswitch, t).expect("nsswitch");
        }
        assert_eq!(lauf(&a), abgelehnt(&a), "{grund}");
    }
    fs::create_dir(&a.nsswitch).expect("folder");
    assert_eq!(lauf(&a), abgelehnt(&a), "nsswitch is a folder");
    fs::remove_dir(&a.nsswitch).expect("remove");
    fs::write(&a.nsswitch, gut).expect("nsswitch");
    assert_eq!(lauf(&a), (10, CLI.to_owned()), "the good switch again");
    // o+w at a level, also with the private group: no remedy with chmod g-w for that level
    let local = a.heim.join(".local");
    fs::set_permissions(&local, fs::Permissions::from_mode(0o777)).expect("mode");
    assert_eq!(lauf(&a), (14, ZIEL.to_owned()), "o+w");
    fs::set_permissions(&local, fs::Permissions::from_mode(0o775)).expect("mode");
    // `/etc/group` that cannot be read: a folder in its place
    fs::remove_file(&a.group).expect("remove");
    fs::create_dir(&a.group).expect("folder");
    assert_eq!(lauf(&a), abgelehnt(&a), "group is a folder");
}
