//! The `aufruf` cases of contract 1 (`rust/spec/start/faelle/aufruf-*.json` with `erwartet/`) against the program,
//! with the stand-ins of the search process, the model path and the harnesses and a recording `bwrap`.
//!
//! Checked per case: exit code and the one line on standard error, whether the search process and the model path
//! started and were ended, the folder marks afterwards, and for a valid case the start of `bwrap` as the child of the program,
//! the harness argv, the end of the argv of both children and the provider file of `pi`.
//!
//! Not run here, with the reason in [`AUSGELASSEN`]: the mutants of the build (they are mutants of a test set-up, not
//! inputs of the program; `cargo mutants` covers the code) and the owner case that needs a second user. The
//! properties E3, E4 and E5 inside a real enclosure are the cases of `tests/einschluss.rs`.

#![cfg(all(feature = "pruefbau", target_os = "linux"))]
#![allow(
    clippy::print_stderr,
    clippy::too_many_lines,
    clippy::zombie_processes,
    clippy::case_sensitive_file_extension_comparisons
)]

mod gemeinsam;

use std::fs;
use std::path::{Path, PathBuf};

use gemeinsam::{Welt, datei, lebt};
use serde_json::Value;

const SPEC: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../spec/start");

/// Cases this runner leaves out, with the reason.
const AUSGELASSEN: [(&str, &str); 4] = [
    (
        "-mutant-",
        "a mutant of the test set-up; the code is covered by cargo mutants",
    ),
    (
        "aufruf-marker-ordner-fremd",
        "needs a folder of another user (root)",
    ),
    (
        "aufruf-konfig-kaputt-eigentuemer-fremd",
        "needs a file of another user (root); the rule is the unit test of `modus_gut`",
    ),
    (
        "aufruf-starter-",
        "the way through the starter of the delivered build; measured in the throw-away VM, the starter itself by the program tests of einschluss",
    ),
];

