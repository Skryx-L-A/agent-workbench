//! The company default (contract, sections 15.2, 15.11 and 15.12): its form (stage H4), the check of file, folder and
//! parents from the data of `fstat` (stage H3) and the part of the [`Stand`] that comes from it.
//!
//! The library reads no file. The caller opens folder and file through descriptors, measures them with `fstat` and
//! hands in the data ([`Angabe`]) and the bytes. The mapping of an account to a pseudonym belongs to the caller as well
//! (15.2, DI-L11 point 4); [`Firmenvorgabe::stand`] takes it as a function.

use serde_json::{Map, Value};

use crate::einstellung::{Stand, Werte};
use crate::json;
use crate::katalog::{self, LESEKLASSEN};

/// Name of the list of the admin accounts.
pub const ADMINS: &str = "admins";
/// Most bytes of the file; a longer file is a load error (H4, DI-E2 point 1).
pub const GRENZE_DATEI: usize = 1_048_576;
/// Write permission for group or others (H3).
pub const UNSICHER: u32 = 0o022;
/// uid of root, the owner of every parent folder (H3).
pub const ROOT: u32 = 0;

/// The two codes of the company default that the library decides (15.12.3, H3 and H4; DI-E2 point 1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Vorgabefehler {
    /// H3: the file, its folder or a parent is not in the required form.
    DateiUnsicher,
    /// H4: the file does not have the form, or an account in `leseklassen` has no pseudonym.
    Ladefehler,
}

impl Vorgabefehler {
    /// The fixed text of the code.
    #[must_use]
    pub fn text(self) -> &'static str {
        match self {
            Vorgabefehler::DateiUnsicher => "firmenvorgabe.datei_unsicher",
            Vorgabefehler::Ladefehler => "firmenvorgabe.ladefehler",
        }
    }
}

/// The kind of a directory entry, as `lstat` or `fstat` reports it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Art {
    /// A regular file.
    Datei,
    /// A folder.
    Ordner,
    /// A symbolic link.
    Verweis,
    /// Anything else (socket, FIFO, device).
    Andere,
}

/// What H3 needs of one entry: kind, owner and permission bits (`st_mode & 0o7777`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Angabe {
    /// The kind.
    pub art: Art,
    /// The uid of the owner.
    pub uid: u32,
    /// The permission bits.
    pub modus: u32,
}

/// `true` if the entry has the kind `art`, belongs to `eigner` and is writable neither for the group nor for others.
#[must_use]
pub fn sicher(a: &Angabe, art: Art, eigner: u32) -> bool {
    a.art == art && a.uid == eigner && a.modus & UNSICHER == 0
}

/// The data of H3 for one check. `None` stands for a call that failed (the entry is missing, `O_NOFOLLOW` met a
/// link, `fstat` failed); it counts as not safe.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lage {
    /// Every parent of the folder up to `/`, from `lstat`.
    pub eltern: Vec<Option<Angabe>>,
    /// The folder, from `fstat` of its open descriptor.
    pub ordner: Option<Angabe>,
    /// The file, from `fstat` of its open descriptor.
    pub datei: Option<Angabe>,
    /// uid of the system user `werkbank-vorgabe`, owner of folder and file.
    pub helfer_uid: u32,
}

/// H3 (15.12.3, DI-E2 point 1): every parent is a folder of root without `& 022`, the folder is a folder of the helper
/// user without `& 022`, the file is a regular file of the helper user without `& 022`.
///
/// # Errors
/// [`Vorgabefehler::DateiUnsicher`] at the first entry that is not safe, in the order parents, folder, file.
pub fn h3(lage: &Lage) -> Result<(), Vorgabefehler> {
    let gut = |a: &Option<Angabe>, art, eigner| a.as_ref().is_some_and(|a| sicher(a, art, eigner));
    if lage.eltern.iter().all(|e| gut(e, Art::Ordner, ROOT))
        && gut(&lage.ordner, Art::Ordner, lage.helfer_uid)
        && gut(&lage.datei, Art::Datei, lage.helfer_uid)
    {
        Ok(())
    } else {
        Err(Vorgabefehler::DateiUnsicher)
    }
}

/// `true` for an account name of the form `[a-z_][a-z0-9_-]*`, without a line end.
#[must_use]
pub fn ist_konto(s: &str) -> bool {
    let mut zeichen = s.bytes();
    let Some(erstes) = zeichen.next() else {
        return false;
    };
    (erstes.is_ascii_lowercase() || erstes == b'_')
        && zeichen.all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'-')
}

