//! The request of the search socket: the fields, `k`, `space`, `id`, and everything that makes a line invalid.

use serde_json::{Value, json};

use crate::sitzung::anfrage::{Anfrage, Op, lesen};

fn suche(query: &str, k: usize, space: Option<&str>) -> Op {
    Op::Search {
        query: query.to_string(),
        k,
        space: space.map(str::to_string),
    }
}

fn gut(zeile: &str) -> Anfrage {
    lesen(zeile.as_bytes()).unwrap_or_else(|id| panic!("{zeile} is invalid (id {id:?})"))
}

/// The `id` that comes back with the answer `bad_request`.
fn schlecht(zeile: &str) -> Option<Value> {
    match lesen(zeile.as_bytes()) {
        Ok(anfrage) => panic!("{zeile} is valid: {anfrage:?}"),
        Err(id) => id,
    }
}

#[test]
fn a_ping_carries_only_op_and_id() {
    assert_eq!(
        gut(r#"{"op":"ping"}"#),
        Anfrage {
            id: None,
            op: Op::Ping
        }
    );
    assert_eq!(gut(r#"{"op":"ping","id":7}"#).id, Some(json!(7)));
    assert_eq!(gut(r#"{"op":"ping","id":"a"}"#).id, Some(json!("a")));
    assert_eq!(gut(r#"{"id":"a","op":"ping"}"#).op, Op::Ping);
    assert_eq!(
        schlecht(r#"{"op":"ping","query":"x","id":3}"#),
        Some(json!(3))
    );
    assert_eq!(schlecht(r#"{"op":"ping","k":1}"#), None);
    assert_eq!(schlecht(r#"{"op":"ping","space":null}"#), None);
}

#[test]
fn a_search_has_a_query_and_defaults() {
    assert_eq!(
        gut(r#"{"op":"search","query":"Lieferplan"}"#).op,
        suche("Lieferplan", 5, None)
    );
    assert_eq!(gut(r#"{"op":"search","query":""}"#).op, suche("", 5, None));
    assert_eq!(
        gut(r#"{"op":"search","query":"x","k":3,"space":"kern","id":1}"#),
        Anfrage {
            id: Some(json!(1)),
            op: suche("x", 3, Some("kern"))
        }
    );
    assert_eq!(
        gut(r#"{"op":"search","query":"x","space":null}"#).op,
        suche("x", 5, None)
    );
    assert_eq!(schlecht(r#"{"op":"search","id":2}"#), Some(json!(2)));
    assert_eq!(
        schlecht(r#"{"op":"search","query":1,"id":2}"#),
        Some(json!(2))
    );
    assert_eq!(schlecht(r#"{"op":"search","query":null}"#), None);
    assert_eq!(schlecht(r#"{"op":"search","query":"x","space":1}"#), None);
    assert_eq!(
        schlecht(r#"{"op":"search","query":"x","space":["a"]}"#),
        None
    );
}

#[test]
fn k_is_a_whole_number_from_1_to_50() {
    for k in [1, 2, 49, 50] {
        assert_eq!(
            gut(&format!(r#"{{"op":"search","query":"x","k":{k}}}"#)).op,
            suche("x", k, None)
        );
    }
    for k in [
        "0",
        "51",
        "-1",
        "5.0",
        "5e0",
        "\"5\"",
        "null",
        "true",
        "[5]",
        "100000000000000000000",
    ] {
        assert_eq!(
            schlecht(&format!(r#"{{"op":"search","query":"x","k":{k},"id":4}}"#)),
            Some(json!(4)),
            "k {k}"
        );
    }
}

#[test]
fn no_field_of_the_route_the_customer_or_the_session_is_valid() {
    for feld in [
        "route",
        "kunde",
        "sitzung",
        "klasse",
        "datenklasse",
        "today",
        "query_vector",
        "vault",
        "x",
    ] {
        assert_eq!(
            schlecht(&format!(
                r#"{{"op":"search","query":"x","{feld}":"v","id":"i"}}"#
            )),
            Some(json!("i")),
            "{feld}"
        );
        assert_eq!(
            schlecht(&format!(r#"{{"op":"ping","{feld}":null}}"#)),
            None,
            "{feld}"
        );
    }
}

#[test]
fn quit_and_unknown_operations_are_invalid() {
    assert_eq!(schlecht(r#"{"op":"quit","id":7}"#), Some(json!(7)));
    assert_eq!(schlecht(r#"{"op":"quit"}"#), None);
    assert_eq!(schlecht(r#"{"op":"Ping"}"#), None);
    assert_eq!(schlecht(r#"{"op":"stats","id":1}"#), Some(json!(1)));
    assert_eq!(schlecht(r#"{"op":1}"#), None);
    assert_eq!(schlecht(r#"{"id":1}"#), Some(json!(1)));
    assert_eq!(schlecht("{}"), None);
}

#[test]
fn a_line_that_is_no_object_or_repeats_a_key_is_invalid() {
    for zeile in [
        "",
        "kein json",
        "[]",
        "1",
        "null",
        "\"x\"",
        "{",
        r#"{"op":"ping"} {"op":"ping"}"#,
    ] {
        assert_eq!(schlecht(zeile), None, "{zeile:?}");
    }
    assert_eq!(schlecht(r#"{"op":"ping","op":"ping"}"#), None);
    assert_eq!(
        schlecht(r#"{"op":"search","query":"a","query":"b","id":5}"#),
        Some(json!(5))
    );
    assert_eq!(schlecht(r#"{"op":"ping","id":1,"id":2}"#), None);
    assert_eq!(
        schlecht(r#"{"op":"search","query":"x","k":1,"k":2,"id":6}"#),
        Some(json!(6))
    );
}

#[test]
fn an_id_is_a_text_or_a_whole_number_and_otherwise_the_request_is_invalid() {
    for id in ["1.5", "null", "true", "[1]", "{}", "1e3"] {
        assert_eq!(
            schlecht(&format!(r#"{{"op":"ping","id":{id}}}"#)),
            None,
            "{id}"
        );
    }
    assert_eq!(gut(r#"{"op":"ping","id":-3}"#).id, Some(json!(-3)));
    assert_eq!(gut(r#"{"op":"ping","id":""}"#).id, Some(json!("")));
}