fn s_text(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

fn lesen(p: &Path) -> Value {
    serde_json::from_slice(&fs::read(p).expect("read")).expect("json")
}

fn text(w: &Welt, v: &Value) -> String {
    w.abb(&serde_json::to_string(v).expect("json"))
}

/// Spoils the own configuration of the world in the way the case names.
fn konfig_verderben(w: &Welt, art: &str) {
    use std::os::unix::fs::PermissionsExt;
    let pfad = w.bin.join("lauf.json");
    let text = fs::read_to_string(&pfad).expect("konfig");
    let neu = |t: &str| datei(&pfad, t.as_bytes(), 0o644);
    match art {
        "fehlt" => fs::remove_file(&pfad).expect("remove"),
        "kein-json" => neu("{"),
        "einstellungen-fehlt" => fs::remove_file(&w.einstellungen).expect("remove"),
        "schluessel-fehlt" => {
            let von = text.find("\"vault\":").expect("vault");
            let bis = von + text[von..].find("\",").expect("end") + 2;
            neu(&format!("{}{}", &text[..von], &text[bis..]));
        }
        "schluessel-unbekannt" => {
            neu(&text.replacen("{\"fassung\":1", "{\"fassung\":1,\"unbekannt\":1", 1));
        }
        "pfad-relativ" => neu(&text.replacen(
            &format!("\"heim\":\"{}\"", w.heim.to_string_lossy()),
            "\"heim\":\"relativ\"",
            1,
        )),
        "zu-gross" => {
            let mut t = text.into_bytes();
            t.resize(65_537, b' ');
            datei(&pfad, &t, 0o644);
        }
        "ordner" => {
            fs::remove_file(&pfad).expect("remove");
            fs::create_dir(&pfad).expect("folder");
        }
        "eigentuemer-fremd" => {}
        modus => {
            let m = modus.strip_prefix("modus:").expect("mode");
            let m = u32::from_str_radix(m, 8).expect("octal");
            fs::set_permissions(&pfad, fs::Permissions::from_mode(m)).expect("mode");
        }
    }
}

/// The conversations of the case: `gespraeche/<sitzung>/` (0700) with `gespraech.json` (0600) and the state folder of
/// the harness with one file `verlauf`, so that a start can tell whether it got the state of the predecessor.
fn gespraeche_legen(w: &Welt, lage: &Value) {
    use std::os::unix::fs::PermissionsExt;
    let Some(liste) = lage.get("gespraeche").and_then(Value::as_object) else {
        return;
    };
    for (sitzung, inhalt) in liste {
        let o = w.gespraeche.join(sitzung);
        for d in [&w.gespraeche, &o, &o.join("zustand")] {
            fs::create_dir_all(d).expect("folder");
            fs::set_permissions(d, fs::Permissions::from_mode(0o700)).expect("mode");
        }
        datei(&o.join("gespraech.json"), text(w, inhalt).as_bytes(), 0o600);
        datei(
            &o.join("zustand").join("verlauf"),
            format!("GESPRAECH {sitzung}\n").as_bytes(),
            0o600,
        );
    }
}

/// The names in a folder, sorted; empty when it does not exist.
fn namen(o: &Path) -> Vec<String> {
    let mut n: Vec<String> = fs::read_dir(o)
        .map(|l| {
            l.filter_map(Result::ok)
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default();
    n.sort();
    n
}

/// Every path below `o` with its kind, mode and content, sorted (the tree of the conversations at a moment).
fn baum(o: &Path) -> Vec<String> {
    use std::os::unix::fs::MetadataExt;
    let mut aus = Vec::new();
    let mut offen = vec![o.to_path_buf()];
    while let Some(p) = offen.pop() {
        for n in namen(&p) {
            let q = p.join(&n);
            let m = fs::symlink_metadata(&q).expect("meta");
            let rel = q.strip_prefix(o).unwrap_or(&q).display().to_string();
            if m.is_dir() {
                aus.push(format!("{rel}/ {:o}", m.mode() & 0o7777));
                offen.push(q);
            } else {
                aus.push(format!(
                    "{rel} {:o} {}",
                    m.mode() & 0o7777,
                    String::from_utf8_lossy(&fs::read(&q).unwrap_or_default())
                ));
            }
        }
    }
    aus.sort();
    aus
}

/// The conversations after the run (`gespraeche_danach`): exactly the sessions of the case, each a folder 0700 with
/// `gespraech.json` 0600 of the expected content; `<harness-kennung>` is a key of the form, the one of the argv.
/// Returns the key of the new start, if the case has one.
fn gespraeche_pruefen(
    w: &Welt,
    soll: &Value,
    gleich: &mut impl FnMut(&str, String, String),
) -> Option<String> {
    use std::os::unix::fs::PermissionsExt;
    let liste = soll.get("gespraeche_danach")?.as_object()?;
    gleich(
        "gespraeche_danach: sitzungen",
        format!("{:?}", namen(&w.gespraeche)),
        format!("{:?}", liste.keys().collect::<Vec<_>>()),
    );
    let mut neu = None;
    for (sitzung, inhalt) in liste {
        let o = w.gespraeche.join(sitzung);
        let modus = |p: &Path| {
            fs::symlink_metadata(p).map_or(0, |m| {
                if m.file_type().is_symlink() {
                    0
                } else {
                    m.permissions().mode() & 0o7777
                }
            })
        };
        gleich(
            "gespraech ordner 0700",
            format!("{:o}", modus(&o)),
            "700".to_owned(),
        );
        gleich(
            "gespraech.json 0600",
            format!("{:o}", modus(&o.join("gespraech.json"))),
            "600".to_owned(),
        );
        let ist: Value =
            serde_json::from_slice(&fs::read(o.join("gespraech.json")).unwrap_or_default())
                .unwrap_or(Value::Null);
        let mut soll_inhalt: Value = serde_json::from_str(&text(w, inhalt)).expect("json");
        if soll_inhalt["kennung"] == "<harness-kennung>" {
            let k = ist["kennung"].as_str().unwrap_or("").to_owned();
            gleich(
                "harness-kennung ist eine UUID v4",
                lauf::gespraech::ist_kennung(&k).to_string(),
                "true".to_owned(),
            );
            soll_inhalt["kennung"] = Value::String(k.clone());
            neu = Some(k);
        }
        gleich(
            &format!("gespraech {sitzung}"),
            ist.to_string(),
            soll_inhalt.to_string(),
        );
        // a conversation of version 1 has no state folder of the harness
        let soll_namen: &[&str] = if soll_inhalt["kennung"].is_null() {
            &["gespraech.json"]
        } else {
            &["gespraech.json", "zustand"]
        };
        gleich(
            &format!("inhalt gespraech {sitzung}"),
            format!("{:?}", namen(&o)),
            format!("{soll_namen:?}"),
        );
    }
    neu
}

/// Builds the world of one case and runs the program. Returns the world, the run and the PID-file folder.
fn aufbauen_und_laufen(name: &str, lage: &Value) -> (Welt, gemeinsam::Lauf) {
    let w = Welt::neu();
    // settings; the default of the firm carries the same targets, so the cases of contract 1 keep their codes
    let einst = text(&w, &lage["einstellungen"]);
    datei(&w.einstellungen, einst.as_bytes(), 0o600);
    w.vorgabe_schreiben(&einst);
    // own configuration; a missing enclosure is a bwrap that does not exist
    let mut w = w;
    if lage["einschluss"] == Value::Bool(false) {
        w.bwrap = Some(w.wurzel.join("bin").join("fehlt-bwrap"));
    }
    // a harness without an option to continue (column "Fortsetzen" with "keine")
    if lage.get("harness_fortsetzung").and_then(Value::as_str) == Some("keine") {
        w.ohne_fortsetzen = vec![lage["start"]["harness"].as_str().unwrap_or("").to_owned()];
    }
    w.konfig_schreiben("zugelassen");
    gespraeche_legen(&w, lage);
    if let Some(art) = lage.get("konfig").and_then(Value::as_str) {
        konfig_verderben(&w, art);
    }
    // the start file
    let start = w.startdatei();
    if let Some(s) = lage.get("start") {
        let modus =
            u32::from_str_radix(lage["datei_modus"].as_str().unwrap_or("0600"), 8).unwrap_or(0o600);
        if lage["datei_art"] == "symlink" {
            let echt = w.start.join("echt.json");
            datei(&echt, text(&w, s).as_bytes(), modus);
            std::os::unix::fs::symlink(&echt, &start).expect("link");
        } else {
            datei(&start, text(&w, s).as_bytes(), modus);
        }
        // the working folder, or a link to the resolved one
        let arbeit = PathBuf::from(w.abb(s["arbeitsordner"].as_str().unwrap_or("/x")));
        if lage["ordner_vorhanden"] != Value::Bool(false) {
            if let Some(echt) = lage.get("realpath").and_then(Value::as_str) {
                let echt = PathBuf::from(w.abb(echt));
                fs::create_dir_all(&echt).expect("folder");
                fs::create_dir_all(arbeit.parent().expect("parent")).expect("folder");
                std::os::unix::fs::symlink(&echt, &arbeit).expect("link");
            } else {
                fs::create_dir_all(&arbeit).expect("folder");
            }
        }
        if let Some(m) = s.get("marker") {
            let z = PathBuf::from(w.abb(m["ordner"].as_str().unwrap_or("/x")));
            match lage.get("marker_ordner").and_then(Value::as_str) {
                Some("fehlt") => {}
                Some("modus:0755") => {
                    fs::create_dir_all(&z).expect("folder");
                    fs::set_permissions(&z, std::os::unix::fs::PermissionsExt::from_mode(0o755))
                        .expect("mode");
                }
                Some("verweis") => {
                    let echt = w.wurzel.join("zaehl-echt");
                    fs::create_dir_all(&echt).expect("folder");
                    fs::set_permissions(&echt, std::os::unix::fs::PermissionsExt::from_mode(0o700))
                        .expect("mode");
                    fs::create_dir_all(z.parent().expect("parent")).expect("folder");
                    std::os::unix::fs::symlink(&echt, &z).expect("link");
                }
                _ => {
                    fs::create_dir_all(&z).expect("folder");
                    fs::set_permissions(&z, std::os::unix::fs::PermissionsExt::from_mode(0o700))
                        .expect("mode");
                }
            }
        }
    }
    match lage.get("ordnermarken") {
        Some(Value::String(k)) if k == "kaputt" => {
            datei(
                &w.zustand.join("ordnermarken.json"),
                b"{\"fassung\":1,\"marken\":[",
                0o600,
            );
        }
        Some(liste @ Value::Array(_)) => {
            let t = format!("{{\"fassung\":1,\"marken\":{}}}", text(&w, liste));
            datei(&w.zustand.join("ordnermarken.json"), t.as_bytes(), 0o600);
        }
        _ => {}
    }
    // the children
    match lage["suche"].as_str() {
        Some(z) if z.starts_with("zeile:") || z == "stumm" => w.verhalten("rangkern", z),
        _ => {}
    }
    match lage.get("modellweg").and_then(Value::as_str) {
        Some("stumm") => w.verhalten("modellweg", "stumm"),
        Some("fehlt") => {
            let _ = fs::remove_file(w.bin.join("modellweg"));
        }
        _ => {}
    }
    if lage["harness_vorhanden"] == Value::Bool(false) {
        for h in gemeinsam::HARNESSE {
            let _ = fs::remove_file(w.bin.join(h));
        }
    }
    if lage.get("sitzungen_ordner").and_then(Value::as_str) == Some("fehlt") {
        fs::remove_dir(w.zustand.join("sitzungen")).expect("remove");
    }
    for v in lage
        .get("vorhanden")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        datei(
            Path::new(&w.abb(v.as_str().unwrap_or("/x"))),
            b"VORHANDEN\n",
            0o600,
        );
    }
    if let Some(n) = lage.get("socket_ordner_laenge").and_then(Value::as_u64) {
        // a socket folder whose path alone has `n` bytes
        let basis = w.wurzel.to_string_lossy().len() + 1;
        let lang = w.wurzel.join(
            "l".repeat(
                usize::try_from(n)
                    .unwrap_or(120)
                    .saturating_sub(basis)
                    .max(1),
            ),
        );
        fs::create_dir_all(&lang).expect("folder");
        let konfig = fs::read_to_string(w.bin.join("lauf.json")).expect("konfig");
        let neu = konfig.replace(
            &format!("\"laufzeit\":\"{}\"", w.laufzeit.to_string_lossy()),
            &format!("\"laufzeit\":\"{}\"", lang.to_string_lossy()),
        );
        datei(&w.bin.join("lauf.json"), neu.as_bytes(), 0o644);
    }
    // argv
    let argv: Vec<String> = lage["argv"]
        .as_array()
        .expect("argv")
        .iter()
        .skip(1)
        .map(|a| match a.as_str() {
            Some("<start>") => start.to_string_lossy().into_owned(),
            Some(andere) => w.abb(andere).replace(['<', '>'], ""),
            None => String::new(),
        })
        .map(|a| {
            if a == "fehlt" {
                w.start.join("fehlt.json").to_string_lossy().into_owned()
            } else {
                a
            }
        })
        .collect();
    let env: Vec<(String, String)> = lage
        .get("umgebung")
        .and_then(Value::as_object)
        .map(|o| {
            o.iter()
                .map(|(k, v)| (k.clone(), w.abb(v.as_str().unwrap_or(""))))
                .collect()
        })
        .unwrap_or_default();
    let befehl = lage["argv"][0].as_str() == Some("werkbank-ordnermarke");
    if befehl {
        // the command works on the folder of the argument; the log folder may be made unusable
        if lage["ordner_vorhanden"] != Value::Bool(false)
            && let Some(o) = argv.get(1)
        {
            fs::create_dir_all(o).expect("folder");
        }
        if lage.get("protokoll").and_then(Value::as_str) == Some("nicht-schreibbar") {
            fs::remove_dir_all(&w.protokoll).expect("remove");
            datei(&w.protokoll, b"KEIN-ORDNER\n", 0o600);
        }
    }
    let lauf = if befehl {
        w.programm_lauf("werkbank-ordnermarke", &argv, &env)
    } else {
        w.lauf(&argv, &env)
    };
    let _ = name;
    (w, lauf)
}

/// The arguments after the `--` of the final `bwrap` call, without the anchor and the forwarder in front of the
/// harness.
fn harness_teil(args: &[String]) -> Vec<String> {
    let naht = args
        .iter()
        .position(|a| a == "--")
        .map_or(args.len(), |i| i + 1);
    let mut rest = &args[naht..];
    if let Some(i) = rest.iter().position(|a| a == einschluss::ANKER_NAME) {
        rest = &rest[i + 1..];
    }
    if rest.first().map(String::as_str) == Some(einschluss::GAST_WEITER)
        && let Some(i) = rest.iter().position(|a| a == "--")
    {
        rest = &rest[i + 1..];
    }
    rest.to_vec()
}

/// The log of the command: the lines of the chain file of the source `ordnermarke` (without `vorher` and `zeit`), the
/// name of the file, the first anchor and that the lines name no path.
fn protokoll_pruefen(
    w: &Welt,
    lage: &Value,
    soll: &Value,
    gleich: &mut impl FnMut(&str, String, String),
) {
    let mut dateien: Vec<PathBuf> = fs::read_dir(&w.protokoll)
        .map(|l| l.filter_map(Result::ok).map(|e| e.path()).collect())
        .unwrap_or_default();
    dateien.retain(|p| {
        p.file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.starts_with("ordnermarke-") && n.ends_with(".jsonl"))
    });
    let roh: String = dateien
        .iter()
        .map(|p| fs::read_to_string(p).unwrap_or_default())
        .collect();
    let mut ist: Vec<Value> = roh
        .lines()
        .map(|z| serde_json::from_str(z).expect("line"))
        .collect();
    let mut monat = String::new();
    let mut vorher = String::new();
    for z in &mut ist {
        z["zeit"]
            .as_str()
            .unwrap_or("")
            .get(..7)
            .unwrap_or("")
            .clone_into(&mut monat);
        if vorher.is_empty() {
            z["vorher"].as_str().unwrap_or("").clone_into(&mut vorher);
        }
        let o = z.as_object_mut().expect("object");
        o.remove("vorher");
        o.remove("zeit");
    }
    let soll_zeilen = soll["zeilen"].as_array().cloned().unwrap_or_default();
    gleich(
        "protokollzeilen",
        Value::Array(ist).to_string(),
        Value::Array(soll_zeilen.clone()).to_string(),
    );
    if soll_zeilen.is_empty() {
        gleich(
            "protokolldateien",
            dateien.len().to_string(),
            "0".to_owned(),
        );
        return;
    }
    gleich(
        "protokolldatei",
        format!(
            "{:?}",
            dateien
                .iter()
                .filter_map(|p| p.file_name())
                .collect::<Vec<_>>()
        ),
        format!("{:?}", [format!("ordnermarke-{monat}.jsonl")]),
    );
    gleich("erste Zeile: vorher", vorher, "0".repeat(64));
    if soll["enthaelt_pfad"] == Value::Bool(false) {
        let ordner = w.abb(lage["argv"][2].as_str().unwrap_or("/x"));
        gleich(
            "zeile ohne pfad",
            (roh.contains(&ordner) || roh.contains("projekt")).to_string(),
            "false".to_owned(),
        );
    }
}

fn pruefen(name: &str, fall: &Value, soll: &Value) -> Result<(), String> {
    let lage = &fall["lage"];
    let (w, lauf) = aufbauen_und_laufen(name, lage);
    let mut fehler = Vec::new();
    let mut gleich = |was: &str, ist: String, soll: String| {
        if ist != soll {
            fehler.push(format!("{was}: ist {ist:?}, soll {soll:?}"));
        }
    };
    let suche = w.pid_von("rangkern");
    let modellweg = w.pid_von("modellweg");
    if let Some(e) = soll.get("exit") {
        gleich("exit", format!("{:?}", lauf.exit()), format!("Some({e})"));
    }
    if let Some(z) = soll.get("stderr").and_then(Value::as_str) {
        let ist = lauf.stderr();
        let soll_zeile = if z.is_empty() {
            String::new()
        } else {
            format!("{z}\n")
        };
        if soll.get("exec").is_some() {
            gleich("stderr", ist, String::new());
        } else {
            gleich("stderr", ist, soll_zeile);
        }
    }
    for (feld, pid) in [
        ("suche_gestartet", suche),
        ("modellweg_gestartet", modellweg),
    ] {
        if let Some(b) = soll.get(feld).and_then(Value::as_bool) {
            gleich(feld, pid.is_some().to_string(), b.to_string());
        }
    }
    for (feld, pid) in [("suche_beendet", suche), ("modellweg_beendet", modellweg)] {
        if soll.get(feld).and_then(Value::as_bool) == Some(true) {
            gleich(
                feld,
                pid.is_some_and(|p| !lebt(p)).to_string(),
                "true".to_owned(),
            );
        }
    }
    if let Some(liste) = soll.get("marke_danach") {
        let ist: Value = serde_json::from_slice(
            &fs::read(w.zustand.join("ordnermarken.json")).unwrap_or_default(),
        )
        .unwrap_or(Value::Null);
        let mut soll_liste: Value = serde_json::from_str(&text(&w, liste)).expect("json");
        // a new mark: any key of the form that no other mark carries (the cases write "<neu>" or a value of their
        // own source of randomness, which the program does not take)
        let alte: Vec<&str> = lage["ordnermarken"]
            .as_array()
            .map(|l| l.iter().filter_map(|m| m["kennung"].as_str()).collect())
            .unwrap_or_default();
        if let (Some(s_liste), Some(i_liste)) =
            (soll_liste.as_array_mut(), ist["marken"].as_array())
        {
            for (s_m, i_m) in s_liste.iter_mut().zip(i_liste) {
                let neu = s_m["kennung"].as_str().is_some_and(|k| !alte.contains(&k));
                let i_k = i_m["kennung"].as_str().unwrap_or("");
                let frei = i_liste
                    .iter()
                    .filter(|x| x["kennung"].as_str() == Some(i_k))
                    .count()
                    == 1;
                if neu && start::ist_marken_kennung(i_k) && frei {
                    s_m["kennung"] = Value::String(i_k.to_owned());
                }
            }
        }
        gleich(
            "marke_danach",
            ist["marken"].to_string(),
            soll_liste.to_string(),
        );
    }
    if let Some(p) = soll.get("protokoll") {
        protokoll_pruefen(&w, lage, p, &mut gleich);
    }
    if let Some(z) = soll.get("zielliste") {
        let dateien: Vec<PathBuf> = fs::read_dir(w.zustand.join("sitzungen"))
            .map(|l| l.filter_map(Result::ok).map(|e| e.path()).collect())
            .unwrap_or_default();
        match z.get("pfad").and_then(Value::as_str) {
            None => gleich("zielliste", format!("{dateien:?}"), "[]".to_owned()),
            Some(pfad) => {
                let pfad = PathBuf::from(w.abb(pfad));
                let inhalt = fs::read_to_string(&pfad).unwrap_or_default();
                gleich(
                    "zielliste da",
                    pfad.is_file().to_string(),
                    "true".to_owned(),
                );
                let modus = fs::metadata(&pfad).map_or(0, |m| {
                    std::os::unix::fs::PermissionsExt::mode(&m.permissions()) & 0o7777
                });
                gleich("zielliste modus", format!("{modus:o}"), "600".to_owned());
                gleich(
                    "zielliste ohne schluessel",
                    inhalt.contains("MARKE-").to_string(),
                    "false".to_owned(),
                );
            }
        }
    }
    if let Some(hosts) = soll.get("zielliste_hosts") {
        let pfad = w.zustand.join("sitzungen/s-3.zielliste.json");
        let ist: Value =
            serde_json::from_slice(&fs::read(pfad).unwrap_or_default()).unwrap_or(Value::Null);
        let ist_hosts: Vec<Value> = ist["ziele"]
            .as_array()
            .map(|l| l.iter().map(|z| z["host"].clone()).collect())
            .unwrap_or_default();
        gleich(
            "zielliste_hosts",
            Value::Array(ist_hosts).to_string(),
            hosts.to_string(),
        );
    }
    for v in soll
        .get("datei_unveraendert")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let inhalt = fs::read(w.abb(v.as_str().unwrap_or("/x"))).unwrap_or_default();
        gleich(
            "datei unveraendert",
            String::from_utf8_lossy(&inhalt).into_owned(),
            "VORHANDEN\n".to_owned(),
        );
    }
    for (feld, da) in [("datei_da", true), ("datei_fehlt", false)] {
        for v in soll
            .get(feld)
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let pfad = PathBuf::from(w.abb(v.as_str().unwrap_or("/x")));
            gleich(
                &format!("{feld} {}", pfad.display()),
                pfad.symlink_metadata().is_ok().to_string(),
                da.to_string(),
            );
        }
    }
    if let Some(teil) = soll
        .get("modellweg_argv_enthaelt")
        .and_then(Value::as_array)
    {
        let teil: Vec<String> = teil
            .iter()
            .map(|x| w.abb(x.as_str().unwrap_or("")))
            .collect();
        let ist = w.argv_von("modellweg").unwrap_or_default();
        gleich(
            "modellweg_argv_enthaelt",
            ist.windows(teil.len())
                .any(|f| f == teil.as_slice())
                .to_string(),
            "true".to_owned(),
        );
    }
    let kennung = gespraeche_pruefen(&w, soll, &mut gleich);
    // fail closed: a start that ends with a code leaves the conversations of the case as they were
    if soll.get("exit").is_some() && lage.get("gespraeche").is_some() {
        gleich(
            "gespraeche unveraendert",
            format!("{:?}", namen(&w.gespraeche)),
            format!(
                "{:?}",
                lage["gespraeche"]
                    .as_object()
                    .map(|o| o.keys().cloned().collect::<Vec<_>>())
                    .unwrap_or_default()
            ),
        );
    }
    if let Some(z) = soll
        .get("zustandsordner_beim_start")
        .and_then(Value::as_str)
    {
        let sitzung = lage["start"]["sitzung"].as_str().unwrap_or("");
        let kopie = w.wurzel.join(gemeinsam::GESPRAECHE_BEIM_START);
        let ist = namen(&kopie.join(sitzung).join("zustand"));
        let erwartet: Vec<String> = match z {
            "leer" => Vec::new(),
            _ => vec!["verlauf".to_owned()],
        };
        gleich(
            "zustandsordner_beim_start",
            format!("{ist:?}"),
            format!("{erwartet:?}"),
        );
        if z != "leer" {
            let alt = lage["start"]["fortsetzung_von"].as_str().unwrap_or("");
            gleich(
                "zustand der vorgaengersitzung",
                fs::read_to_string(kopie.join(sitzung).join("zustand/verlauf")).unwrap_or_default(),
                format!("GESPRAECH {alt}\n"),
            );
        }
        // only the conversation comes from `gespraeche`, at its place inside the empty state folder of the session;
        // the state folder itself is the one of the session folder, so nothing else of the predecessor is bound
        // (contract 1, "Gespräch und Einstellungen im Zustandsordner")
        let harness = lage["start"]["harness"].as_str().unwrap_or("");
        let name = match harness {
            "pi" => ".pi",
            "copilot" => ".copilot",
            _ => ".claude",
        };
        let ort = einschluss::gespraech_ort(harness).unwrap_or("");
        let quelle = s_text(&w.gespraeche.join(sitzung).join("zustand"));
        let ziel = s_text(&w.heim.join(name).join(ort));
        let a = w.bwrap_aufrufe().last().cloned().unwrap_or_default().1;
        gleich(
            "gespraech aus gespraeche gebunden",
            a.windows(3)
                .any(|f| f[0] == "--bind" && f[1] == quelle && f[2] == ziel)
                .to_string(),
            "true".to_owned(),
        );
        let zustand_quelle: Vec<&String> = a
            .windows(3)
            .filter(|f| f[0] == "--bind" && f[2] == s_text(&w.heim.join(name)))
            .map(|f| &f[1])
            .collect();
        gleich(
            "zustandsordner aus dem sitzungsordner",
            format!("{zustand_quelle:?}"),
            format!("{:?}", [s_text(&w.sitzungen.join(sitzung).join("zustand"))]),
        );
        // no work after the start: the conversations are exactly as the starter found them
        gleich(
            "gespraeche nach dem start unveraendert",
            format!("{:?}", baum(&w.gespraeche)),
            format!("{:?}", baum(&kopie)),
        );
    }
    if let Some(h) = soll.get("exec").and_then(Value::as_str) {
        let aufrufe = w.bwrap_aufrufe();
        let letzter = aufrufe.last().cloned().unwrap_or_default();
        gleich(
            "exit nach exec",
            format!("{:?}", lauf.exit()),
            "Some(0)".to_owned(),
        );
        // the program stays the parent of the enclosure (contract 1, "Lebensdauer"; before 21.11.2026 the same PID
        // after an exec, key `gleiche_pid`)
        // for every case with `exec`, as `gleiche PID` was before; the key in `erwartet/` documents it
        gleich(
            "eltern_ist_programm",
            (letzter.0 == lauf.pid).to_string(),
            "true".to_owned(),
        );
        if let Some(h) = soll.get("heim_im_einschluss") {
            let heim = w.heim.to_string_lossy().into_owned();
            let stelle = |wert: &str| wert.replace("<heim>", &heim);
            let a = &letzter.1;
            let folgt = |name: &str, wert: &str| a.windows(2).any(|f| f[0] == name && f[1] == wert);
            // HOME is the home of the host, and the home is a private tmpfs at that path
            let home = stelle(h["home"].as_str().unwrap_or(""));
            gleich(
                "HOME im Einschluss",
                a.windows(3)
                    .any(|f| f[0] == "--setenv" && f[1] == "HOME" && f[2] == home)
                    .to_string(),
                "true".to_owned(),
            );
            gleich(
                "heim als tmpfs",
                folgt("--tmpfs", &stelle(h["tmpfs"].as_str().unwrap_or(""))).to_string(),
                "true".to_owned(),
            );
            // below the home only the state folder (with the file of the harness inside it) is bound
            let erlaubt = format!("{heim}/{}", h["unter_heim_nur"].as_str().unwrap_or(""));
            let unter: Vec<&String> = a
                .windows(3)
                .filter(|f| matches!(f[0].as_str(), "--bind" | "--ro-bind" | "--tmpfs" | "--dir"))
                .map(|f| {
                    if f[0] == "--tmpfs" || f[0] == "--dir" {
                        &f[1]
                    } else {
                        &f[2]
                    }
                })
                .filter(|z| z.starts_with(&format!("{heim}/")))
                .collect();
            // the cloud harness has one more place: its package folder, read-only and exactly at the place of the CLI
            let paket = h["unter_heim_nur_lesend"].as_str().map(|rel| {
                format!(
                    "{heim}/{}",
                    rel.replace("<plattform>", &gemeinsam::plattform())
                        .replace("<fassung>", gemeinsam::FASSUNG)
                )
            });
            gleich(
                "unter dem Heim nur der Zustandsordner",
                format!(
                    "{:?}",
                    unter.iter().all(|z| **z == erlaubt
                        || z.starts_with(&format!("{erlaubt}/"))
                        || paket.as_deref() == Some(z.as_str()))
                ),
                "true".to_owned(),
            );
            if let Some(p) = &paket {
                let binds: Vec<&str> = a
                    .windows(3)
                    .filter(|f| f[2] == *p)
                    .map(|f| f[0].as_str())
                    .collect();
                gleich(
                    "Paketordner nur lesend",
                    format!("{binds:?}"),
                    "[\"--ro-bind\"]".to_owned(),
                );
            }
            gleich(
                "Zustandsordner unter dem Heim",
                unter.iter().any(|z| **z == erlaubt).to_string(),
                "true".to_owned(),
            );
        }
        let teil = harness_teil(&letzter.1);
        gleich(
            "harness",
            teil.first().cloned().unwrap_or_default(),
            w.bin.join(h).to_string_lossy().into_owned(),
        );
        for o in soll
            .get("argv_ohne")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let o = o.as_str().unwrap_or("");
            gleich(
                &format!("argv ohne {o}"),
                teil.iter().any(|a| a == o).to_string(),
                "false".to_owned(),
            );
        }
        if let Some(a) = soll.get("harness_argv").and_then(Value::as_array) {
            let soll_argv: Vec<String> = a
                .iter()
                .skip(1)
                .map(|x| {
                    x.as_str()
                        .unwrap_or("")
                        .replace("<heim>", &w.heim.to_string_lossy())
                        .replace("<harness-kennung>", kennung.as_deref().unwrap_or("<fehlt>"))
                })
                .collect();
            gleich(
                "harness_argv",
                format!("{:?}", &teil[1.min(teil.len())..]),
                format!("{soll_argv:?}"),
            );
        }
        if let Some(a) = soll.get("anbieter") {
            // the recording `bwrap` copied it: the watcher removes the session folder once the harness has ended
            let datei = w.wurzel.join(gemeinsam::ANBIETER_KOPIE);
            let ist: Value =
                serde_json::from_slice(&fs::read(datei).unwrap_or_default()).unwrap_or(Value::Null);
            gleich("anbieter", ist.to_string(), a.to_string());
        }
        // both children ran, and they end with the harness (the recording bwrap has ended: PR_SET_PDEATHSIG)
        for (n, pid) in [("suche", suche), ("modellweg", modellweg)] {
            gleich(
                &format!("{n} gestartet"),
                pid.is_some().to_string(),
                "true".to_owned(),
            );
            gleich(
                &format!("{n} endet mit dem harness"),
                pid.is_some_and(|p| gemeinsam::endet_binnen(p, gemeinsam::HAENGESPERRE_MS))
                    .to_string(),
                "true".to_owned(),
            );
        }
    }
    for (feld, programm) in [
        ("suche_argv_ende", "rangkern"),
        ("modellweg_argv_ende", "modellweg"),
    ] {
        if let Some(a) = soll.get(feld).and_then(Value::as_array) {
            let soll_ende: Vec<String> =
                a.iter().map(|x| w.abb(x.as_str().unwrap_or(""))).collect();
            let ist = w.argv_von(programm).unwrap_or_default();
            let ende = ist[ist.len().saturating_sub(soll_ende.len())..].to_vec();
            gleich(feld, format!("{ende:?}"), format!("{soll_ende:?}"));
        }
    }
    if let Some(a) = soll.get("modellweg_argv").and_then(Value::as_array) {
        let ist = w.argv_von("modellweg").unwrap_or_default();
        // the fixed words of the case; the placeholders stand for paths of the world
        let feste: Vec<&str> = a
            .iter()
            .skip(1)
            .filter_map(Value::as_str)
            .filter(|x| !x.starts_with('<'))
            .collect();
        let ist_feste: Vec<&str> = ist
            .iter()
            .map(String::as_str)
            .filter(|x| feste.contains(x))
            .collect();
        gleich(
            "modellweg_argv",
            format!("{ist_feste:?}"),
            format!("{feste:?}"),
        );
    }
    if let Some(ohne) = soll
        .get("ohne_marker_nicht_in_argv")
        .and_then(Value::as_array)
    {
        for p in ["rangkern", "modellweg"] {
            let ist = w.argv_von(p).unwrap_or_default();
            for o in ohne.iter().filter_map(Value::as_str) {
                gleich(
                    &format!("{p} ohne {o}"),
                    ist.iter().any(|a| a == o).to_string(),
                    "false".to_owned(),
                );
            }
        }
    }
    for pid in [suche, modellweg].into_iter().flatten() {
        if let Some(p) = rustix::process::Pid::from_raw(pid) {
            let _ = rustix::process::kill_process(p, rustix::process::Signal::KILL);
        }
    }
    if fehler.is_empty() {
        Ok(())
    } else {
        Err(fehler.join("; "))
    }
}

