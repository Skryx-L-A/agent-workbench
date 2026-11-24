//! JSON-RPC and MCP: methods, ids, the check of the arguments, the form of the answer.

mod gemeinsam;

use gemeinsam::Attrappe;
use serde_json::{Value, json};
use such_mcp::rpc::{self, antworte};

fn lauf(attrappe: &Attrappe, zeile: &str) -> Option<Value> {
    let antwort = antworte(zeile.as_bytes(), attrappe)?;
    assert!(
        !antwort.contains('\n'),
        "the answer is one line: {antwort:?}"
    );
    Some(serde_json::from_str(&antwort).expect("the answer is JSON"))
}

fn eine(zeile: &str) -> Value {
    lauf(&Attrappe::mit("{}"), zeile).expect("an answer")
}

fn aufruf(argumente: &str) -> String {
    format!(
        r#"{{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{{"name":"brain_suche","arguments":{argumente}}}}}"#
    )
}

fn fehlercode(antwort: &Value) -> i64 {
    assert!(
        antwort.get("result").is_none(),
        "no result with an error: {antwort}"
    );
    antwort["error"]["code"].as_i64().expect("a code")
}

fn text_ergebnis(antwort: &Value, text: &str, ist_fehler: bool) {
    assert_eq!(antwort["jsonrpc"], "2.0");
    assert!(antwort.get("error").is_none(), "{antwort}");
    assert_eq!(
        antwort["result"],
        json!({"content": [{"type": "text", "text": text}], "isError": ist_fehler}),
        "{antwort}"
    );
}

// ---- initialize, ping, list --------------------------------------------------------------------------------

#[test]
fn initialize_answers_the_client_version_when_it_is_known() {
    for fassung in ["2025-11-25", "2025-06-18"] {
        let antwort = eine(&format!(
            r#"{{"jsonrpc":"2.0","id":1,"method":"initialize","params":{{"protocolVersion":"{fassung}","capabilities":{{}},"clientInfo":{{"name":"t","version":"1"}}}}}}"#
        ));
        assert_eq!(antwort["result"]["protocolVersion"], fassung);
    }
}

#[test]
fn initialize_answers_the_newest_version_for_an_unknown_one() {
    for fassung in ["2099-01-01", "2024-11-05", "", "2025-11-25 "] {
        let antwort = eine(&format!(
            r#"{{"jsonrpc":"2.0","id":1,"method":"initialize","params":{{"protocolVersion":"{fassung}"}}}}"#
        ));
        assert_eq!(
            antwort["result"]["protocolVersion"], "2025-11-25",
            "{fassung:?}"
        );
    }
}

#[test]
fn initialize_names_the_tools_capability_and_the_server() {
    let antwort = eine(
        r#"{"jsonrpc":"2.0","id":"a","method":"initialize","params":{"protocolVersion":"2025-11-25"}}"#,
    );
    assert_eq!(antwort["id"], "a");
    assert_eq!(
        antwort["result"],
        json!({
            "protocolVersion": "2025-11-25",
            "capabilities": {"tools": {}},
            "serverInfo": {"name": "such-mcp", "version": env!("CARGO_PKG_VERSION")}
        })
    );
}

#[test]
fn initialize_without_a_text_version_is_invalid_params() {
    for params in [
        String::new(),
        r#","params":{}"#.to_owned(),
        r#","params":[]"#.to_owned(),
        r#","params":{"protocolVersion":5}"#.to_owned(),
        r#","params":{"protocolVersion":null}"#.to_owned(),
    ] {
        let antwort = eine(&format!(
            r#"{{"jsonrpc":"2.0","id":3,"method":"initialize"{params}}}"#
        ));
        assert_eq!(fehlercode(&antwort), -32602, "{params}");
        assert_eq!(antwort["id"], 3);
    }
}

