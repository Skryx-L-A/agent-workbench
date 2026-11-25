//! The rule of the Windows code of the crate (project decision, 25.11.2026, contract `dienst`, section 21): Windows code stands only in
//! `src/sys/windows.rs`, which the mutation gate leaves out because Linux does not compile it. Every other module may name Windows
//! behind `cfg` only in a fixed short form: a line that imports from `sys`, or one call of `sys` without logic of its own. This test
//! reads the sources and reports every place that breaks the rule. It runs under every system, because it only reads text.
//!
//! What counts as a place: an attribute `#[cfg(..)]`, `#![cfg(..)]` or `#[cfg_attr(..)]`, or the macro `cfg!(..)`, whose condition names
//! `windows`. Not a place: a condition with `unix` next to `windows` (both systems alike, as `cfg(any(unix, windows))`), a negation
//! (`not(..)`), and a `cfg_attr` that only sets a lint (`allow(..)`). A test module for Windows counts as a place: `cargo-mutants` does
//! not skip `cfg(all(test, windows, ..))` and mutates its helper functions (measured: 14 mutants of the former module `tests_windows`
//! survived the list under Linux), so its cases stand in `src/sys/windows.rs` too.
//!
//! The short forms (the line or lines right behind the attribute; further attributes and comments between are skipped):
//! 1. `use crate::sys::..;`, `use sys::..;` with `pub` or `pub(crate)`: one line.
//! 2. One call without logic: an optional `return `, `let <name> = ` (also `mut`, or a tuple of names) or `<field>: ` in front, then
//!    `crate::sys::<name>` (in `lib.rs` `sys::<name>`) with arguments that are names, field paths and `&` or `&mut` of them, then an
//!    optional `?`, then an optional `;` or `,`.
//! 3. A block `{`, one call as in 2, `}`.
//! 4. `let Some(<name>) = crate::sys::<call> else {`, `return false;` (or `true`), `};`.
//! 5. `if let Some(<name>) = crate::sys::<call> {`, `return <name>;`, `}`.
//! 6. A function whose body is one call as in 2 without prefix.
//! 7. A field `<name>: crate::sys::<Type>,` and its initial value `<name>,`.
//!
//! Anything else (a comparison, a branch, a loop, a closure, a calculation, a second call) is a finding. The files `src/sys/*.rs` other
//! than `windows.rs` may name Windows behind `cfg` only for `mod windows;` and for `use` lines. The two modules that `lib.rs` swaps
//! for a Windows file (`loeschen_windows.rs`, `rechte_windows.rs`) are the only whole files of Windows code outside `src/sys/`: the
//! mutation gate does not read them either, because `cargo-mutants` does not evaluate `cfg_attr(windows, path = ..)` and keeps the
//! Unix file for the module.

use std::fs;
use std::path::{Path, PathBuf};

/// The two Windows files outside `src/sys/` that `lib.rs` names with `cfg_attr(windows, path = ..)`.
const WINDOWS_DATEIEN: [&str; 2] = ["loeschen_windows.rs", "rechte_windows.rs"];

/// The expressions `cfg!(windows)` that stay, each as the file and a part of the line. The first is code that Linux compiles too, so the
/// mutation gate sees it and a case catches it (`socket_lesen`, point 10). The second is the case of the test module that checks it.
const AUSDRUECKE: [(&str, &str); 2] = [
    (
        "startdatei.rs",
        "|| (cfg!(windows) && name.contains(['\\\\', ':']))",
    ),
    ("startdatei.rs", "if cfg!(windows) {"),
];

/// All files below `ordner` that end in `.rs`, with the path relative to `wurzel` and the text.
fn quellen(wurzel: &Path, ordner: &Path, aus: &mut Vec<(String, String)>) {
    let mut eintraege: Vec<PathBuf> = fs::read_dir(ordner)
        .expect("source folder")
        .map(|e| e.expect("entry").path())
        .collect();
    eintraege.sort();
    for pfad in eintraege {
        if pfad.is_dir() {
            quellen(wurzel, &pfad, aus);
        } else if pfad.extension().is_some_and(|e| e == "rs") {
            let name = pfad
                .strip_prefix(wurzel)
                .expect("below the root")
                .to_string_lossy()
                .replace('\\', "/");
            aus.push((name, fs::read_to_string(&pfad).expect("source file")));
        }
    }
}

