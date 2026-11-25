//! The delivered build reads no seam: outside the code of the feature `pruefbau` the source of the program reads no
//! variable of the environment, and the places of the delivered build are the fixed ones.
//! The program itself is not started here: without the seams it would use the live paths of the account
//! (`/etc/werkbank/halter.json`, `$XDG_RUNTIME_DIR`), which a test must never touch.

#[test]
fn quelle_liest_ausserhalb_des_pruefbaus_keine_umgebung() {
    let quelle = include_str!("../src/main.rs");
    let mut ausserhalb = String::new();
    let mut rest = quelle;
    // drop every statement that stands behind `#[cfg(feature = "pruefbau")]` (one `let`, up to its `;` at the end
    // of a line)
    let mut weg = 0;
    while let Some(pos) = rest.find("#[cfg(feature = \"pruefbau\")]") {
        ausserhalb.push_str(&rest[..pos]);
        let nach = &rest[pos..];
        let ende = nach.find(";\n").map_or(nach.len(), |e| e + 2);
        weg += 1;
        rest = &nach[ende..];
    }
    ausserhalb.push_str(rest);
    assert_eq!(
        weg, 1,
        "one statement of the test build: the places from the seams"
    );
    assert!(
        !ausserhalb.contains("env::var"),
        "a variable of the environment outside the test build"
    );
    assert!(ausserhalb.contains(
        "        #[cfg(not(feature = \"pruefbau\"))]\n        let o = Orte::fest();\n        o\n"
    ));
    let start = include_str!("../src/start.rs");
    // the seams are read only through the reader that the test build hands in, never from the environment directly
    let naht = &start[start.find("pub fn aus_naht").expect("aus_naht")..];
    let naht = &naht[..naht.find("\n    }\n").expect("end of aus_naht")];
    assert!(
        !naht.contains("env::"),
        "aus_naht reads the environment itself"
    );
    assert!(naht.contains("lies(\"WB_TESTUMGEBUNG\")"));
    assert!(start.contains("PathBuf::from(\"/etc/werkbank/halter.json\")"));
    assert!(start.contains("PathBuf::from(\"/etc/passwd\")"));
    assert!(start.contains("PathBuf::from(\"/etc/group\")"));
}