#[test]
fn ping_gives_an_empty_result() {
    let antwort = eine(r#"{"jsonrpc":"2.0","id":9,"method":"ping"}"#);
    assert_eq!(antwort, json!({"jsonrpc": "2.0", "id": 9, "result": {}}));
}

#[test]
fn tools_list_names_exactly_the_one_tool_with_its_fixed_schema() {
    let antwort = eine(r#"{"jsonrpc":"2.0","id":"l","method":"tools/list"}"#);
    let werkzeuge = antwort["result"]["tools"].as_array().expect("a list");
    assert_eq!(werkzeuge.len(), 1);
    assert_eq!(antwort["result"].as_object().expect("an object").len(), 1);
    assert_eq!(werkzeuge[0]["name"], "brain_suche");
    assert!(
        werkzeuge[0]["description"]
            .as_str()
            .is_some_and(|d| !d.is_empty())
    );
    assert_eq!(
        werkzeuge[0]["inputSchema"],
        json!({
            "type": "object",
            "properties": {"query": {"type": "string"}, "k": {"type": "integer", "minimum": 1, "maximum": 50}},
            "required": ["query"],
            "additionalProperties": false
        })
    );
}

// ---- ids, notifications, errors of the frame -------------------------------------------------------------

#[test]
fn the_id_comes_back_unchanged() {
    for id in [
        "1",
        "0",
        "-7",
        "\"x\"",
        "\"\"",
        "\"ä\\n\"",
        "9007199254740993",
    ] {
        let antwort = eine(&format!(r#"{{"jsonrpc":"2.0","id":{id},"method":"ping"}}"#));
        let erwartet: Value = serde_json::from_str(id).expect("an id");
        assert_eq!(antwort["id"], erwartet, "{id}");
    }
}

#[test]
fn a_message_without_id_gets_no_answer_and_no_question() {
    let attrappe = Attrappe::mit("{}");
    for zeile in [
        r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
        r#"{"jsonrpc":"2.0","method":"ping"}"#,
        r#"{"jsonrpc":"2.0","method":"nicht/da"}"#,
        r#"{"jsonrpc":"2.0","method":"initialize","params":{}}"#,
        r#"{"jsonrpc":"2.0","method":"tools/call","params":{"name":"brain_suche","arguments":{"query":"x"}}}"#,
    ] {
        assert!(lauf(&attrappe, zeile).is_none(), "{zeile}");
    }
    assert!(attrappe.gefragt.borrow().is_empty());
}

#[test]
fn an_unknown_method_is_method_not_found() {
    for methode in ["resources/list", "prompts/list", "", "Ping", "tools/list "] {
        let antwort = eine(&format!(
            r#"{{"jsonrpc":"2.0","id":2,"method":"{methode}"}}"#
        ));
        assert_eq!(fehlercode(&antwort), -32601, "{methode:?}");
        assert_eq!(antwort["id"], 2);
        assert_eq!(antwort["jsonrpc"], "2.0");
    }
}

#[test]
fn a_line_without_valid_json_is_a_parse_error_with_a_null_id() {
    let attrappe = Attrappe::mit("{}");
    for zeile in [
        &b"kein json"[..],
        b" ",
        b"\r",
        b"\t",
        b"{",
        b"{\"a\":1} x",
        b"\xff\xfe",
        b"{\"method\":\"ping\",\"id\":1,}",
    ] {
        let antwort = antworte(zeile, &attrappe).expect("an answer");
        let antwort: Value = serde_json::from_str(&antwort).expect("JSON");
        assert_eq!(fehlercode(&antwort), -32700, "{zeile:?}");
        assert_eq!(antwort["id"], Value::Null);
        assert_eq!(antwort["jsonrpc"], "2.0");
    }
}

#[test]
fn a_message_that_is_not_a_request_is_invalid_request() {
    for zeile in [
        "[]",
        "[{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"ping\"}]",
        "5",
        "\"x\"",
        "null",
        "{}",
        r#"{"id":4,"method":"ping"}"#,
        r#"{"jsonrpc":"1.0","id":4,"method":"ping"}"#,
        r#"{"jsonrpc":2.0,"id":4,"method":"ping"}"#,
        r#"{"jsonrpc":"2.0","id":4}"#,
        r#"{"jsonrpc":"2.0","id":4,"method":5}"#,
        r#"{"jsonrpc":"2.0","id":null,"method":"ping"}"#,
        r#"{"jsonrpc":"2.0","id":true,"method":"ping"}"#,
        r#"{"jsonrpc":"2.0","id":{"a":1},"method":"ping"}"#,
        r#"{"jsonrpc":"2.0","id":[1],"method":"ping"}"#,
    ] {
        let antwort = eine(zeile);
        assert_eq!(fehlercode(&antwort), -32600, "{zeile}");
    }
}

#[test]
fn an_invalid_request_with_a_valid_id_keeps_the_id() {
    for (zeile, id) in [
        (r#"{"id":4,"method":"ping"}"#, json!(4)),
        (r#"{"jsonrpc":"1.0","id":"s","method":"ping"}"#, json!("s")),
        (r#"{"jsonrpc":"2.0","id":4}"#, json!(4)),
        (r#"{"jsonrpc":"2.0","id":5,"method":7}"#, json!(5)),
    ] {
        assert_eq!(eine(zeile)["id"], id, "{zeile}");
    }
    for zeile in ["[]", "{}", r#"{"jsonrpc":"2.0","id":null,"method":"ping"}"#] {
        assert_eq!(eine(zeile)["id"], Value::Null, "{zeile}");
    }
}

#[test]
fn an_answer_of_the_client_gets_no_answer() {
    let attrappe = Attrappe::mit("{}");
    for zeile in [
        r#"{"jsonrpc":"2.0","id":1,"result":{}}"#,
        r#"{"jsonrpc":"2.0","id":1,"error":{"code":-1,"message":"x"}}"#,
        r#"{"jsonrpc":"2.0","result":{}}"#,
    ] {
        assert!(lauf(&attrappe, zeile).is_none(), "{zeile}");
    }
}

// ---- contract 4, clarifications 7 to 9 (20.11.) ------------------------------------------------------------

/// Clarification 7: a JSON-RPC batch (an array on one line) is one error -32600 with a null id; nobody is asked.
#[test]
fn clarification_7_a_batch_is_one_error_with_a_null_id_and_asks_nobody() {
    let attrappe = Attrappe::mit("{}");
    for zeile in [
        r#"[{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"brain_suche","arguments":{"query":"x"}}}]"#,
        r#"[{"jsonrpc":"2.0","id":1,"method":"ping"},{"jsonrpc":"2.0","id":2,"method":"ping"}]"#,
        r#"[{"jsonrpc":"2.0","method":"notifications/initialized"}]"#,
        "[]",
        "[[]]",
        "[1]",
    ] {
        let antwort = antworte(zeile.as_bytes(), &attrappe).expect("one answer");
        assert!(!antwort.contains('\n'));
        let antwort: Value = serde_json::from_str(&antwort).expect("JSON");
        assert!(
            antwort.is_object(),
            "a single answer, not an array: {antwort}"
        );
        assert_eq!(fehlercode(&antwort), -32600, "{zeile}");
        assert_eq!(antwort["id"], Value::Null);
        assert_eq!(antwort["jsonrpc"], "2.0");
    }
    assert!(attrappe.gefragt.borrow().is_empty());
}

/// Clarification 8: `tools/call` without an id is a notification: no answer and the socket is not asked.
#[test]
fn clarification_8_a_call_without_an_id_gets_no_answer_and_asks_nobody() {
    let attrappe = Attrappe::mit("{}");
    for zeile in [
        r#"{"jsonrpc":"2.0","method":"tools/call","params":{"name":"brain_suche","arguments":{"query":"x"}}}"#,
        r#"{"jsonrpc":"2.0","method":"tools/call","params":{"name":"brain_suche","arguments":{"query":"x","k":3}}}"#,
        r#"{"jsonrpc":"2.0","method":"tools/call","params":{"name":"brain_suche","arguments":{"query":"x","route":"lokal"}}}"#,
        r#"{"jsonrpc":"2.0","method":"tools/call","params":{"name":"anderes"}}"#,
        r#"{"jsonrpc":"2.0","method":"tools/call"}"#,
    ] {
        assert!(lauf(&attrappe, zeile).is_none(), "{zeile}");
    }
    assert!(attrappe.gefragt.borrow().is_empty());
}

/// Clarification 9: `k` is a JSON whole number without a fraction; `1.0`, `"5"` and `true` are invalid.
#[test]
fn clarification_9_k_is_a_whole_number_without_a_fraction() {
    for k in [
        "1.0", "5.0", "\"5\"", "true", "false", "null", "2.5", "1e0", "5E0", "[5]",
    ] {
        ungueltig(&format!(r#"{{"query":"x","k":{k}}}"#));
    }
    for k in ["1", "5", "50"] {
        let attrappe = Attrappe::mit("{}");
        let antwort =
            lauf(&attrappe, &aufruf(&format!(r#"{{"query":"x","k":{k}}}"#))).expect("an answer");
        text_ergebnis(&antwort, "{}", false);
        assert_eq!(attrappe.gefragt.borrow().len(), 1, "k {k}");
    }
}

/// Clarification 9: `query` is text; every other type is invalid without a call of the socket.
#[test]
fn clarification_9_query_is_text() {
    for query in [
        "5",
        "1.5",
        "true",
        "null",
        "[\"x\"]",
        "{\"a\":1}",
        "[]",
        "{}",
    ] {
        ungueltig(&format!(r#"{{"query":{query}}}"#));
        ungueltig(&format!(r#"{{"query":{query},"k":3}}"#));
    }
}

/// Clarification 9: an invalid argument is a tool result with `isError`, not a JSON-RPC error, and gives the fixed text.
#[test]
fn clarification_9_an_invalid_argument_is_a_tool_result_with_is_error() {
    let attrappe = Attrappe::mit("{}");
    for argumente in [
        r#"{"query":"x","route":"lokal"}"#,
        r#"{"query":"x","k":1.0}"#,
        r#"{"query":5}"#,
        r#"{"k":3}"#,
    ] {
        let antwort = lauf(&attrappe, &aufruf(argumente)).expect("an answer");
        assert!(antwort.get("error").is_none(), "{antwort}");
        text_ergebnis(&antwort, "Ungültige Anfrage", true);
    }
    assert!(attrappe.gefragt.borrow().is_empty());
}

/// Clarification 9: an empty line on stdin is skipped without an answer; a blank line that is not empty is no JSON.
#[test]
fn clarification_9_an_empty_line_gets_no_answer_and_asks_nobody() {
    let attrappe = Attrappe::mit("{}");
    assert_eq!(antworte(b"", &attrappe), None);
    assert!(attrappe.gefragt.borrow().is_empty());
}

#[test]
fn the_too_long_answer_is_an_invalid_request_with_a_null_id() {
    let antwort: Value = serde_json::from_str(&rpc::zu_lang()).expect("JSON");
    assert_eq!(fehlercode(&antwort), -32600);
    assert_eq!(antwort["id"], Value::Null);
}

// ---- tools/call: errors of the frame ---------------------------------------------------------------------

#[test]
fn a_call_with_a_wrong_frame_is_invalid_params_and_asks_nobody() {
    let attrappe = Attrappe::mit("{}");
    for params in [
        String::new(),
        r#","params":null"#.to_owned(),
        r#","params":[]"#.to_owned(),
        r#","params":"brain_suche""#.to_owned(),
        r#","params":{"arguments":{"query":"x"}}"#.to_owned(),
        r#","params":{"name":"datei_lesen","arguments":{"query":"x"}}"#.to_owned(),
        r#","params":{"name":"Brain_suche","arguments":{"query":"x"}}"#.to_owned(),
        r#","params":{"name":"brain_suche ","arguments":{"query":"x"}}"#.to_owned(),
        r#","params":{"name":"","arguments":{"query":"x"}}"#.to_owned(),
        r#","params":{"name":5,"arguments":{"query":"x"}}"#.to_owned(),
        r#","params":{"name":null,"arguments":{"query":"x"}}"#.to_owned(),
    ] {
        let antwort = lauf(
            &attrappe,
            &format!(r#"{{"jsonrpc":"2.0","id":4,"method":"tools/call"{params}}}"#),
        )
        .expect("an answer");
        assert_eq!(fehlercode(&antwort), -32602, "{params}");
        assert_eq!(antwort["id"], 4);
    }
    assert!(attrappe.gefragt.borrow().is_empty());
}

// ---- tools/call: the arguments -----------------------------------------------------------------------------

#[test]
fn a_valid_call_hands_on_the_query_and_nothing_else() {
    let attrappe = Attrappe::mit("{}");
    lauf(&attrappe, &aufruf(r#"{"query":"Wartung"}"#)).expect("an answer");
    lauf(&attrappe, &aufruf(r#"{"query":""}"#)).expect("an answer");
    assert_eq!(
        *attrappe.gefragt.borrow(),
        [
            r#"{"op":"search","query":"Wartung"}"#,
            r#"{"op":"search","query":""}"#
        ]
    );
}

#[test]
fn k_goes_along_only_when_it_is_given() {
    for k in [1, 2, 5, 49, 50] {
        let attrappe = Attrappe::mit("{}");
        lauf(&attrappe, &aufruf(&format!(r#"{{"k":{k},"query":"x"}}"#))).expect("an answer");
        assert_eq!(
            *attrappe.gefragt.borrow(),
            [format!(r#"{{"op":"search","query":"x","k":{k}}}"#)]
        );
    }
}

#[test]
fn the_query_is_handed_on_as_one_line_with_its_text_intact() {
    for query in [
        "a\nb",
        "a\"b\\c",
        "\u{0}\u{1f}",
        "Größe 日本語 \u{1f600}",
        "\r\n",
        "{\"op\":\"quit\"}\n{\"op\":\"ping\"}",
    ] {
        let attrappe = Attrappe::mit("{}");
        let argumente = json!({"query": query}).to_string();
        lauf(&attrappe, &aufruf(&argumente)).expect("an answer");
        let gefragt = attrappe.gefragt.borrow();
        assert_eq!(gefragt.len(), 1);
        assert!(
            !gefragt[0].contains('\n') && !gefragt[0].contains('\r'),
            "{:?}",
            gefragt[0]
        );
        let zeile: Value = serde_json::from_str(&gefragt[0]).expect("JSON");
        assert_eq!(zeile, json!({"op": "search", "query": query}));
    }
}

fn ungueltig(argumente: &str) {
    let attrappe = Attrappe::mit("{}");
    let antwort = lauf(&attrappe, &aufruf(argumente)).expect("an answer");
    text_ergebnis(&antwort, "Ungültige Anfrage", true);
    assert_eq!(antwort["id"], 1);
    assert!(
        attrappe.gefragt.borrow().is_empty(),
        "the socket was asked for {argumente}"
    );
}

#[test]
fn every_argument_besides_query_and_k_makes_the_call_invalid() {
    for argumente in [
        r#"{"query":"x","route":"lokal"}"#,
        r#"{"route":"lokal","query":"x"}"#,
        r#"{"query":"x","kunde":"k-nord"}"#,
        r#"{"query":"x","sitzung":"s"}"#,
        r#"{"query":"x","klasse":"intern"}"#,
        r#"{"query":"x","datenklasse":"intern"}"#,
        r#"{"query":"x","space":null}"#,
        r#"{"query":"x","op":"search"}"#,
        r#"{"query":"x","vault":"/"}"#,
        r#"{"query":"x","k":3,"route":"lokal"}"#,
        r#"{"query":"x","Query":"y"}"#,
        r#"{"query":"x","":1}"#,
        r#"{"queryx":"x"}"#,
        r#"{"route":"lokal"}"#,
    ] {
        ungueltig(argumente);
    }
}

#[test]
fn a_k_outside_one_to_fifty_or_not_a_whole_number_makes_the_call_invalid() {
    for k in [
        "0",
        "51",
        "-1",
        "100",
        "1000000",
        "9223372036854775807",
        "9223372036854775808",
        "18446744073709551615",
        "-9223372036854775808",
        "\"5\"",
        "\"\"",
        "3.0",
        "3.5",
        "1e1",
        "0.5",
        "50.0",
        "null",
        "true",
        "false",
        "[3]",
        "{\"a\":3}",
    ] {
        ungueltig(&format!(r#"{{"query":"x","k":{k}}}"#));
    }
}

#[test]
fn a_missing_or_wrong_query_makes_the_call_invalid() {
    for argumente in [
        "{}",
        r#"{"k":5}"#,
        r#"{"query":5}"#,
        r#"{"query":null}"#,
        r#"{"query":["x"]}"#,
        r#"{"query":{"a":"x"}}"#,
        r#"{"query":true}"#,
        "null",
        "[]",
        r#"["x"]"#,
        "\"x\"",
        "5",
    ] {
        ungueltig(argumente);
    }
}

#[test]
fn a_call_without_arguments_is_invalid() {
    let attrappe = Attrappe::mit("{}");
    let antwort = lauf(
        &attrappe,
        r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"brain_suche"}}"#,
    )
    .expect("an answer");
    text_ergebnis(&antwort, "Ungültige Anfrage", true);
    assert!(attrappe.gefragt.borrow().is_empty());
}

// ---- tools/call: the answer --------------------------------------------------------------------------------

#[test]
fn the_answer_line_comes_back_unchanged_as_text() {
    for antwort in [
        r#"{"fallback":true,"hits":[],"gesperrt":1}"#,
        r#"{"hits":[{"rel":"a\"b","titel":"Größe ä"}],"gesperrt":0,"fallback":false}"#,
        r#"{ "ok" : true }"#,
        r#"{"x":{"error":"nur verschachtelt"}}"#,
        r#"{"errors":1}"#,
    ] {
        let attrappe = Attrappe::mit(antwort);
        let ergebnis = lauf(&attrappe, &aufruf(r#"{"query":"x"}"#)).expect("an answer");
        text_ergebnis(&ergebnis, antwort, false);
        assert_eq!(attrappe.gefragt.borrow().len(), 1);
    }
}

/// JSON that is no object is marked as an error as well (fail closed, decision of 23.11.2026); its text comes back unchanged.
/// Until 23.11.2026 these four answers stood in `the_answer_line_comes_back_unchanged_as_text` with `isError: false`.
#[test]
fn json_that_is_no_object_comes_back_unchanged_and_is_marked_as_an_error() {
    for antwort in [
        "[1,2]",
        "5",
        r#"[{"error":"in einer Liste"}]"#,
        r#""error""#,
        "null",
        "true",
    ] {
        let attrappe = Attrappe::mit(antwort);
        let ergebnis = lauf(&attrappe, &aufruf(r#"{"query":"x"}"#)).expect("an answer");
        text_ergebnis(&ergebnis, antwort, true);
        assert_eq!(attrappe.gefragt.borrow().len(), 1);
    }
}

/// An answer without valid JSON is marked as an error (fail closed, decision of 23.11.2026); its text comes back unchanged. Until
/// 23.11.2026 these three answers stood in `the_answer_line_comes_back_unchanged_as_text` with `isError: false`.
#[test]
fn an_answer_without_valid_json_comes_back_unchanged_and_is_marked_as_an_error() {
    for antwort in ["kein json", "", "mit \\ und \" und \t", r#"{"hits":["#] {
        let attrappe = Attrappe::mit(antwort);
        let ergebnis = lauf(&attrappe, &aufruf(r#"{"query":"x"}"#)).expect("an answer");
        text_ergebnis(&ergebnis, antwort, true);
        assert_eq!(attrappe.gefragt.borrow().len(), 1);
    }
}

#[test]
fn an_answer_with_an_error_field_is_marked_as_an_error() {
    for antwort in [
        r#"{"error":"protokoll"}"#,
        r#"{"error":"bad_request","id":3}"#,
        r#"{"id":3,"error":"unreadable"}"#,
        r#"{"error":null}"#,
        r#"{"error":false}"#,
        r#"{"fallback":true,"hits":[],"gesperrt":0,"error":"x"}"#,
    ] {
        let ergebnis =
            lauf(&Attrappe::mit(antwort), &aufruf(r#"{"query":"x"}"#)).expect("an answer");
        text_ergebnis(&ergebnis, antwort, true);
    }
}

#[test]
fn no_answer_of_the_socket_gives_the_fixed_text_as_an_error() {
    let attrappe = Attrappe::unerreichbar();
    let ergebnis = lauf(&attrappe, &aufruf(r#"{"query":"x"}"#)).expect("an answer");
    text_ergebnis(&ergebnis, "Suche nicht erreichbar", true);
    assert_eq!(ergebnis["id"], 1);
    assert_eq!(attrappe.gefragt.borrow().len(), 1);
}

#[test]
fn every_call_asks_the_socket_once_in_order() {
    let attrappe = Attrappe::mit("{}");
    for query in ["a", "b", "c"] {
        lauf(&attrappe, &aufruf(&format!(r#"{{"query":"{query}"}}"#))).expect("an answer");
    }
    assert_eq!(
        *attrappe.gefragt.borrow(),
        [
            r#"{"op":"search","query":"a"}"#,
            r#"{"op":"search","query":"b"}"#,
            r#"{"op":"search","query":"c"}"#
        ]
    );
}
