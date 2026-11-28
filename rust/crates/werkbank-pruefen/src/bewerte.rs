//! From a [`Lage`] to the output: lines, stream and exit code (contract: "Ausgabe und Codes", "Exitcodes").
//!
//! The order of the checks in the output is fixed: units, `AppArmor`, settings, user folder, key, vault, package state.
//! The command names the code and the sentence and nothing more: no path outside the manifest, no content.

use crate::lage::{
    ABGELEHNT_SATZ, APPARMOR, Abweichung, COPILOT, COPILOT_ALTER_ORT, DIENST, DIENSTE,
    HALTER_KETTE, HARNESS_HINWEIS, Lage, MANIFEST, NEUSTART, NUTZERORDNER, OK, PROGRAMMORDNER,
    ROLLENGRUPPE, ROLLENPROGRAMM, SCHLUESSEL, VAULT,
};

/// The stream a result goes to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Strom {
    /// Standard output.
    Stdout,
    /// Standard error.
    Stderr,
}

/// What the command prints and returns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ergebnis {
    /// Exit code: 0, 1, 20, 64 or 70.
    pub rc: u8,
    /// The stream of all lines.
    pub strom: Strom,
    /// The lines, without line end.
    pub zeilen: Vec<String>,
}

/// The number of checks on Ubuntu.
pub const PRUEFUNGEN: u32 = 8;

/// At most this many lines name a file of the package state; more are counted in one line.
pub const MAX_DATEIZEILEN: usize = 20;

/// The longest line in bytes.
pub const MAX_ZEILE: usize = 200;

/// The sentence of the line `pruef.aufruf` and of `pruef.intern`.
pub const AUFRUF: &str = "pruef.aufruf: Der Pruefbefehl kennt dieses Argument nicht.";
/// The sentence of the call `--vor-entfernen` when a key file is still there (after `apt remove`).
pub const SCHLUESSEL_REST: &str =
    "pruef.schluessel_rest: Eine Schluesseldatei liegt noch da; apt purge loescht sie.";
/// The sentence of the call `--vor-entfernen` when a sign-in file of the Copilot CLI is still there.
pub const COPILOT_REST: &str =
    "pruef.copilot_rest: Eine Anmeldedatei der Copilot CLI liegt noch da; apt purge loescht sie.";
/// The note when the system `bwrap` is open only through the profile that Ubuntu ships with `bubblewrap`. It is no finding:
/// it neither turns the result red nor counts in `pruef.rot`.
pub const HINWEIS_SYSTEM_BWRAP_UBUNTU: &str = "pruef.hinweis_system_bwrap_ubuntu: Das System-bwrap ist durch das Ubuntu-Profil frei; die Firma kann dieses Profil entfernen.";
/// The note on the state of Ubuntu's own profile `bwrap-userns-restrict` while the lock on user namespaces is on: one line in
/// both states, so the IT of the company sees it. No finding: no effect on the result or on `pruef.rot`.
pub const HINWEIS_UBUNTU_PROFIL_GELADEN: &str =
    "pruef.hinweis_ubuntu_profil_geladen: Das Ubuntu-Profil bwrap-userns-restrict ist geladen.";
/// See [`HINWEIS_UBUNTU_PROFIL_GELADEN`].
pub const HINWEIS_UBUNTU_PROFIL_ENTLADEN: &str = "pruef.hinweis_ubuntu_profil_entladen: Das Ubuntu-Profil bwrap-userns-restrict ist nicht geladen.";
/// The notes on the approval of the local CA (`ca_zulassung` of `lauf.json`). No finding: `offen` and `abgelehnt` close the cloud route
/// and are no error of the installation, so they neither turn the result red nor count in `pruef.rot`.
pub const HINWEIS_CA_ZUGELASSEN: &str = "pruef.hinweis_ca_zugelassen: Die Zulassung der lokalen CA ist erteilt; die Cloud-Route ist nicht gesperrt.";
/// See [`HINWEIS_CA_ZUGELASSEN`].
pub const HINWEIS_CA_OFFEN: &str = "pruef.hinweis_ca_offen: Die Zulassung der lokalen CA ist offen; die Cloud-Route ist gesperrt. Das ist kein Fehler der Installation.";
/// See [`HINWEIS_CA_ZUGELASSEN`].
pub const HINWEIS_CA_ABGELEHNT: &str = "pruef.hinweis_ca_abgelehnt: Die Zulassung der lokalen CA ist abgelehnt; die Cloud-Route ist gesperrt.";
/// `lauf.json` holds the key `ca_zulassung` more than once: the package reads this as `offen` (contract "Zulassung der CA"), the cloud route is closed.
pub const HINWEIS_CA_DOPPELT: &str = "pruef.hinweis_ca_doppelt: lauf.json nennt ca_zulassung mehr als einmal; das gilt als offen, die Cloud-Route ist gesperrt.";
/// The file is there but unreadable or without one of the three values: the cloud route counts as closed.
pub const HINWEIS_CA_UNBEKANNT: &str = "pruef.hinweis_ca_unbekannt: Die Zulassung der lokalen CA war nicht lesbar; die Cloud-Route ist gesperrt.";
/// The note for a call without root while a check was `nicht_pruefbar`: no finding, no effect on `pruef.rot` or the exit code.
pub const HINWEIS_SUDO: &str = "pruef.hinweis_sudo: Ohne root waren nicht alle Pruefungen moeglich; fuer diese Pruefungen mit sudo aufrufen.";
/// The note with the remedy for one level writable for a group that is not the private group (addendum 1 to the decision
/// of the operator, 28.11.2026); `chmod g-w` and the path follow.
pub const HINWEIS_COPILOT_GRUPPE: &str =
    "pruef.hinweis_copilot_gruppe: Die Gruppe ist nicht als private Gruppe pruefbar. Abhilfe:";