#[test]
fn the_call_cases_of_contract_1() {
    let mut namen: Vec<String> = fs::read_dir(format!("{SPEC}/faelle"))
        .expect("cases")
        .filter_map(Result::ok)
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.starts_with("aufruf-") && n.ends_with(".json"))
        .collect();
    namen.sort();
    let (mut gruen, mut rot, mut aus) = (0, Vec::new(), Vec::new());
    for n in &namen {
        if let Some((_, grund)) = AUSGELASSEN.iter().find(|(m, _)| n.contains(m)) {
            aus.push(format!("{n}: {grund}"));
            continue;
        }
        let fall = lesen(Path::new(&format!("{SPEC}/faelle/{n}")));
        let soll = lesen(Path::new(&format!("{SPEC}/erwartet/{n}")));
        match pruefen(n, &fall, &soll) {
            Ok(()) => gruen += 1,
            Err(e) => rot.push(format!("{n}: {e}")),
        }
    }
    eprintln!(
        "faelle: {gruen} gruen, {} rot, {} ausgelassen",
        rot.len(),
        aus.len()
    );
    for a in &aus {
        eprintln!("ausgelassen {a}");
    }
    assert!(rot.is_empty(), "{}", rot.join("\n"));
    assert!(gruen >= 40, "too few cases ran: {gruen}");
}