/// The sources of the crate `dienst`: `src/**/*.rs`, as `(path below src, text)`.
fn eigene_quellen() -> Vec<(String, String)> {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut aus = Vec::new();
    quellen(&src, &src, &mut aus);
    aus
}

/// `true` when `text` holds `wort` as a whole word.
fn hat_wort(text: &str, wort: &str) -> bool {
    text.match_indices(wort).any(|(i, _)| {
        let davor = text[..i].chars().next_back();
        let danach = text[i + wort.len()..].chars().next();
        let wortzeichen = |c: char| c.is_ascii_alphanumeric() || c == '_';
        !davor.is_some_and(wortzeichen) && !danach.is_some_and(wortzeichen)
    })
}

/// `true` for a name: letters, digits and `_`, not empty.
fn ist_name(text: &str) -> bool {
    !text.is_empty() && text.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// `true` for the arguments of a call: nothing, or names and field paths with `&` and `mut`, separated by `, `.
fn sind_argumente(text: &str) -> bool {
    text.is_empty()
        || text.split(", ").all(|a| {
            let a = a.strip_prefix('&').unwrap_or(a);
            let a = a.strip_prefix("mut ").unwrap_or(a);
            !a.is_empty() && a.split('.').all(ist_name)
        })
}

/// `true` for `crate::sys::<name>` with optional arguments in parentheses.
fn ist_sys_aufruf(text: &str) -> bool {
    let Some(rest) = text
        .strip_prefix("crate::sys::")
        .or_else(|| text.strip_prefix("sys::"))
    else {
        return false;
    };
    match rest.split_once('(') {
        Some((name, args)) => ist_name(name) && args.strip_suffix(')').is_some_and(sind_argumente),
        None => ist_name(rest),
    }
}

/// `true` for the target of a `let`: a name, `mut <name>` or a tuple of names.
fn ist_muster(text: &str) -> bool {
    let text = text.strip_prefix("mut ").unwrap_or(text);
    match text.strip_prefix('(').and_then(|t| t.strip_suffix(')')) {
        Some(innen) => innen.split(", ").all(ist_name),
        None => ist_name(text),
    }
}

/// Form 2: one call without logic, with an optional prefix and an optional `;` or `,` behind.
fn ist_kurzer_aufruf(zeile: &str) -> bool {
    let z = zeile.trim();
    let z = z
        .strip_suffix(';')
        .or_else(|| z.strip_suffix(','))
        .unwrap_or(z);
    let z = z.strip_suffix('?').unwrap_or(z);
    let z = if let Some(rest) = z.strip_prefix("return ") {
        rest
    } else if let Some(rest) = z.strip_prefix("let ") {
        match rest.split_once(" = ") {
            Some((muster, aufruf)) if ist_muster(muster) => aufruf,
            _ => return false,
        }
    } else if let Some((feld, aufruf)) = z.split_once(": ") {
        if ist_name(feld) { aufruf } else { return false }
    } else {
        z
    };
    ist_sys_aufruf(z)
}

/// Form 1: one `use` line from `sys`.
fn ist_use_aus_sys(zeile: &str) -> bool {
    let z = zeile.trim();
    let z = z
        .strip_prefix("pub(crate) use ")
        .or_else(|| z.strip_prefix("pub use "))
        .or_else(|| z.strip_prefix("use "));
    let Some(z) = z.and_then(|z| z.strip_suffix(';')) else {
        return false;
    };
    let Some(rest) = z
        .strip_prefix("crate::sys::")
        .or_else(|| z.strip_prefix("sys::"))
    else {
        return false;
    };
    // a name, `a as b`, or a list in braces
    let innen = rest
        .strip_prefix('{')
        .and_then(|r| r.strip_suffix('}'))
        .unwrap_or(rest);
    innen.split(", ").all(|teil| match teil.split_once(" as ") {
        Some((a, b)) => ist_name(a) && ist_name(b),
        None => ist_name(teil),
    })
}

/// The lines of the file with comments and blank lines left out, as `(line number, trimmed text)`.
fn codezeilen(text: &str) -> Vec<(usize, &str)> {
    text.lines()
        .enumerate()
        .map(|(i, z)| (i + 1, z.trim()))
        .filter(|(_, z)| !z.is_empty() && !z.starts_with("//"))
        .collect()
}

/// Whether the attribute line names Windows in a way that counts as a place (see the module text).
fn ist_windows_attribut(zeile: &str) -> bool {
    let Some(rest) = zeile
        .strip_prefix("#[cfg(")
        .or_else(|| zeile.strip_prefix("#![cfg("))
        .or_else(|| zeile.strip_prefix("#[cfg_attr("))
    else {
        return false;
    };
    if !hat_wort(rest, "windows") {
        return false;
    }
    if rest.starts_with("not(") {
        return false;
    }
    if hat_wort(rest, "unix") {
        return false;
    }
    // a `cfg_attr` that sets only a lint
    if zeile.starts_with("#[cfg_attr(") && rest.contains(", allow(") {
        return false;
    }
    true
}

/// The index after the attributes and comments that stand between a place and the code it guards.
fn hinter_attributen(zeilen: &[(usize, &str)], von: usize) -> usize {
    let mut i = von;
    while i < zeilen.len() && zeilen[i].1.starts_with("#[") {
        i += 1;
    }
    i
}

/// Whether the code from `zeilen[i]` on is one of the short forms. A function is read to the end of its signature.
fn ist_kurze_form(zeilen: &[(usize, &str)], i: usize) -> bool {
    let Some(&(_, z)) = zeilen.get(i) else {
        return false;
    };
    let text = |k: usize| zeilen.get(k).map_or("", |(_, t)| *t);
    // 1, 2
    if ist_use_aus_sys(z) || ist_kurzer_aufruf(z) {
        return true;
    }
    // 3: a block
    if z == "{" {
        return ist_kurzer_aufruf(text(i + 1)) && text(i + 2) == "}";
    }
    // 4: let-else
    if let Some(rest) = z
        .strip_prefix("let Some(")
        .and_then(|r| r.split_once(") = "))
        .and_then(|(name, aufruf)| ist_name(name).then_some(aufruf))
        .and_then(|a| a.strip_suffix(" else {"))
    {
        return ist_sys_aufruf(rest)
            && matches!(text(i + 1), "return false;" | "return true;")
            && text(i + 2) == "};";
    }
    // 5: if let
    if let Some((name, aufruf)) = z
        .strip_prefix("if let Some(")
        .and_then(|r| r.split_once(") = "))
        .and_then(|(name, aufruf)| aufruf.strip_suffix(" {").map(|a| (name, a)))
    {
        return ist_name(name)
            && ist_sys_aufruf(aufruf)
            && text(i + 1) == format!("return {name};")
            && text(i + 2) == "}";
    }
    // 7: a field and its initial value
    if let Some((feld, art)) = z.strip_suffix(',').and_then(|f| f.split_once(": ")) {
        let feld = feld.strip_prefix("pub(crate) ").unwrap_or(feld);
        return ist_name(feld) && art.strip_prefix("crate::sys::").is_some_and(ist_name);
    }
    if let Some(feld) = z.strip_suffix(',') {
        return ist_name(feld);
    }
    // 6: a function whose body is one call
    let ist_fn = ["fn ", "pub fn ", "pub(crate) fn "]
        .iter()
        .any(|p| z.starts_with(p));
    if ist_fn {
        let mut k = i;
        while k < zeilen.len() && !text(k).ends_with('{') {
            k += 1;
        }
        return text(k + 2) == "}" && {
            let koerper = text(k + 1);
            ist_sys_aufruf(koerper)
        };
    }
    false
}

/// Whether the match is a place of the allowed expressions `cfg!(windows)`.
fn ist_erlaubter_ausdruck(datei: &str, zeile: &str) -> bool {
    AUSDRUECKE
        .iter()
        .any(|(d, t)| datei.ends_with(d) && zeile.contains(t))
}

/// Every place in `quellen` that breaks the rule: `"<file>:<line>: <reason>"`. The paths are below `src`.
fn funde(quellen: &[(String, String)]) -> Vec<String> {
    let mut funde = Vec::new();
    for (datei, text) in quellen {
        if datei == "sys/windows.rs" {
            continue;
        }
        let name = datei.rsplit('/').next().unwrap_or(datei);
        if name.ends_with("_windows.rs") && !datei.starts_with("sys/") {
            if !WINDOWS_DATEIEN.contains(&name) {
                funde.push(format!("{datei}:1: a Windows file outside `src/sys/`"));
            }
            continue;
        }
        let zeilen = codezeilen(text);
        for (i, &(nr, z)) in zeilen.iter().enumerate() {
            if z.contains("cfg!(") && hat_wort(z, "windows") && !ist_erlaubter_ausdruck(datei, z) {
                funde.push(format!("{datei}:{nr}: `cfg!` names Windows"));
            }
            if !ist_windows_attribut(z) {
                continue;
            }
            // the swap of a module for a Windows file
            if z.starts_with("#[cfg_attr(windows, path = \"") {
                let pfad = z
                    .strip_prefix("#[cfg_attr(windows, path = \"")
                    .and_then(|r| r.strip_suffix("\")]"));
                if !pfad.is_some_and(|p| WINDOWS_DATEIEN.contains(&p)) {
                    funde.push(format!("{datei}:{nr}: a module path that is not listed"));
                }
                continue;
            }
            let k = hinter_attributen(&zeilen, i + 1);
            if datei.starts_with("sys/") {
                let gut = zeilen.get(k).is_some_and(|(_, t)| {
                    *t == "mod windows;"
                        || t.starts_with("use ")
                        || t.starts_with("pub use ")
                        || t.starts_with("pub(crate) use ")
                });
                if !gut {
                    funde.push(format!("{datei}:{nr}: more than `mod windows;` or a `use`"));
                }
            } else if !ist_kurze_form(&zeilen, k) {
                funde.push(format!(
                    "{datei}:{nr}: the code behind the condition is not a short form"
                ));
            }
        }
    }
    funde
}

/// The rule holds for the sources of the crate.
#[test]
fn windows_code_steht_nur_in_sys() {
    let gefunden = funde(&eigene_quellen());
    assert!(
        gefunden.is_empty(),
        "Windows code outside `src/sys/windows.rs`:\n{}",
        gefunden.join("\n")
    );
}

/// The check looks at the files it should: the Windows file exists, and the places of the short forms are found (a check that finds
/// nothing would be green for any rule).
#[test]
fn die_pruefung_sieht_die_kurzen_formen() {
    let quellen = eigene_quellen();
    assert!(quellen.iter().any(|(d, _)| d == "sys/windows.rs"));
    let plaetze: usize = quellen
        .iter()
        .filter(|(d, _)| d != "sys/windows.rs" && !d.starts_with("sys/"))
        .map(|(_, t)| {
            codezeilen(t)
                .iter()
                .filter(|(_, z)| ist_windows_attribut(z))
                .count()
        })
        .sum();
    assert!(plaetze >= 20, "only {plaetze} places found");
}

/// The counter-test: a line with logic behind `cfg(windows)` in a real file turns the check red. Each case appends one block to
/// `starten.rs` of the real sources.
#[test]
fn eine_zeile_mit_logik_hinter_cfg_windows_wird_gemeldet() {
    let faelle = [
        "#[cfg(windows)]\nlet x = a > 3;\n",
        "#[cfg(windows)]\nif a == b {\n    return 1;\n}\n",
        "#[cfg(windows)]\nlet x = crate::sys::f(a) + 1;\n",
        "#[cfg(windows)]\nlet x = crate::sys::f(g(a));\n",
        "#[cfg(windows)]\nlet x = crate::sys::f(a).unwrap_or(0);\n",
        "#[cfg(windows)]\nlet x = crate::sys::f(a) && crate::sys::g(b);\n",
        "#[cfg(windows)]\nlet x = |p| crate::sys::f(p);\n",
        "#[cfg(windows)]\nmatch a {\n    _ => {}\n}\n",
        "#[cfg(windows)]\nfn f() -> bool {\n    a == b\n}\n",
        "#[cfg(all(windows, feature = \"pruefbau\"))]\nlet n = a * 2;\n",
        "#[cfg_attr(windows, path = \"fremd_windows.rs\")]\nmod fremd;\n",
        "let b = cfg!(windows) && a;\n",
        "#[cfg(windows)]\nuse std::fs::File;\n",
        "#[cfg(all(test, windows, feature = \"pruefbau\"))]\nmod tests_windows {\n    fn f() -> bool {\n        a == b\n    }\n}\n",
    ];
    for fall in faelle {
        let mut quellen = eigene_quellen();
        let eins = quellen
            .iter_mut()
            .find(|(d, _)| d == "starten.rs")
            .expect("starten.rs");
        eins.1.push_str(fall);
        let gefunden = funde(&quellen);
        assert!(
            gefunden.iter().any(|f| f.starts_with("starten.rs:")),
            "no finding for:\n{fall}"
        );
    }
}

/// The counter-test the other way: every short form of the module text is accepted, and the places the check leaves alone stay free.
#[test]
fn die_kurzen_formen_und_die_ausnahmen_bleiben_frei() {
    let gut = [
        "#[cfg(windows)]\nuse crate::sys::gleiche_datei;\n",
        "#[cfg(windows)]\npub(crate) use crate::sys::marker_lesen as lesen;\n",
        "#[cfg(windows)]\npub use crate::sys::{Signale, signale_anmelden};\n",
        "#[cfg(windows)]\npub use sys::Signale;\n",
        "#[cfg(windows)]\ncrate::sys::kind_beenden(&mut self.kind);\n",
        "#[cfg(windows)]\ncrate::sys::eintrag_durchsuchbar(&pfad)?;\n",
        "#[cfg(windows)]\nlet (kind, job) = crate::sys::kind_mit_job(kind);\n",
        "#[cfg(windows)]\nlet modus = crate::sys::modus_von;\n",
        "#[cfg(windows)]\nlet name0 = crate::sys::ohne_exe(name0);\n",
        "    #[cfg(windows)]\n    tv_nsec: crate::sys::nanosekunden(frist),\n",
        "#[cfg(windows)]\n{\n    crate::sys::gegenseite_gut(strom, self.pid, self.euid)\n}\n",
        "#[cfg(windows)]\nlet Some(sitzung) = crate::sys::sitzung_mit_job(self, sitzung, nr) else {\n    return false;\n};\n",
        "#[cfg(all(windows, feature = \"pruefbau\"))]\nif let Some(code) = crate::sys::laufkind() {\n    return code;\n}\n",
        "#[cfg(all(windows, feature = \"pruefbau\"))]\nif let Some(code) = sys::laufkind() {\n    return code;\n}\n",
        "#[cfg(windows)]\npub fn laufen(&mut self, signale: Signale) -> i32 {\n    crate::sys::laufen(self, signale)\n}\n",
        "#[cfg(windows)]\npub(crate) job: crate::sys::JobStand,\n",
        "#[cfg(windows)]\njob,\n",
        "#[cfg(windows)]\n#[allow(dead_code)]\nuse crate::sys::stty;\n",
        "#[cfg_attr(not(windows), allow(unused_variables))]\nfn f() {}\n",
        "#[cfg_attr(windows, allow(dead_code))]\nfn f() {}\n",
        "#[cfg(not(windows))]\nfn f() { a == b }\n",
        "#![cfg(any(unix, windows))]\n",
        "#[cfg_attr(windows, path = \"loeschen_windows.rs\")]\nmod loeschen;\n",
        "// #[cfg(windows)] in a comment\n",
    ];
    for fall in gut {
        let q = vec![("beispiel.rs".to_owned(), fall.to_owned())];
        let gefunden = funde(&q);
        assert!(gefunden.is_empty(), "{fall}\n-> {gefunden:?}");
    }
    // `sys/mod.rs`: only `mod windows;` and `use` lines
    let mod_gut = "#[cfg(windows)]\n#[allow(unsafe_code)]\nmod windows;\n#[cfg(windows)]\npub(crate) use windows::{a, b};\n";
    let ohne_fund = funde(&[("sys/mod.rs".to_owned(), mod_gut.to_owned())]);
    assert_eq!(ohne_fund.len(), 0, "{ohne_fund:?}");
    let mod_schlecht = "#[cfg(windows)]\npub(crate) fn euid() -> u32 {\n    0\n}\n";
    assert_eq!(
        funde(&[("sys/mod.rs".to_owned(), mod_schlecht.to_owned())]).len(),
        1
    );
    // a Windows file that is not listed
    let fremd = vec![("neu_windows.rs".to_owned(), "fn f() {}\n".to_owned())];
    assert_eq!(funde(&fremd).len(), 1);
}