fn liste_der_admins(a: &Value) -> Option<Vec<&str>> {
    let liste = a.as_array()?;
    let mut namen: Vec<&str> = Vec::with_capacity(liste.len());
    for k in liste {
        let k = k.as_str()?;
        if !ist_konto(k) || namen.contains(&k) {
            return None;
        }
        namen.push(k);
    }
    Some(namen)
}

fn leseklassen_der_form(l: &Value) -> bool {
    let Some(liste) = l.as_array() else {
        return false;
    };
    if liste.len() > katalog::LESEKLASSEN_MAX {
        return false;
    }
    let mut paare: Vec<(&str, &str)> = Vec::with_capacity(liste.len());
    for e in liste {
        let Some(e) = e.as_object() else {
            return false;
        };
        if e.len() != 3 {
            return false;
        }
        let (Some(ebene), Some(wer), Some(_)) = (
            e.get("ebene").and_then(Value::as_str),
            e.get("wer").and_then(Value::as_str),
            e.get("klasse").and_then(Value::as_str),
        ) else {
            return false;
        };
        if !matches!(ebene, "nutzer" | "gruppe") || paare.contains(&(ebene, wer)) {
            return false;
        }
        paare.push((ebene, wer));
    }
    true
}

/// The admin accounts of `inhalt` if it has the form of the company default (H4, H8): an object with at least
/// `admins` (distinct accounts) and `leseklassen` (at most 4096 objects with exactly `ebene` `nutzer` or `gruppe`,
/// `wer` and `klasse` as texts, each pair of `ebene` and `wer` once). Further keys are allowed. Classes and groups are
/// not checked here; [`Firmenvorgabe::stand`] does that. `None` otherwise.
#[must_use]
pub fn admins_der_form(inhalt: &Value) -> Option<Vec<&str>> {
    let o = inhalt.as_object()?;
    let admins = liste_der_admins(o.get(ADMINS)?)?;
    leseklassen_der_form(o.get(LESEKLASSEN)?).then_some(admins)
}

/// The loaded company default: the accounts as the file names them.
#[derive(Debug, Clone, PartialEq)]
pub struct Firmenvorgabe {
    /// The list of the admin accounts, in the order of the file.
    pub admins: Vec<String>,
    /// `leseklassen` as in the file: at `ebene` `nutzer`, `wer` is an account.
    pub leseklassen: Vec<Value>,
    /// The whole content of the file; the service builds the request to the write helper from it (DI-E1 point 4).
    pub inhalt: Value,
    /// sha256 (hex, lower case) of the loaded bytes, the `basis` of a request; `None` without a file.
    pub sha256: Option<String>,
}

/// H4 for the bytes of the file (DI-E2 point 1): at most 1048576 bytes, one JSON value read strictly
/// ([`json::streng_lesen`]), and the form of [`admins_der_form`].
///
/// # Errors
/// [`Vorgabefehler::Ladefehler`].
pub fn laden(bytes: &[u8]) -> Result<Firmenvorgabe, Vorgabefehler> {
    if bytes.len() > GRENZE_DATEI {
        return Err(Vorgabefehler::Ladefehler);
    }
    let inhalt = json::streng_lesen(bytes).ok_or(Vorgabefehler::Ladefehler)?;
    let admins = admins_der_form(&inhalt)
        .ok_or(Vorgabefehler::Ladefehler)?
        .into_iter()
        .map(str::to_owned)
        .collect();
    let leseklassen = inhalt[LESEKLASSEN].as_array().cloned().unwrap_or_default();
    Ok(Firmenvorgabe {
        admins,
        leseklassen,
        sha256: Some(formate::kette::sha256_hex(bytes)),
        inhalt,
    })
}

impl Firmenvorgabe {
    /// No company default: the file is missing (15.12.4, "Fehlende Firmenvorgabe"). Nobody is admin, nobody reads.
    #[must_use]
    pub fn fehlt() -> Firmenvorgabe {
        Firmenvorgabe {
            admins: Vec::new(),
            leseklassen: Vec::new(),
            inhalt: Value::Null,
            sha256: None,
        }
    }

    /// Every account of the file once, in the order of the file: the admins first, then `wer` of the entries of
    /// `leseklassen` with `ebene` `nutzer`. The service maps these accounts to pseudonyms (15.2).
    #[must_use]
    pub fn konten(&self) -> Vec<&str> {
        let mut aus: Vec<&str> = Vec::new();
        let nutzer = self.leseklassen.iter().filter_map(|x| {
            (x.get("ebene").and_then(Value::as_str) == Some("nutzer"))
                .then(|| x.get("wer").and_then(Value::as_str))
                .flatten()
        });
        for k in self.admins.iter().map(String::as_str).chain(nutzer) {
            if !aus.contains(&k) {
                aus.push(k);
            }
        }
        aus
    }

