//! The state of the machine as the seven checks report it (contract: section "Prüfbefehl", Ubuntu).
//!
//! A check reports one state. `ok` is the green state; every other state names its sentence in a table below.
//! The tables are the only place where a state name is written; a state outside its table is refused when a
//! [`Lage`] is read, so the sentences in `bewerte` cannot meet an unknown name.

use serde_json::Value;

/// The green state of every check.
pub const OK: &str = "ok";

/// A state of a service unit and the sentence for it (`{n}` is the unit name).
pub const DIENST: &[(&str, &str)] = &[
    (
        "fehlt",
        "pruef.dienst_fehlt: Der Dienst {n} ist nicht eingetragen.",
    ),
    (
        "inaktiv",
        "pruef.dienst_inaktiv: Der Dienst {n} ist nicht aktiv.",
    ),
    (
        "fehlgeschlagen",
        "pruef.dienst_fehlgeschlagen: Der Dienst {n} ist fehlgeschlagen.",
    ),
    (
        "nicht_pruefbar",
        "pruef.dienst_nicht_pruefbar: Der Dienst {n} war nicht pruefbar.",
    ),
];

/// The units of the package, in the order of the output. The model path is no unit of the package: `werkbank-lauf` starts it
/// for each session (contract part A, "Start der Dienste").
pub const DIENSTE: [&str; 2] = ["werkbank-kern.service", "werkbank-bruecke.service"];

/// The state of the `AppArmor` profile and the sentence for it.
pub const APPARMOR: &[(&str, &str)] = &[
    (
        "bwrap_fehlt",
        "pruef.apparmor_bwrap_fehlt: Das Programm /usr/bin/bwrap fehlt.",
    ),
    (
        "datei_fehlt",
        "pruef.apparmor_datei_fehlt: Die Datei des AppArmor-Profils fehlt.",
    ),
    (
        "nicht_geladen",
        "pruef.apparmor_nicht_geladen: Das AppArmor-Profil ist nicht geladen.",
    ),
    (
        "abweichend",
        "pruef.apparmor_abweichend: Das AppArmor-Profil weicht vom Paket ab.",
    ),
    (
        "fremd",
        "pruef.apparmor_fremd: Ein anderes AppArmor-Profil gilt fuer /usr/bin/bwrap.",
    ),
    (
        "probelauf",
        "pruef.apparmor_probelauf: Der Probelauf des Einschlusses scheitert.",
    ),
    (
        "unconfined",
        "pruef.apparmor_unconfined: Ein Profil der Werkbank ist ohne Einschraenkung geladen.",
    ),
    (
        "system_bwrap_frei",
        "pruef.apparmor_system_bwrap_frei: Das System-bwrap ist fuer jeden Nutzer freigegeben.",
    ),
    (
        "kopie_abweichend",
        "pruef.apparmor_kopie_abweichend: Die bwrap-Kopie des Produkts weicht vom System-bwrap ab.",
    ),
    (
        "nicht_pruefbar",
        "pruef.apparmor_nicht_pruefbar: Das AppArmor-Profil war nicht pruefbar.",
    ),
];

/// The state of the user folder under `/var/lib/werkbank` and the sentence for it.
pub const NUTZERORDNER: &[(&str, &str)] = &[
    (
        "fehlt",
        "pruef.nutzerordner_fehlt: Der Ordner des Nutzers unter /var/lib/werkbank fehlt.",
    ),
    (
        "rechte",
        "pruef.nutzerordner_rechte: Der Ordner des Nutzers hat falsche Rechte oder einen falschen Eigentuemer.",
    ),
    (
        "nicht_pruefbar",
        "pruef.nutzerordner_nicht_pruefbar: Der Ordner des Nutzers war nicht pruefbar.",
    ),
];