/// The sentence of an internal error.
pub const INTERN: &str = "pruef.intern: Interner Fehler.";
/// The line of the decision point (A108).
pub const STELLE: &str =
    "pruef.stelle_einrichten: Installation fertig, Stelle einrichten. Befehl: wb stelle einrichten";

fn satz(tabelle: &[(&str, &str)], zustand: &str) -> Option<String> {
    if zustand == OK {
        return None;
    }
    tabelle
        .iter()
        .find(|(k, _)| *k == zustand)
        .map(|(_, s)| (*s).to_owned())
}

/// The result of a call with arguments.
#[must_use]
pub fn aufruf_falsch() -> Ergebnis {
    Ergebnis {
        rc: 64,
        strom: Strom::Stderr,
        zeilen: vec![AUFRUF.to_owned()],
    }
}

/// The result of `--vor-entfernen` (only `prerm` calls it): it checks nothing but the key file and never fails.
///
/// `zustand` is the state of the key check (`ok`, `fehlt` or any other state of the check): a key file in any state is
/// reported with the fixed sentence, none gives no line, and a check that was not possible gives its own sentence.
#[must_use]
pub fn vor_entfernen(schluessel: &str, copilot: &str) -> Ergebnis {
    let mut zeilen = Vec::new();
    for (zustand, tabelle, rest) in [
        (schluessel, SCHLUESSEL, SCHLUESSEL_REST),
        (copilot, COPILOT, COPILOT_REST),
    ] {
        match zustand {
            "nicht_pruefbar" => zeilen.extend(satz(tabelle, zustand)),
            "fehlt" => {}
            _ => zeilen.push(rest.to_owned()),
        }
    }
    Ergebnis {
        rc: 0,
        strom: Strom::Stdout,
        zeilen,
    }
}

/// The result of an internal error.
#[must_use]
pub fn intern() -> Ergebnis {
    Ergebnis {
        rc: 70,
        strom: Strom::Stderr,
        zeilen: vec![INTERN.to_owned()],
    }
}

/// Cuts a line to the longest form that is ASCII and at most [`MAX_ZEILE`] bytes.
#[must_use]
pub fn kuerzen(zeile: &str) -> String {
    zeile
        .chars()
        .map(|c| {
            if c.is_ascii_graphic() || c == ' ' {
                c
            } else {
                '?'
            }
        })
        .take(MAX_ZEILE)
        .collect()
}