    /// The state for `setzen` and `sicht` (15.3) from the company file (`klassen`, `gruppen`), the stored `werte`
    /// and this company default, with `pseudonym` as the mapping of an account to the pseudonym of the person.
    ///
    /// - `admins` holds the pseudonym of every admin account that `pseudonym` maps to a pseudonym, once each. An account
    ///   without one is no admin (fail closed, DI-L11 point 4).
    /// - `werte.firma.leseklassen` is the list of the file with the pseudonym in place of each account. It replaces a
    ///   stored value of that key.
    ///
    /// # Errors
    /// [`Vorgabefehler::Ladefehler`] if an account in `leseklassen` has no pseudonym, or if the list with pseudonyms
    /// is not valid after 15.11 (a class not in `klassen`, a group not in `gruppen`); no gate comes into being
    /// (15.12.3, "Ladefehler").
    pub fn stand(
        &self,
        klassen: Vec<String>,
        gruppen: Vec<(String, Vec<String>)>,
        mut werte: Werte,
        mut pseudonym: impl FnMut(&str) -> Option<String>,
    ) -> Result<Stand, Vorgabefehler> {
        let mut admins: Vec<String> = Vec::new();
        for k in &self.admins {
            if let Some(p) = pseudonym(k).filter(|p| katalog::ist_pseudonym(p))
                && !admins.contains(&p)
            {
                admins.push(p);
            }
        }
        let mut liste = Vec::with_capacity(self.leseklassen.len());
        for x in &self.leseklassen {
            let mut x = x.clone();
            if let Some(o) = x.as_object_mut()
                && o.get("ebene").and_then(Value::as_str) == Some("nutzer")
            {
                let konto = o.get("wer").and_then(Value::as_str).unwrap_or_default();
                let p = pseudonym(konto).ok_or(Vorgabefehler::Ladefehler)?;
                o.insert("wer".to_owned(), Value::String(p));
            }
            liste.push(x);
        }
        let liste = Value::Array(liste);
        if !katalog::leseklassen_gut(&liste, &klassen, &gruppen) {
            return Err(Vorgabefehler::Ladefehler);
        }
        werte.firma.insert(LESEKLASSEN.to_owned(), liste);
        Ok(Stand {
            klassen,
            admins,
            gruppen,
            werte,
        })
    }
}

