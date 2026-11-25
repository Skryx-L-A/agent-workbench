//! W7 (TR16 point 2): the title of the prelude of `ConPTY`. The logic is the same on every system and is tested under Linux; only Windows
//! switches it on.
//!
//! Before the first byte of the child, `ConPTY` writes `ESC[6n ESC[?9001h ESC[?1004h ESC[m ESC]0;<path of the program file> BEL ESC[?25h`
//! (measurement plan, section 13 point 4 and section 15 W7). The title in it is no title of the program. So the carrier reports no `Titel`
//! for it and does not change the session title. The other sequences of the prelude are treated like any other sequence (TR5, TR10, TR14).
//!
//! The prelude is told apart by two things together: the title is the path of the program file that was started, and it comes before
//! anything else was reported (no row with content, no other event). Only the first title of a session can be the prelude; a title the
//! program writes itself stays an event, also when it reads the same.
//!
//! When the size of the pseudo console changes before the prelude, `ConPTY` repaints first: it hides the cursor, clears every row and
//! goes home, then writes the title (measured 25.11.2026, contract `dienst` section 21c). That gives an `Ausgabe` with the cursor alone;
//! it does not close the prelude, because no row has content yet (`Raster::ausgabe_melden`).

/// Whether the next title may still be the prelude of `ConPTY`, and which program it names.
#[derive(Debug, Default)]
pub(crate) struct Vorspann {
    /// The program that was started; `None` where there is no prelude (Linux, macOS).
    programm: Option<String>,
    /// `true` until the first title (or any other event) has come.
    offen: bool,
}

impl Vorspann {
    /// A session that expects the prelude of `ConPTY` with a title that names `programm`.
    pub(crate) fn erwartet(programm: &str) -> Self {
        Self {
            programm: Some(programm.to_owned()),
            offen: true,
        }
    }

    /// `true` if this title is the prelude: it is the first title, nothing else was reported before it (`schon_gemeldet` is
    /// `false`) and it names the program. The first title closes the question, whatever the answer.
    pub(crate) fn verwirft(&mut self, titel: &str, schon_gemeldet: bool) -> bool {
        let offen = std::mem::replace(&mut self.offen, false);
        if !offen || schon_gemeldet {
            return false;
        }
        self.programm
            .as_deref()
            .is_some_and(|programm| nennt(titel, programm))
    }
}

/// The form in which `ConPTY` and the caller may write the same path differently: case, `/` or `\`, the `\\?\` prefix.
fn normal(pfad: &str) -> String {
    let klein = pfad.replace('/', "\\").to_lowercase();
    klein.strip_prefix("\\\\?\\").unwrap_or(&klein).to_owned()
}

/// The last part of a path in the normal form, without `.exe`.
fn dateiname(pfad: &str) -> &str {
    let name = pfad.rsplit('\\').next().unwrap_or(pfad);
    name.strip_suffix(".exe").unwrap_or(name)
}

/// `true` if `titel` is the path of the program file `programm`. `programm` is what the caller started with: a full path (it must
/// equal the title) or a bare name that the system searched (the title is the path found; the file name must equal it).
fn nennt(titel: &str, programm: &str) -> bool {
    let (titel, programm) = (normal(titel), normal(programm));
    if titel == programm {
        return true;
    }
    !programm.contains('\\') && !programm.is_empty() && dateiname(&titel) == dateiname(&programm)
}

#[cfg(test)]
mod tests {
    use super::*;

    const PFAD: &str = r"C:\Work\tr.exe";

    #[test]
    fn der_erste_titel_mit_dem_pfad_des_programms_vor_allem_anderen_ist_der_vorspann() {
        let mut v = Vorspann::erwartet(PFAD);
        assert!(v.verwirft(PFAD, false));
    }

    #[test]
    fn ein_anderer_titel_ist_nicht_der_vorspann_und_schliesst_die_frage() {
        let mut v = Vorspann::erwartet(PFAD);
        assert!(!v.verwirft("mein Titel", false), "does not name the path");
        assert!(
            !v.verwirft(PFAD, false),
            "only the first title can be the prelude"
        );
    }

    #[test]
    fn nach_dem_ersten_anderen_ereignis_bleibt_auch_der_gleiche_titel_ein_ereignis() {
        let mut v = Vorspann::erwartet(PFAD);
        assert!(!v.verwirft(PFAD, true));
    }

    #[test]
    fn nur_der_erste_titel_wird_verworfen_der_gleiche_noch_einmal_nicht() {
        let mut v = Vorspann::erwartet(PFAD);
        assert!(v.verwirft(PFAD, false));
        assert!(!v.verwirft(PFAD, false));
    }

    #[test]
    fn ohne_vorspann_unter_linux_und_macos_wird_nie_verworfen() {
        let mut v = Vorspann::default();
        assert!(!v.verwirft(PFAD, false));
        assert!(!Vorspann::default().verwirft("/bin/sh", false));
    }

    #[test]
    fn der_pfad_wird_geprueft_ein_anderer_pfad_ist_kein_vorspann() {
        for titel in [r"C:\Other\tr.exe", r"C:\Work\other.exe", "", "tr.exe;x"] {
            assert!(!Vorspann::erwartet(PFAD).verwirft(titel, false), "{titel}");
        }
    }

    #[test]
    fn schreibweise_von_pfad_und_name_wird_toleriert_der_pfad_aber_nicht_erraten() {
        assert!(Vorspann::erwartet(PFAD).verwirft(r"c:\work\TR.EXE", false));
        assert!(Vorspann::erwartet(PFAD).verwirft("C:/Work/tr.exe", false));
        assert!(Vorspann::erwartet(PFAD).verwirft(r"\\?\C:\Work\tr.exe", false));
        // a bare name: the system searched it, the title is the path found
        assert!(Vorspann::erwartet("tr").verwirft(r"C:\Anywhere\tr.exe", false));
        assert!(Vorspann::erwartet("tr.exe").verwirft(r"C:\Anywhere\tr.exe", false));
        assert!(!Vorspann::erwartet("tr").verwirft(r"C:\Anywhere\trx.exe", false));
        // a full path must match in full
        assert!(!Vorspann::erwartet(PFAD).verwirft(r"D:\Work\tr.exe", false));
    }
}