/// The state of the key file and the sentence for it. Only kind, owner, mode and emptiness are ever read.
pub const SCHLUESSEL: &[(&str, &str)] = &[
    (
        "fehlt",
        "pruef.schluessel_fehlt: Die Schluesseldatei fehlt.",
    ),
    (
        "art",
        "pruef.schluessel_art: Die Schluesseldatei ist keine gewoehnliche Datei.",
    ),
    (
        "eigentuemer",
        "pruef.schluessel_eigentuemer: Die Schluesseldatei gehoert nicht dem Nutzer.",
    ),
    (
        "modus",
        "pruef.schluessel_modus: Die Schluesseldatei hat nicht den Modus 0600.",
    ),
    (
        "leer",
        "pruef.schluessel_leer: Die Schluesseldatei ist leer.",
    ),
    (
        "nicht_pruefbar",
        "pruef.schluessel_nicht_pruefbar: Die Schluesseldatei war nicht pruefbar.",
    ),
];

/// The state of the sign-in file of the route `copilot` and the sentence for it. Only kind, owner, mode, emptiness and
/// form are ever read, never the content, the length or the path.
pub const COPILOT: &[(&str, &str)] = &[
    (
        "fehlt",
        "pruef.copilot_fehlt: Die Anmeldedatei der Copilot CLI fehlt.",
    ),
    (
        "art",
        "pruef.copilot_art: Die Anmeldedatei der Copilot CLI ist keine gewoehnliche Datei.",
    ),
    (
        "eigentuemer",
        "pruef.copilot_eigentuemer: Die Anmeldedatei der Copilot CLI gehoert nicht dem Nutzer.",
    ),
    (
        "modus",
        "pruef.copilot_modus: Die Anmeldedatei der Copilot CLI hat nicht den Modus 0600.",
    ),
    (
        "leer",
        "pruef.copilot_leer: Die Anmeldedatei der Copilot CLI ist leer.",
    ),
    (
        "form",
        "pruef.copilot_form: Die Anmeldedatei der Copilot CLI hat falsche Form.",
    ),
    (
        "nicht_pruefbar",
        "pruef.copilot_nicht_pruefbar: Die Anmeldedatei der Copilot CLI war nicht pruefbar.",
    ),
];

/// The state of the vault folder and the sentence for it.
pub const VAULT: &[(&str, &str)] = &[
    ("fehlt", "pruef.vault_fehlt: Der Vault-Ordner fehlt."),
    (
        "kein_ordner",
        "pruef.vault_kein_ordner: Der Vault-Pfad ist kein Ordner.",
    ),
    (
        "rechte",
        "pruef.vault_rechte: Der Vault-Ordner hat falsche Rechte.",
    ),
    (
        "nicht_pruefbar",
        "pruef.vault_nicht_pruefbar: Der Vault-Ordner war nicht pruefbar.",
    ),
];

/// The state of the manifest inside the package state and the sentence for it.
pub const MANIFEST: &[(&str, &str)] = &[
    (
        "fehlt",
        "pruef.paketstand_manifest_fehlt: Das Manifest des Pakets fehlt.",
    ),
    (
        "ungueltig",
        "pruef.paketstand_manifest_ungueltig: Das Manifest des Pakets ist ungueltig.",
    ),
    (
        "nicht_pruefbar",
        "pruef.paketstand_nicht_pruefbar: Der Paketstand war nicht pruefbar.",
    ),
];

/// The state of the program folders of `lauf.json` and the sentence for it. `verboten`: a folder touches a login store of the account or
/// holds a secret segment, the rule of the enclosure (`einschluss::verbote`); the enclosure then refuses every session.
pub const PROGRAMMORDNER: &[(&str, &str)] = &[(
    "verboten",
    "pruef.programmordner_verboten: Ein Programmordner in lauf.json beruehrt den Anmeldespeicher oder ein Geheimnis; der Einschluss lehnt jede Sitzung ab (5 verboten).",
)];

/// The sentence for a harness that the package did not enter because a program folder of it touches a login store or holds a secret segment
/// (`{n}` is the harness, `{o}` the folder; the folder stands last, so a cut at the line limit takes only the end of the path). The folder
/// comes from the state file of the package (`/var/lib/werkbank/programmordner-abgelehnt`, written by `postinst`), not from a second search.
pub const ABGELEHNT_SATZ: &str = "pruef.programmordner_verboten: Harness {n} nicht eingetragen. Abhilfe: Harness unter einem anderen Praefix installieren, dann dpkg-reconfigure werkbank. Ordner: {o}";

