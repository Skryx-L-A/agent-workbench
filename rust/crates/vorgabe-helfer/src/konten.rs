//! Accounts of the system: uid to account (`getpwuid_r`, H1) and account to uid (`getpwnam_r`, H5).

/// The lookups of accounts. A test gives a fixed table.
pub trait Konten {
    /// The account name of `uid`, `None` without an account.
    fn konto(&self, uid: u32) -> Option<String>;
    /// The uid of the account `name`, `None` without such an account.
    fn uid(&self, name: &str) -> Option<u32>;
}

/// The lookups of the system through NSS (`getpwuid_r`, `getpwnam_r`). An error counts as "no account": fail closed.
#[derive(Debug, Clone, Copy, Default)]
pub struct System;

impl Konten for System {
    fn konto(&self, uid: u32) -> Option<String> {
        nix::unistd::User::from_uid(nix::unistd::Uid::from_raw(uid))
            .ok()
            .flatten()
            .map(|u| u.name)
    }

    fn uid(&self, name: &str) -> Option<u32> {
        nix::unistd::User::from_name(name)
            .ok()
            .flatten()
            .map(|u| u.uid.as_raw())
    }
}

/// A fixed table of accounts (tests and the outside test of the cases).
#[derive(Debug, Clone, Default)]
pub struct Tabelle(pub Vec<(String, u32)>);

impl Konten for Tabelle {
    fn konto(&self, uid: u32) -> Option<String> {
        self.0
            .iter()
            .find(|(_, u)| *u == uid)
            .map(|(k, _)| k.clone())
    }

    fn uid(&self, name: &str) -> Option<u32> {
        self.0.iter().find(|(k, _)| k == name).map(|(_, u)| *u)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_kennt_das_eigene_konto() {
        let uid = rustix::process::getuid().as_raw();
        let name = System.konto(uid).expect("own account");
        assert_eq!(System.uid(&name), Some(uid));
        assert_eq!(System.uid("konto-das-es-nicht-gibt-0815"), None);
        assert_eq!(System.konto(u32::MAX - 7), None);
        assert_eq!(System.uid("root"), Some(0));
        assert_eq!(System.konto(0).as_deref(), Some("root"));
    }

    #[test]
    fn tabelle_in_beide_richtungen() {
        let t = Tabelle(vec![("a".into(), 1), ("b".into(), 2)]);
        assert_eq!(t.konto(2).as_deref(), Some("b"));
        assert_eq!(t.uid("a"), Some(1));
        assert_eq!(t.konto(3), None);
        assert_eq!(t.uid("c"), None);
    }
}
