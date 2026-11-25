//! The delivered build reads no seam: outside the code of the feature `pruefbau` the source of the program reads no
//! variable of the environment, and the places of the delivered build are the fixed ones.
//! The program itself is not started here: without the seams it would use the live paths of the account
//! (`/etc/werkbank/halter.json`, `$XDG_RUNTIME_DIR`), which a test must never touch.

#[test]
fn quelle_liest_ausserhalb_des_pruefbaus_keine_umgebung() {
    let quelle = include_str!("../src/main.rs");
    let mut ausserhalb = String::new();
    let mut rest = quelle;
    // drop every item that stands behind `#[cfg(feature = "pruefbau")]` (one function, up to its closing brace at
    // the indentation of the item)
    while let Some(pos) = rest.find("#[cfg(feature = \"pruefbau\")]") {
        ausserhalb.push_str(&rest[..pos]);
        let nach = &rest[pos..];
        let ende = nach.find("\n    }\n").map_or(nach.len(), |e| e + 7);
        rest = &nach[ende..];
    }
    ausserhalb.push_str(rest);
    assert!(
        !ausserhalb.contains("env::var"),
        "a variable of the environment outside the test build"
    );
    assert!(ausserhalb.contains("fn orte() -> Orte {\n        Orte::fest()\n    }"));
    let start = include_str!("../src/start.rs");
    assert!(start.contains("PathBuf::from(\"/etc/werkbank/halter.json\")"));
    assert!(start.contains("PathBuf::from(\"/etc/passwd\")"));
    assert!(start.contains("PathBuf::from(\"/etc/group\")"));
}