/// The harnesses the search looks for, in the order of the output.
pub const HARNESS: [&str; 2] = ["copilot", "pi"];

/// The state of a harness that `lauf.json` does not list and the note for it (`{n}` is the name). No finding: a machine may have only one of them.
/// The places of the search are the fixed ones of the contract ("Suche der Harness"); `~` stands for the home of the user.
pub const HARNESS_HINWEIS: &[(&str, &str)] = &[
    (
        "nicht_gefunden",
        "pruef.hinweis_harness_nicht_gefunden: {n} nicht gefunden in ~/.nvm/versions/node/v*/bin, ~/.local/bin, ~/.npm-global/bin, ~/.volta/bin, /usr/local/bin, /usr/bin; andere Orte werden nicht gesucht.",
    ),
    (
        "ausserhalb",
        "pruef.hinweis_harness_ausserhalb: {n} liegt an einem gesuchten Ort, der echte Pfad aber nicht unter /usr, /opt, ~/.nvm, ~/.local, ~/.npm-global oder ~/.volta; er wird nicht genommen.",
    ),
    (
        "nicht_eingetragen",
        "pruef.hinweis_harness_nicht_eingetragen: {n} liegt an einem gesuchten Ort, steht aber nicht in lauf.json; sudo dpkg-reconfigure werkbank.",
    ),
];

/// The settings file: its state is `ok`, `fehlt`, `ungueltig` (with a code) or `nicht_pruefbar`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Einstellungen {
    /// `ok`, `fehlt`, `ungueltig` or `nicht_pruefbar`.
    pub zustand: String,
    /// The `einst.*` code, only with `ungueltig`.
    pub code: Option<String>,
}

/// How a file of the package differs from the manifest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Abweichung {
    /// A listed file is missing.
    Fehlt,
    /// A listed file has another size or hash.
    Abweichend,
    /// A file under the package folders is not listed.
    Fremd,
}

impl Abweichung {
    /// The name in the case files.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Fehlt => "fehlt",
            Self::Abweichend => "abweichend",
            Self::Fremd => "fremd",
        }
    }

    /// The sentence for `pfad`.
    #[must_use]
    pub fn satz(self, pfad: &str) -> String {
        match self {
            Self::Fehlt => format!("pruef.paketstand_datei_fehlt: Die Datei {pfad} fehlt."),
            Self::Abweichend => {
                format!(
                    "pruef.paketstand_datei_abweichend: Die Datei {pfad} weicht vom Manifest ab."
                )
            }
            Self::Fremd => {
                format!("pruef.paketstand_datei_fremd: Die Datei {pfad} steht nicht im Manifest.")
            }
        }
    }
}

/// The package state: manifest, kind of build, version and the files that differ.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Paketstand {
    /// `ok`, `fehlt`, `ungueltig` or `nicht_pruefbar`.
    pub manifest: String,
    /// `true` when the manifest says `bau: test`.
    pub testbau: bool,
    /// `true` when the installed version differs from the manifest.
    pub version_abweichend: bool,
    /// The files that differ, as `(path, kind)`.
    pub dateien: Vec<(String, Abweichung)>,
}