/// The company default as the service sends it to the write helper (DI-E1 point 4): the loaded content in which only
/// `leseklassen` carries `neu` (in the account form the admin typed). `None` without a loaded object.
#[must_use]
pub fn inhalt_mit(geladen: &Value, neu: &Value) -> Option<Value> {
    let mut o: Map<String, Value> = geladen.as_object()?.clone();
    o.insert(LESEKLASSEN.to_owned(), neu.clone());
    Some(Value::Object(o))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn le(ebene: &str, wer: &str) -> Value {
        json!({"ebene": ebene, "wer": wer, "klasse": "intern"})
    }

    #[allow(clippy::unnecessary_wraps)]
    fn a(art: Art, uid: u32, modus: u32) -> Option<Angabe> {
        Some(Angabe { art, uid, modus })
    }

    fn lage() -> Lage {
        Lage {
            eltern: vec![a(Art::Ordner, 0, 0o755), a(Art::Ordner, 0, 0o755)],
            ordner: a(Art::Ordner, 990, 0o755),
            datei: a(Art::Datei, 990, 0o644),
            helfer_uid: 990,
        }
    }

    #[test]
    fn codes() {
        assert_eq!(
            [Vorgabefehler::DateiUnsicher, Vorgabefehler::Ladefehler].map(Vorgabefehler::text),
            ["firmenvorgabe.datei_unsicher", "firmenvorgabe.ladefehler"]
        );
        assert_eq!((GRENZE_DATEI, UNSICHER, ROOT), (1_048_576, 0o022, 0));
    }

    #[test]
    fn h3_je_ort() {
        assert_eq!(h3(&lage()), Ok(()));
        let mut ohne_eltern = lage();
        ohne_eltern.eltern.clear();
        assert_eq!(h3(&ohne_eltern), Ok(()));
        let schlecht: [fn(&mut Lage); 13] = [
            |l| l.eltern[1] = None,
            |l| l.eltern[0] = a(Art::Ordner, 5, 0o755),
            |l| l.eltern[1] = a(Art::Ordner, 0, 0o775),
            |l| l.eltern[0] = a(Art::Verweis, 0, 0o755),
            |l| l.ordner = None,
            |l| l.ordner = a(Art::Ordner, 0, 0o755),
            |l| l.ordner = a(Art::Ordner, 990, 0o757),
            |l| l.ordner = a(Art::Verweis, 990, 0o755),
            |l| l.datei = None,
            |l| l.datei = a(Art::Datei, 1001, 0o644),
            |l| l.datei = a(Art::Datei, 990, 0o664),
            |l| l.datei = a(Art::Ordner, 990, 0o644),
            |l| l.datei = a(Art::Andere, 990, 0o644),
        ];
        for (i, f) in schlecht.iter().enumerate() {
            let mut l = lage();
            f(&mut l);
            assert_eq!(h3(&l), Err(Vorgabefehler::DateiUnsicher), "{i}");
        }
    }

    #[test]
    fn sicher_verlangt_art_eigner_und_modus() {
        let m = |art, uid, modus| Angabe { art, uid, modus };
        assert!(sicher(&m(Art::Datei, 5, 0o644), Art::Datei, 5));
        assert!(sicher(&m(Art::Datei, 5, 0o755), Art::Datei, 5));
        assert!(!sicher(&m(Art::Datei, 5, 0o664), Art::Datei, 5), "group");
        assert!(!sicher(&m(Art::Datei, 5, 0o646), Art::Datei, 5), "others");
        assert!(!sicher(&m(Art::Datei, 6, 0o644), Art::Datei, 5), "owner");
        assert!(!sicher(&m(Art::Verweis, 5, 0o644), Art::Datei, 5), "link");
    }

    #[test]
    fn konto() {
        for gut in ["a", "_", "konto-admin", "k0_9-x", "_a"] {
            assert!(ist_konto(gut), "{gut}");
        }
        for schlecht in [
            "",
            "-a",
            "0a",
            "A",
            "konto admin",
            "konto\n",
            "kö",
            "a/b",
            "a.",
        ] {
            assert!(!ist_konto(schlecht), "{schlecht:?}");
        }
    }

    #[test]
    fn form() {
        let gut = json!({"admins": ["konto-a", "konto-b"], "leseklassen": [le("nutzer", "x"), le("gruppe", "x")]});
        assert_eq!(admins_der_form(&gut), Some(vec!["konto-a", "konto-b"]));
        assert_eq!(
            admins_der_form(&json!({"admins": [], "leseklassen": [], "weiteres": 1})),
            Some(vec![])
        );
        for schlecht in [
            json!([]),
            json!({"admins": []}),
            json!({"leseklassen": []}),
            json!({"admins": "konto-a", "leseklassen": []}),
            json!({"admins": ["konto-a", "konto-a"], "leseklassen": []}),
            json!({"admins": ["Konto"], "leseklassen": []}),
            json!({"admins": [1], "leseklassen": []}),
            json!({"admins": [], "leseklassen": {}}),
            json!({"admins": [], "leseklassen": [1]}),
            json!({"admins": [], "leseklassen": [le("nutzer", "x"), le("nutzer", "x")]}),
            json!({"admins": [], "leseklassen": [le("firma", "x")]}),
            json!({"admins": [], "leseklassen": [{"ebene": "nutzer", "wer": "x"}]}),
            json!({"admins": [], "leseklassen": [{"ebene": "nutzer", "wer": "x", "klasse": 1}]}),
            json!({"admins": [], "leseklassen": [{"ebene": "nutzer", "wer": 1, "klasse": "a"}]}),
            json!({"admins": [], "leseklassen": [{"ebene": 1, "wer": "x", "klasse": "a"}]}),
            json!({"admins": [], "leseklassen": [{"ebene": "nutzer", "wer": "x", "klasse": "a", "k": []}]}),
        ] {
            assert_eq!(admins_der_form(&schlecht), None, "{schlecht}");
        }
        let liste: Vec<Value> = (0..katalog::LESEKLASSEN_MAX)
            .map(|i| le("nutzer", &format!("k{i}")))
            .collect();
        let mut v = json!({"admins": [], "leseklassen": liste});
        assert!(admins_der_form(&v).is_some(), "4096 entries");
        v[LESEKLASSEN]
            .as_array_mut()
            .expect("list")
            .push(le("nutzer", "eins-mehr"));
        assert!(admins_der_form(&v).is_none(), "4097 entries");
    }

    #[test]
    fn laden_grenze_und_pruefsumme() {
        let bytes = br#"{"admins": ["konto-a"], "leseklassen": [{"ebene": "nutzer", "wer": "konto-n", "klasse": "intern"}]}"#;
        let v = laden(bytes).expect("valid");
        assert_eq!(v.admins, vec!["konto-a"]);
        assert_eq!(v.leseklassen.len(), 1);
        assert_eq!(
            v.sha256.as_deref(),
            Some(formate::kette::sha256_hex(bytes).as_str())
        );
        assert_eq!(v.konten(), vec!["konto-a", "konto-n"]);
        assert_eq!(
            laden(br#"{"admins": [], "admins": [], "leseklassen": []}"#),
            Err(Vorgabefehler::Ladefehler)
        );
        assert_eq!(laden(b"[]"), Err(Vorgabefehler::Ladefehler));
        let mut gross = br#"{"admins": [], "leseklassen": []}"#.to_vec();
        gross.resize(GRENZE_DATEI, b' ');
        assert!(laden(&gross).is_ok(), "1048576 bytes");
        gross.push(b' ');
        assert_eq!(
            laden(&gross),
            Err(Vorgabefehler::Ladefehler),
            "1048577 bytes"
        );
        assert_eq!(Firmenvorgabe::fehlt().konten(), Vec::<&str>::new());
    }

    #[test]
    fn stand_mit_zuordnung() {
        let p = |n: u8| format!("ps_{n:032x}");
        let v = laden(
            br#"{"admins": ["konto-a", "konto-x", "konto-a2"], "leseklassen": [
                {"ebene": "gruppe", "wer": "g-eins", "klasse": "intern"},
                {"ebene": "nutzer", "wer": "konto-n", "klasse": "oeffentlich"}]}"#,
        )
        .expect("valid");
        let zu = |k: &str| match k {
            "konto-a" => Some(p(1)),
            "konto-a2" => Some("kein-pseudonym".to_owned()),
            "konto-n" => Some(p(3)),
            _ => None,
        };
        let klassen = vec!["oeffentlich".to_owned(), "intern".to_owned()];
        let gruppen = vec![("g-eins".to_owned(), vec![p(3)])];
        let mut werte = Werte::default();
        werte.firma.insert(LESEKLASSEN.to_owned(), json!("alt"));
        let s = v
            .stand(klassen.clone(), gruppen.clone(), werte, zu)
            .expect("state");
        assert_eq!(s.admins, vec![p(1)], "konto-x and konto-a2 are no admins");
        assert_eq!(
            s.werte.firma[LESEKLASSEN],
            json!([{"ebene": "gruppe", "wer": "g-eins", "klasse": "intern"},
                   {"ebene": "nutzer", "wer": p(3), "klasse": "oeffentlich"}])
        );
        assert_eq!(
            v.stand(klassen.clone(), gruppen.clone(), Werte::default(), |k| (k
                != "konto-n")
                .then(|| p(1))),
            Err(Vorgabefehler::Ladefehler),
            "an account in leseklassen without a pseudonym"
        );
        assert_eq!(
            v.stand(klassen.clone(), Vec::new(), Werte::default(), zu),
            Err(Vorgabefehler::Ladefehler),
            "unknown group"
        );
        assert_eq!(
            v.stand(
                vec!["oeffentlich".to_owned()],
                gruppen.clone(),
                Werte::default(),
                zu
            ),
            Err(Vorgabefehler::Ladefehler),
            "unknown class"
        );
        let doppelt =
            laden(br#"{"admins": ["konto-a", "konto-b"], "leseklassen": []}"#).expect("valid");
        let s = doppelt
            .stand(klassen.clone(), gruppen.clone(), Werte::default(), |_| {
                Some(p(1))
            })
            .expect("state");
        assert_eq!(s.admins, vec![p(1)], "one pseudonym once");
        let leer = Firmenvorgabe::fehlt()
            .stand(klassen, gruppen, Werte::default(), zu)
            .expect("state");
        assert_eq!(leer.admins, Vec::<String>::new());
        assert_eq!(leer.werte.firma[LESEKLASSEN], json!([]));
    }

    #[test]
    fn inhalt_fuer_den_helfer() {
        let geladen = json!({"admins": ["konto-a"], "leseklassen": [], "x": 1});
        assert_eq!(
            inhalt_mit(&geladen, &json!([le("nutzer", "konto-n")])),
            Some(json!({"admins": ["konto-a"], "leseklassen": [le("nutzer", "konto-n")], "x": 1}))
        );
        assert_eq!(inhalt_mit(&Value::Null, &json!([])), None);
    }
}
