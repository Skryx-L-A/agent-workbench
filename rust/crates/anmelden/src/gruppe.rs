//! The private group of the account (decision of the operator, 28.11.2026): the same rule as the holder, contract
//! `rust/spec/halter/README.md`, point "Private Gruppe". The code below the head and the test cases are a verbatim copy of
//! `private_gruppe`, `nss_nur_dateien` and `chmod_g_w` in `crates/halter/src/ablage.rs` (with their helpers): the rule
//! belongs to three security crates, and no crate that they all use is one of them. A change goes into all three
//! copies; the cases are the same in each.

/// A number field of `/etc/passwd` or `/etc/group`: only ASCII digits, as `u32`.
fn zahl(feld: &str) -> Option<u32> {
    (!feld.is_empty() && feld.bytes().all(|c| c.is_ascii_digit()))
        .then(|| feld.parse().ok())
        .flatten()
}

/// The private group of the account `uid` (decision of the operator, 28.11.2026), or `None`. Ubuntu gives an account
/// with a group of its own the umask 002, so `~/.local` is often 0775; the group write is harmless only if nobody else
/// is in that group. All three must hold: (a) the gid is the primary group (field 4) of the one entry of `uid` in
/// `passwd`; (b) `group` has exactly one line with that gid, and it names no member but the account; (c) no other entry
/// of `passwd` has that gid as primary group. Strict: any line in the form of NIS (`+` or `-` at the start), a broken
/// line of `passwd` that names the account, its uid or the gid, a broken line of `group` with the gid or the name of the
/// group, and a second entry of the account give `None`.
#[must_use]
pub fn private_gruppe(passwd: &str, group: &str, uid: u32) -> Option<u32> {
    let konten: Vec<Vec<&str>> = zeilen(passwd).collect();
    let mut eigene = konten
        .iter()
        .filter(|f| konto_zeile(f).is_some_and(|(u, _)| u == uid));
    let (Some(eigen), None) = (eigene.next(), eigene.next()) else {
        return None;
    };
    let name = eigen[0];
    let (_, gid) = konto_zeile(eigen)?;
    let (u, g) = (uid.to_string(), gid.to_string());
    // (c), and every other line that names the account, its uid or the gid must be readable
    let fremd = konten
        .iter()
        .filter(|f| !std::ptr::eq(*f, eigen))
        .any(|f| match konto_zeile(f) {
            Some((_, h)) => f[0] == name || h == gid,
            None => {
                nis(f)
                    || f[0] == name
                    || f.get(2) == Some(&u.as_str())
                    || f.get(3) == Some(&g.as_str())
            }
        });
    if fremd {
        return None;
    }
    // (b): exactly one line with the gid, no other member; a broken line with the gid or the name of the group refuses
    let gruppen: Vec<Vec<&str>> = zeilen(group).collect();
    let mut treffer = gruppen.iter().filter(|f| gruppen_zeile(f) == Some(gid));
    let (Some(eigene_gruppe), None) = (treffer.next(), treffer.next()) else {
        return None;
    };
    let kaputt = gruppen.iter().any(|f| {
        gruppen_zeile(f).is_none()
            && (nis(f) || f[0] == eigene_gruppe[0] || f.get(2) == Some(&g.as_str()))
    });
    let mitglied = eigene_gruppe[3]
        .split(',')
        .any(|m| !m.is_empty() && m != name);
    (!kaputt && !mitglied).then_some(gid)
}

/// The non-empty lines of `/etc/passwd` or `/etc/group`, split at `:`.
fn zeilen(text: &str) -> impl Iterator<Item = Vec<&str>> {
    text.split('\n')
        .filter(|z| !z.is_empty())
        .map(|z| z.split(':').collect())
}

/// A line in the form of NIS (`+` or `-` at the start): it can bring in any account or member, so it always refuses.
fn nis(f: &[&str]) -> bool {
    f[0].starts_with(['+', '-'])
}

/// uid and gid of a line of `/etc/passwd` in its form (seven fields, both numbers, not NIS), else `None`.
fn konto_zeile(f: &[&str]) -> Option<(u32, u32)> {
    if f.len() != 7 || nis(f) {
        return None;
    }
    Some((zahl(f[2])?, zahl(f[3])?))
}

/// The gid of a line of `/etc/group` in its form (four fields, a number, not NIS), else `None`.
fn gruppen_zeile(f: &[&str]) -> Option<u32> {
    if f.len() != 4 || nis(f) {
        return None;
    }
    zahl(f[2])
}

/// The databases of `/etc/nsswitch.conf` that say who is in a group: `passwd`, `group` and `initgroups`.
const NSS_DATENBANKEN: [&str; 3] = ["passwd", "group", "initgroups"];