/// Everything the seven checks found, plus the two inputs that are no checks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lage {
    /// Number of human local accounts (at least 1).
    pub konten: u32,
    /// The state of each unit, by name; a unit not named here is `ok`.
    pub dienste: Vec<(String, String)>,
    /// State of the `AppArmor` profile.
    pub apparmor: String,
    /// The settings file.
    pub einstellungen: Einstellungen,
    /// State of the user folder.
    pub nutzerordner: String,
    /// State of the key file.
    pub schluessel: String,
    /// State of the sign-in file of the route `copilot`.
    pub copilot: String,
    /// `gesperrt` or `ubuntu`: the system `bwrap` is open only through the profile Ubuntu ships (a note, not a finding).
    pub system_bwrap: String,
    /// `""` (not shown), `geladen` or `entladen`: Ubuntu's own profile `bwrap-userns-restrict` is loaded or not (a note, not a finding).
    pub ubuntu_profil: String,
    /// State of the vault folder.
    pub vault: String,
    /// `""` (no line), `zugelassen`, `offen`, `abgelehnt`, `unbekannt` or `doppelt`: the approval of the local CA, as `lauf.json` holds it (a note,
    /// not a finding: `offen` and `abgelehnt` close the cloud route and are no error of the installation).
    pub ca_zulassung: String,
    /// `ok` (also when absent) or `verboten`: a program folder of `lauf.json` that the enclosure refuses (a finding).
    pub programmordner: String,
    /// The harnesses that `postinst` did not enter because of a program folder, as `(harness, folder)` in the order of [`HARNESS`] (a finding each;
    /// the file `programmordner-abgelehnt` of the package state, optional in the `lage`: absent gives none).
    pub abgelehnt: Vec<(String, String)>,
    /// The harnesses that `lauf.json` does not list, as `(name, state)` with a state of [`HARNESS_HINWEIS`] (notes, no findings).
    pub harness: Vec<(String, String)>,
    /// The package state.
    pub paketstand: Paketstand,
    /// `true` when the decision point of the machine is missing.
    pub stelle_fehlt: bool,
    /// `true` when a user without root called the command: a check the user cannot read stays `nicht_pruefbar` and the note
    /// `pruef.hinweis_sudo` names the way out.
    pub ohne_root: bool,
}

impl Lage {
    /// `true` when any of the eight checks could not be done (state `nicht_pruefbar`).
    #[must_use]
    pub fn nicht_pruefbar(&self) -> bool {
        let np = "nicht_pruefbar";
        self.dienste.iter().any(|(_, z)| z == np)
            || [
                &self.apparmor,
                &self.einstellungen.zustand,
                &self.nutzerordner,
                &self.schluessel,
                &self.copilot,
                &self.vault,
                &self.paketstand.manifest,
            ]
            .iter()
            .any(|z| z.as_str() == np)
    }
}

fn zustand_pruefen(tabelle: &[(&str, &str)], name: &str, was: &str) -> Result<String, String> {
    if name == OK || tabelle.iter().any(|(k, _)| *k == name) {
        Ok(name.to_owned())
    } else {
        Err(format!("{was}: unknown state {name:?}"))
    }
}

/// The optional field `system_bwrap`: `gesperrt` when absent, else `gesperrt` or `ubuntu`.
fn system_bwrap_aus(wert: &Value) -> Result<String, String> {
    match wert.get("system_bwrap").and_then(Value::as_str) {
        None | Some("gesperrt") => Ok("gesperrt".to_owned()),
        Some("ubuntu") => Ok("ubuntu".to_owned()),
        Some(andere) => Err(format!("unknown system_bwrap {andere:?}")),
    }
}

/// The optional field `ubuntu_profil`: `""` when absent, else `geladen` or `entladen`.
fn ubuntu_profil_aus(wert: &Value) -> Result<String, String> {
    match wert.get("ubuntu_profil").and_then(Value::as_str) {
        None => Ok(String::new()),
        Some(z @ ("geladen" | "entladen")) => Ok(z.to_owned()),
        Some(andere) => Err(format!("unknown ubuntu_profil {andere:?}")),
    }
}

/// The optional field `ca_zulassung`: `""` when absent, else `zugelassen`, `offen`, `abgelehnt` or `unbekannt`.
fn ca_zulassung_aus(wert: &Value) -> Result<String, String> {
    match wert.get("ca_zulassung") {
        None => Ok(String::new()),
        Some(Value::String(z))
            if ["zugelassen", "offen", "abgelehnt", "unbekannt", "doppelt"]
                .contains(&z.as_str()) =>
        {
            Ok(z.clone())
        }
        Some(andere) => Err(format!("unknown ca_zulassung {andere}")),
    }
}

