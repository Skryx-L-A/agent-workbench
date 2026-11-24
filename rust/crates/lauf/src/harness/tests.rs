//! Tests of the start form of each harness.

use std::path::PathBuf;

use super::*;

fn w(t: &[&str]) -> Vec<String> {
    t.iter().map(|x| (*x).to_owned()).collect()
}

#[test]
fn each_harness_gets_the_argv_of_the_table() {
    let heim = PathBuf::from("/srv/beispiel/heim");
    assert_eq!(
        argv("pi", Path::new("/srv/bin/pi"), "qwen3:8b", &heim, &[]),
        w(&[
            "/srv/bin/pi",
            "--provider",
            "wb-lokal",
            "--model",
            "qwen3:8b"
        ])
    );
    assert_eq!(
        argv(
            "copilot",
            Path::new("/srv/bin/copilot"),
            "claude-sonnet-5.5",
            &heim,
            &[]
        ),
        w(&[
            "/srv/bin/copilot",
            "--model",
            "claude-sonnet-5.5",
            "--disable-builtin-mcps",
            "--additional-mcp-config",
            "@/srv/beispiel/heim/.copilot/wb-mcp.json"
        ])
    );
    assert_eq!(
        argv("copilot", Path::new("/srv/bin/copilot"), "auto", &heim, &[]),
        w(&[
            "/srv/bin/copilot",
            "--model",
            "auto",
            "--auto-tier",
            "efficiency",
            "--disable-builtin-mcps",
            "--additional-mcp-config",
            "@/srv/beispiel/heim/.copilot/wb-mcp.json"
        ])
    );
    assert_eq!(
        argv(
            "claude",
            Path::new("/srv/bin/claude"),
            "claude-sonnet-5-5",
            &heim,
            &[]
        ),
        w(&["/srv/bin/claude", "--model", "claude-sonnet-5-5"])
    );
    // the model stays one element, whatever it holds
    let a = argv("pi", Path::new("/p"), "a --b", &heim, &[]);
    assert_eq!(a.last().map(String::as_str), Some("a --b"));
    // the tools of the settings block go to copilot only, after the MCP file
    let werkzeuge = [
        einstellungen::Werkzeug::SuchMcp,
        einstellungen::Werkzeug::Shell,
    ];
    let c = argv(
        "copilot",
        Path::new("/srv/bin/copilot"),
        "auto",
        &heim,
        &werkzeuge,
    );
    assert_eq!(
        c[c.len() - 6..].to_vec(),
        [
            "--additional-mcp-config",
            "@/srv/beispiel/heim/.copilot/wb-mcp.json",
            "--allow-tool",
            "such-mcp",
            "--allow-tool",
            "shell"
        ]
    );
    assert_eq!(c[0], "/srv/bin/copilot");
    assert_eq!(argv("pi", Path::new("/p"), "m", &heim, &werkzeuge).len(), 5);
    assert_eq!(
        argv("claude", Path::new("/p"), "m", &heim, &werkzeuge).len(),
        3
    );
    // auto is a word of copilot only
    assert_eq!(argv("pi", Path::new("/p"), "auto", &heim, &[]).len(), 5);
}

#[test]
fn the_files_are_the_provider_of_pi_and_the_mcp_file_of_copilot() {
    let anbieter = pi_anbieter("qwen3:8b");
    assert_eq!(
        anbieter,
        "{\"providers\":{\"wb-lokal\":{\"baseUrl\":\"http://127.0.0.1:11434/v1\",\"api\":\"openai-completions\",\"apiKey\":\"wb-ohne-schluessel\",\"models\":[{\"id\":\"qwen3:8b\"}]}}}\n"
    );
    assert!(einstellungen::json::lesen(anbieter.as_bytes()).is_ok());
    let mcp = copilot_mcp(Path::new("/usr/lib/werkbank/such-mcp"));
    assert_eq!(
        mcp,
        "{\"mcpServers\":{\"such-mcp\":{\"type\":\"local\",\"command\":\"/usr/lib/werkbank/such-mcp\",\"args\":[],\"tools\":[\"*\"]}}}\n"
    );
    assert!(
        einstellungen::json::lesen(pi_anbieter("a\"b").as_bytes()).is_ok(),
        "escaped"
    );
    assert_eq!(datei_text("claude", "m", Path::new("/x")), None);
    assert_eq!(datei_ziel("claude"), None);
    assert_eq!(datei_ziel("pi"), Some(".pi/agent/models.json"));
    assert_eq!(datei_ziel("copilot"), Some(".copilot/wb-mcp.json"));
    assert_eq!(
        datei_text("pi", "m", Path::new("/x")),
        Some(pi_anbieter("m"))
    );
    assert_eq!(
        datei_text("copilot", "m", Path::new("/x")),
        Some(copilot_mcp(Path::new("/x")))
    );
    for (h, n) in [
        ("pi", ".pi"),
        ("copilot", ".copilot"),
        ("claude", ".claude"),
    ] {
        assert_eq!(zustand_name(h), n);
    }
}

#[test]
fn the_places_match_the_fixed_places_of_the_enclosure() {
    for (h, ort) in einschluss::DATEI_ZIELE {
        assert_eq!(datei_ziel(h), Some(ort));
    }
}