/// All findings of `lage` in the order of the contract.
#[must_use]
pub fn befunde(lage: &Lage) -> Vec<String> {
    let mut b = Vec::new();
    for name in DIENSTE {
        let z = lage
            .dienste
            .iter()
            .find(|(n, _)| n == name)
            .map_or(OK, |(_, z)| z.as_str());
        if let Some(s) = satz(DIENST, z) {
            b.push(s.replace("{n}", name));
        }
    }
    // addendum "Rolle aus der Gruppe": part of the check of the units, the group before the program
    b.extend(satz(ROLLENGRUPPE, &lage.rollengruppe));
    b.extend(satz(ROLLENPROGRAMM, &lage.rollenprogramm));
    // the chain of the holder (decision of the operator, 28.11.2026), still part of the check of the units
    b.extend(satz(HALTER_KETTE, &lage.halter_kette));
    // restart needed (decision of the operator, 28.11.2026), after the chain, still part of the check of the units
    b.extend(satz(NEUSTART, &lage.neustart));
    b.extend(satz(APPARMOR, &lage.apparmor));
    match lage.einstellungen.zustand.as_str() {
        "fehlt" => b.push(
            "pruef.einstellungen_fehlt: Die Einstellungsdatei fehlt oder ist nicht lesbar."
                .to_owned(),
        ),
        "ungueltig" => b.push(format!(
            "pruef.einstellungen_ungueltig: Die Einstellungsdatei ist ungueltig ({}).",
            lage.einstellungen
                .code
                .as_deref()
                .unwrap_or("einst.unbekannt")
        )),
        "nicht_pruefbar" => b.push(
            "pruef.einstellungen_nicht_pruefbar: Die Einstellungsdatei war nicht pruefbar."
                .to_owned(),
        ),
        _ => {}
    }
    b.extend(satz(NUTZERORDNER, &lage.nutzerordner));
    b.extend(satz(PROGRAMMORDNER, &lage.programmordner));
    for (harness, ordner) in &lage.abgelehnt {
        b.push(
            ABGELEHNT_SATZ
                .replace("{n}", harness)
                .replace("{o}", ordner),
        );
    }
    b.extend(satz(SCHLUESSEL, &lage.schluessel));
    b.extend(satz(COPILOT, &lage.copilot));
    if lage.copilot_alt {
        b.push(COPILOT_ALTER_ORT.to_owned());
    }
    b.extend(satz(VAULT, &lage.vault));
    let p = &lage.paketstand;
    if p.manifest == OK {
        if p.testbau {
            b.push("pruef.paketstand_testbau: Das Paket ist ein Testbau.".to_owned());
        }
        if p.version_abweichend {
            b.push(
                "pruef.paketstand_version: Die installierte Version weicht vom Manifest ab."
                    .to_owned(),
            );
        }
        let mut dateien: Vec<&(String, Abweichung)> = p.dateien.iter().collect();
        dateien.sort_by(|x, y| (x.0.as_bytes(), x.1).cmp(&(y.0.as_bytes(), y.1)));
        for (pfad, art) in dateien.iter().take(MAX_DATEIZEILEN).copied() {
            b.push(art.satz(pfad));
        }
        if dateien.len() > MAX_DATEIZEILEN {
            b.push(format!(
                "pruef.paketstand_weitere: {} weitere Dateien weichen ab.",
                dateien.len() - MAX_DATEIZEILEN
            ));
        }
    } else {
        b.extend(satz(MANIFEST, &p.manifest));
    }
    b
}

/// The notes of `lage` (no findings): the remedy for each level of `copilot_gruppe`, the state of Ubuntu's profile, the note on the system `bwrap`, the approval of the CA, then the note on `sudo`.
fn hinweise(lage: &Lage) -> Vec<String> {
    let mut h = Vec::new();
    for pfad in &lage.copilot_gruppe {
        h.push(format!(
            "{HINWEIS_COPILOT_GRUPPE} {}",
            crate::gruppe::chmod_g_w(std::slice::from_ref(pfad))
        ));
    }
    match lage.ubuntu_profil.as_str() {
        "geladen" => h.push(HINWEIS_UBUNTU_PROFIL_GELADEN.to_owned()),
        "entladen" => h.push(HINWEIS_UBUNTU_PROFIL_ENTLADEN.to_owned()),
        _ => {}
    }
    if lage.system_bwrap == "ubuntu" {
        h.push(HINWEIS_SYSTEM_BWRAP_UBUNTU.to_owned());
    }
    match lage.ca_zulassung.as_str() {
        "zugelassen" => h.push(HINWEIS_CA_ZUGELASSEN.to_owned()),
        "offen" => h.push(HINWEIS_CA_OFFEN.to_owned()),
        "abgelehnt" => h.push(HINWEIS_CA_ABGELEHNT.to_owned()),
        "unbekannt" => h.push(HINWEIS_CA_UNBEKANNT.to_owned()),
        "doppelt" => h.push(HINWEIS_CA_DOPPELT.to_owned()),
        _ => {}
    }
    for (name, zustand) in &lage.harness {
        // A harness that the package refused has its own finding; the note "not entered" would say the same twice.
        if lage.abgelehnt.iter().any(|(n, _)| n == name) {
            continue;
        }
        h.extend(satz(HARNESS_HINWEIS, zustand).map(|t| t.replace("{n}", name)));
    }
    if lage.ohne_root && lage.nicht_pruefbar() {
        h.push(HINWEIS_SUDO.to_owned());
    }
    h
}

/// The output of a call without arguments for `lage`.
#[must_use]
pub fn bewerte(lage: &Lage) -> Ergebnis {
    let mut zeilen = befunde(lage);
    let hinweise = hinweise(lage);
    let (rc, zeilen) = if zeilen.is_empty() {
        let mut z = hinweise;
        if lage.stelle_fehlt && lage.konten >= 2 {
            z.push(STELLE.to_owned());
            (20, z)
        } else {
            z.push(format!("pruef.ok: Alle {PRUEFUNGEN} Pruefungen bestanden."));
            (0, z)
        }
    } else {
        let n = zeilen.len();
        zeilen.extend(hinweise);
        zeilen.push(format!("pruef.rot: {n} Befunde."));
        (1, zeilen)
    };
    Ergebnis {
        rc,
        strom: Strom::Stdout,
        zeilen: zeilen.iter().map(|z| kuerzen(z)).collect(),
    }
}