/// The optional field `programmordner`: `ok` when absent, else `ok` or `verboten`.
fn programmordner_aus(wert: &Value) -> Result<String, String> {
    match wert.get("programmordner") {
        None => Ok("ok".to_owned()),
        Some(Value::String(z)) if z == "ok" || z == "verboten" => Ok(z.clone()),
        Some(andere) => Err(format!("unknown programmordner {andere}")),
    }
}

/// A folder as the state file and the `lage` carry it: absolute, printable ASCII without a blank, at most 1024 bytes.
#[must_use]
pub fn ordner_form(o: &str) -> bool {
    o.starts_with('/') && o.len() <= 1024 && o.bytes().all(|b| b.is_ascii_graphic())
}

/// The optional field `programmordner_abgelehnt`: a list of `{harness, ordner}` with a harness of [`HARNESS`] (once each) and a folder of [`ordner_form`].
fn abgelehnt_aus(wert: &Value) -> Result<Vec<(String, String)>, String> {
    let Some(l) = wert.get("programmordner_abgelehnt") else {
        return Ok(Vec::new());
    };
    let l = l
        .as_array()
        .ok_or("programmordner_abgelehnt is not a list")?;
    let mut aus: Vec<(String, String)> = Vec::new();
    for e in l {
        let o = e
            .as_object()
            .filter(|o| o.len() == 2)
            .ok_or("programmordner_abgelehnt: entry is not {harness, ordner}")?;
        let h = o
            .get("harness")
            .and_then(Value::as_str)
            .filter(|h| HARNESS.contains(h))
            .ok_or("programmordner_abgelehnt: unknown harness")?;
        let d = o
            .get("ordner")
            .and_then(Value::as_str)
            .filter(|d| ordner_form(d))
            .ok_or("programmordner_abgelehnt: bad folder")?;
        if aus.iter().any(|(n, _)| n == h) {
            return Err("programmordner_abgelehnt: harness twice".to_owned());
        }
        aus.push((h.to_owned(), d.to_owned()));
    }
    aus.sort_by_key(|(n, _)| HARNESS.iter().position(|k| k == n));
    Ok(aus)
}

/// The optional field `harness`: an object `{copilot|pi: state}`; absent gives no note.
fn harness_aus(wert: &Value) -> Result<Vec<(String, String)>, String> {
    let Some(o) = wert.get("harness") else {
        return Ok(Vec::new());
    };
    let o = o.as_object().ok_or("harness is not an object")?;
    let mut aus = Vec::new();
    for name in HARNESS {
        match o.get(name).map(Value::as_str) {
            None => {}
            Some(Some(z)) if HARNESS_HINWEIS.iter().any(|(k, _)| *k == z) => {
                aus.push((name.to_owned(), z.to_owned()));
            }
            Some(_) => return Err(format!("unknown state of harness {name}")),
        }
    }
    if o.keys().any(|k| !HARNESS.contains(&k.as_str())) {
        return Err("unknown harness".to_owned());
    }
    Ok(aus)
}

/// The optional field `ohne_root`: `false` when absent, else a boolean.
fn ohne_root_aus(wert: &Value) -> Result<bool, String> {
    match wert.get("ohne_root") {
        None => Ok(false),
        Some(Value::Bool(b)) => Ok(*b),
        Some(_) => Err("ohne_root is not a boolean".to_owned()),
    }
}

fn text<'a>(wert: &'a Value, schluessel: &str) -> Result<&'a str, String> {
    wert.get(schluessel)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("field {schluessel:?} missing or not a text"))
}

fn code_form(code: &str) -> bool {
    code.strip_prefix("einst.").is_some_and(|rest| {
        !rest.is_empty()
            && rest.len() <= 64
            && rest.bytes().all(|b| b.is_ascii_lowercase() || b == b'_')
    })
}

