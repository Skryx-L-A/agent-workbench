//! Tests of the start form of each harness.

use std::path::PathBuf;

use super::*;

fn w(t: &[&str]) -> Vec<String> {
    t.iter().map(|x| (*x).to_owned()).collect()
}

#[test]
#[allow(clippy::too_many_lines)]
fn each_harness_gets_the_argv_of_the_table() {
    let heim = PathBuf::from("/srv/beispiel/heim");
    assert_eq!(
        argv(
            "pi",
            Path::new("/srv/bin/pi"),
            "qwen3:8b",
            &heim,
            &[],
            None,
            &[]
        ),
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
            &[],
            None,
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
        argv(
            "copilot",
            Path::new("/srv/bin/copilot"),
            "auto",
            &heim,
            &[],
            None,
            &[]
        ),
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
            &[],
            None,
            &[]
        ),
        w(&["/srv/bin/claude", "--model", "claude-sonnet-5-5"])
    );
    // the model stays one element, whatever it holds
    let a = argv("pi", Path::new("/p"), "a --b", &heim, &[], None, &[]);
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
        None,
        &[],
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
    assert_eq!(
        argv("pi", Path::new("/p"), "m", &heim, &werkzeuge, None, &[]).len(),
        5
    );
    assert_eq!(
        argv("claude", Path::new("/p"), "m", &heim, &werkzeuge, None, &[]).len(),
        3
    );
    // auto is a word of copilot only
    assert_eq!(
        argv("pi", Path::new("/p"), "auto", &heim, &[], None, &[]).len(),
        5
    );
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

#[test]
fn an_effort_is_exactly_one_pair_right_after_the_model() {
    let heim = PathBuf::from("/srv/beispiel/heim");
    let k = "6f1c2e3a-4b5d-4e6f-8a7b-9c0d1e2f3a4b";
    assert_eq!(
        argv(
            "pi",
            Path::new("/p"),
            "qwen3:8b",
            &heim,
            &[],
            Some("low"),
            &[]
        ),
        w(&[
            "/p",
            "--provider",
            "wb-lokal",
            "--model",
            "qwen3:8b",
            "--thinking",
            "low"
        ])
    );
    assert_eq!(
        argv(
            "claude",
            Path::new("/p"),
            "m",
            &heim,
            &[],
            Some("high"),
            &neu_optionen(k)
        ),
        w(&["/p", "--model", "m", "--effort", "high", "--session-id", k])
    );
    let c = argv(
        "copilot",
        Path::new("/p"),
        "auto",
        &heim,
        &[einstellungen::Werkzeug::Shell],
        Some("medium"),
        &fortsetzen_optionen("copilot", k).unwrap_or_default(),
    );
    assert_eq!(
        c,
        w(&[
            "/p",
            "--model",
            "auto",
            "--reasoning-effort",
            "medium",
            "--auto-tier",
            "efficiency",
            "--disable-builtin-mcps",
            "--additional-mcp-config",
            "@/srv/beispiel/heim/.copilot/wb-mcp.json",
            "--allow-tool",
            "shell",
            &format!("--resume={k}")
        ])
    );
    // without an effort no option comes in
    assert_eq!(
        argv("claude", Path::new("/p"), "m", &heim, &[], None, &[]).len(),
        3
    );
}

#[test]
fn each_harness_has_its_option_to_continue_and_never_the_option_of_a_new_start() {
    let k = "6f1c2e3a-4b5d-4e6f-8a7b-9c0d1e2f3a4b";
    assert_eq!(fortsetzen_optionen("claude", k), Some(w(&["--resume", k])));
    assert_eq!(
        fortsetzen_optionen("copilot", k),
        Some(vec![format!("--resume={k}")])
    );
    assert_eq!(fortsetzen_optionen("pi", k), Some(w(&["--session", k])));
    assert_eq!(fortsetzen_optionen("andere", k), None);
    assert_eq!(neu_optionen(k), w(&["--session-id", k]));
    for (h, o) in [
        ("claude", "--effort"),
        ("copilot", "--reasoning-effort"),
        ("pi", "--thinking"),
    ] {
        assert_eq!(effort_option(h), o);
    }
}