/// `true` only when `/etc/nsswitch.conf` (the text `nsswitch`) takes `passwd` and `group`, and `initgroups` when it is
/// named, from the files and systemd alone (decision of the operator, 28.11.2026): a source the program cannot read
/// (`sss`, `ldap`, ...) could add a member, so the group is then not checkable. Strict: `passwd` and `group` stand
/// exactly once, `initgroups` at most once; each names at least one source, and every word after the colon is `files`
/// or `systemd` (an action in brackets, `compat` or an unknown word refuses). A comment starts at `#`.
#[must_use]
pub fn nss_nur_dateien(nsswitch: &str) -> bool {
    let mut anzahl = [0_usize; 3];
    for zeile in nsswitch.split('\n') {
        let text = zeile.split('#').next().unwrap_or_default();
        let (name, quellen) = match text.split_once(':') {
            Some((n, q)) => (n.trim(), Some(q)),
            None => (text.split_whitespace().next().unwrap_or_default(), None),
        };
        let Some(i) = NSS_DATENBANKEN.iter().position(|d| *d == name) else {
            continue;
        };
        anzahl[i] += 1;
        let Some(quellen) = quellen else {
            return false;
        };
        let mut woerter = quellen.split_whitespace().peekable();
        if woerter.peek().is_none() || !woerter.all(|w| w == "files" || w == "systemd") {
            return false;
        }
    }
    anzahl[0] == 1 && anzahl[1] == 1 && anzahl[2] <= 1
}

/// The command that takes the group write from the folders `pfade` (decision of the operator, 28.11.2026): `chmod g-w`
/// and each path, in single quotes when it is empty or holds another byte than a letter, a digit or one of `/._-`.
#[must_use]
pub fn chmod_g_w(pfade: &[String]) -> String {
    let mut befehl = String::from("chmod g-w");
    for p in pfade {
        befehl.push(' ');
        if !p.is_empty()
            && p.bytes()
                .all(|c| c.is_ascii_alphanumeric() || b"/._-".contains(&c))
        {
            befehl.push_str(p);
        } else {
            befehl.push('\'');
            befehl.push_str(&p.replace('\'', "'\\''"));
            befehl.push('\'');
        }
    }
    befehl
}

#[cfg(test)]
mod tests {
    use super::*;

    const P: &str = "root:x:0:0:root:/root:/bin/sh\nperson:x:1000:1000::/heim/beispiel/person:/bin/bash\n\
                     dienst:x:999:999::/:/usr/sbin/nologin\n";
    const G: &str = "root:x:0:\nperson:x:1000:\nsudo:x:27:person,andere\ndienst:x:999:\n";

    /// The decision of the operator of 28.11.2026: a private group of the account is found; each of (a), (b), (c) and
    /// each strict reading alone refuses (every case changes one line of the good pair).
    #[test]
    fn private_gruppe_nur_wenn_alle_drei_gelten() {
        assert_eq!(private_gruppe(P, G, 1000), Some(1000));
        // the account itself as member, also twice, and an empty member between commas
        assert_eq!(
            private_gruppe(
                P,
                &G.replace("person:x:1000:", "person:x:1000:person,,person"),
                1000
            ),
            Some(1000)
        );
        // the gid of another account or of nobody
        assert_eq!(private_gruppe(P, G, 999), Some(999));
        assert_eq!(private_gruppe(P, G, 7), None);
        let falsch: [(&str, String, String); 16] = [
            (
                "(b) second member",
                P.to_owned(),
                G.replace("person:x:1000:", "person:x:1000:andere"),
            ),
            (
                "(b) member after the own",
                P.to_owned(),
                G.replace("person:x:1000:", "person:x:1000:person,andere"),
            ),
            (
                "(b) no line with the gid",
                P.to_owned(),
                G.replace("person:x:1000:\n", ""),
            ),
            (
                "(b) two lines with the gid",
                P.to_owned(),
                format!("{G}zweite:x:1000:\n"),
            ),
            (
                "(b) broken line with the gid",
                P.to_owned(),
                format!("{G}kaputt:x:1000\n"),
            ),
            (
                "(b) broken line with the name",
                P.to_owned(),
                format!("{G}person:x\n"),
            ),
            ("(b) NIS in group", P.to_owned(), format!("{G}+:::\n")),
            (
                "(b) gid no number",
                P.to_owned(),
                G.replace("person:x:1000:", "person:x:1000a:"),
            ),
            (
                "(b) gid with a sign",
                P.to_owned(),
                G.replace("person:x:1000:", "person:x:+1000:"),
            ),
            (
                "(c) second account with the gid",
                format!("{P}andere:x:1001:1000::/heim/beispiel/a:/bin/sh\n"),
                G.to_owned(),
            ),
            (
                "(a) two entries of the uid",
                format!("{P}zwei:x:1000:1000::/heim/beispiel/z:/bin/sh\n"),
                G.to_owned(),
            ),
            (
                "second entry with the name",
                format!("{P}person:x:1002:1002::/heim/beispiel/z:/bin/sh\n"),
                G.to_owned(),
            ),
            (
                "broken line with the gid",
                format!("{P}kaputt:x:1003:1000\n"),
                G.to_owned(),
            ),
            (
                "broken line with the uid",
                format!("{P}kaputt:x:1000\n"),
                G.to_owned(),
            ),
            (
                "broken line with the name",
                format!("{P}person:x:1\n"),
                G.to_owned(),
            ),
            ("NIS in passwd", format!("{P}+::::::\n"), G.to_owned()),
        ];
        for (grund, p, g) in &falsch {
            assert_eq!(private_gruppe(p, g, 1000), None, "{grund}");
        }
        // a broken line that names none of them does not count
        assert_eq!(
            private_gruppe(&format!("{P}kaputt:x:5\n"), &format!("{G}kaputt:x\n"), 1000),
            Some(1000)
        );
        // the own entry broken: no account, no group
        let p = P.replace("person:x:1000:1000:", "person:x:1000:10x0:");
        assert_eq!(private_gruppe(&p, G, 1000), None);
        assert_eq!(private_gruppe("", G, 1000), None);
    }