impl Lage {
    /// Reads the state of a case file (`lage` of the form `pruefung`, platform `linux`).
    ///
    /// # Errors
    /// A text that says what is wrong when the value does not have the form of a Linux state.
    pub fn aus_json(wert: &Value) -> Result<Self, String> {
        if text(wert, "plattform")? != "linux" {
            return Err("platform is not linux".to_owned());
        }
        let konten = wert
            .get("konten")
            .and_then(Value::as_u64)
            .and_then(|n| u32::try_from(n).ok())
            .ok_or("konten missing or not a number")?;
        let mut dienste = Vec::new();
        for d in wert
            .get("dienste")
            .and_then(Value::as_array)
            .ok_or("dienste missing")?
        {
            let name = text(d, "name")?;
            if !DIENSTE.contains(&name) {
                return Err(format!("unknown unit {name:?}"));
            }
            dienste.push((
                name.to_owned(),
                zustand_pruefen(DIENST, text(d, "zustand")?, "dienste")?,
            ));
        }
        let e = wert.get("einstellungen").ok_or("einstellungen missing")?;
        let ez = text(e, "zustand")?;
        let code = e.get("code").and_then(Value::as_str).map(str::to_owned);
        if !["ok", "fehlt", "ungueltig", "nicht_pruefbar"].contains(&ez) {
            return Err(format!("einstellungen: unknown state {ez:?}"));
        }
        if (ez == "ungueltig") != code.is_some() || code.as_deref().is_some_and(|c| !code_form(c)) {
            return Err(
                "einstellungen: code only with ungueltig, in the form einst.<name>".to_owned(),
            );
        }
        let p = wert.get("paketstand").ok_or("paketstand missing")?;
        let mut dateien = Vec::new();
        for d in p
            .get("dateien")
            .and_then(Value::as_array)
            .ok_or("paketstand.dateien missing")?
        {
            let art = match text(d, "art")? {
                "fehlt" => Abweichung::Fehlt,
                "abweichend" => Abweichung::Abweichend,
                "fremd" => Abweichung::Fremd,
                andere => return Err(format!("unknown file kind {andere:?}")),
            };
            let pfad = text(d, "pfad")?;
            if pfad.is_empty() || !pfad.is_ascii() {
                return Err("file path empty or not ASCII".to_owned());
            }
            dateien.push((pfad.to_owned(), art));
        }
        let manifest = text(p, "manifest")?;
        if manifest != OK && !MANIFEST.iter().any(|(k, _)| *k == manifest) {
            return Err(format!("paketstand.manifest: unknown state {manifest:?}"));
        }
        Ok(Self {
            konten,
            dienste,
            apparmor: zustand_pruefen(APPARMOR, text(wert, "apparmor")?, "apparmor")?,
            einstellungen: Einstellungen {
                zustand: ez.to_owned(),
                code,
            },
            nutzerordner: zustand_pruefen(
                NUTZERORDNER,
                text(wert, "nutzerordner")?,
                "nutzerordner",
            )?,
            schluessel: zustand_pruefen(SCHLUESSEL, text(wert, "schluessel")?, "schluessel")?,
            copilot: zustand_pruefen(COPILOT, text(wert, "copilot")?, "copilot")?,
            system_bwrap: system_bwrap_aus(wert)?,
            ubuntu_profil: ubuntu_profil_aus(wert)?,
            vault: zustand_pruefen(VAULT, text(wert, "vault")?, "vault")?,
            ca_zulassung: ca_zulassung_aus(wert)?,
            programmordner: programmordner_aus(wert)?,
            abgelehnt: abgelehnt_aus(wert)?,
            harness: harness_aus(wert)?,
            paketstand: Paketstand {
                manifest: manifest.to_owned(),
                testbau: match text(p, "bau")? {
                    "test" => true,
                    "auslieferung" => false,
                    andere => return Err(format!("unknown bau {andere:?}")),
                },
                version_abweichend: match text(p, "version")? {
                    "abweichend" => true,
                    "gleich" => false,
                    andere => return Err(format!("unknown version state {andere:?}")),
                },
                dateien,
            },
            stelle_fehlt: match text(wert, "stelle")? {
                "fehlt" => true,
                "ok" => false,
                andere => return Err(format!("unknown stelle {andere:?}")),
            },
            ohne_root: ohne_root_aus(wert)?,
        })
    }
}