    /// The decision of the operator of 28.11.2026: only `files` and `systemd` for `passwd`, `group` and `initgroups`;
    /// every other source, an action, a missing or doubled line refuses.
    #[test]
    fn nss_nur_aus_den_dateien() {
        let gut = "# comment\npasswd:         files systemd\ngroup:          files systemd\nshadow:         files\n\
                   hosts:          files dns\nnetgroup:       nis\n";
        assert!(nss_nur_dateien(gut));
        assert!(nss_nur_dateien("passwd: files\ngroup: files\n"));
        assert!(nss_nur_dateien(
            "passwd:files\ngroup:\tsystemd files\ninitgroups: files\n"
        ));
        assert!(nss_nur_dateien(
            "passwd: files # sss\ngroup: files\n#group: sss\n"
        ));
        let falsch = [
            ("sss for group", "passwd: files\ngroup: files sss\n"),
            ("ldap for passwd", "passwd: files ldap\ngroup: files\n"),
            ("only sss", "passwd: files\ngroup: sss\n"),
            (
                "an action",
                "passwd: files\ngroup: files [NOTFOUND=return] systemd\n",
            ),
            ("compat", "passwd: compat\ngroup: compat\n"),
            ("unknown word", "passwd: files\ngroup: files winbind\n"),
            ("no source", "passwd: files\ngroup:\n"),
            ("no colon", "passwd: files\ngroup files\n"),
            ("group missing", "passwd: files\n"),
            ("passwd missing", "group: files\n"),
            ("group twice", "passwd: files\ngroup: files\ngroup: files\n"),
            (
                "passwd twice",
                "passwd: files\npasswd: files\ngroup: files\n",
            ),
            (
                "initgroups with sss",
                "passwd: files\ngroup: files\ninitgroups: files sss\n",
            ),
            (
                "initgroups twice",
                "passwd: files\ngroup: files\ninitgroups: files\ninitgroups: files\n",
            ),
            ("empty", ""),
        ];
        for (grund, text) in falsch {
            assert!(!nss_nur_dateien(text), "{grund}");
        }
    }

    #[test]
    fn chmod_mit_den_pfaden() {
        let p = |v: &[&str]| v.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
        assert_eq!(
            chmod_g_w(&p(&[
                "/srv/beispiel/wb/.local",
                "/srv/beispiel/wb/.local/state"
            ])),
            "chmod g-w /srv/beispiel/wb/.local /srv/beispiel/wb/.local/state"
        );
        assert_eq!(chmod_g_w(&p(&["/h/a b"])), "chmod g-w '/h/a b'");
        assert_eq!(chmod_g_w(&p(&["/h/it's"])), "chmod g-w '/h/it'\\''s'");
        assert_eq!(chmod_g_w(&p(&["", "/h_1-2.x"])), "chmod g-w '' /h_1-2.x");
        assert_eq!(chmod_g_w(&p(&["/h/\u{e4}"])), "chmod g-w '/h/\u{e4}'");
        assert_eq!(chmod_g_w(&[]), "chmod g-w");
    }
}
