//! The pure parts of the embedding call: the address, the body of the request, the first vector of
//! an answer. The exchange itself is tested against a stand-in for Ollama in `tests/embed.rs`.

use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr};
use std::time::Duration;

use crate::embed::{
    BODY_MAX, DEFAULT_KEEP_ALIVE, DEFAULT_MODEL, DEFAULT_TIMEOUT_MS, Embedder, Endpoint,
    HEAD_LINE_MAX, HEAD_LINES_MAX, MAX_CHARS, connect_limit, first_vector, json_string,
    parse_endpoint, request_body,
};

fn v4(port: u16) -> SocketAddr {
    SocketAddr::from((Ipv4Addr::LOCALHOST, port))
}

fn v6(port: u16) -> SocketAddr {
    SocketAddr::from((Ipv6Addr::LOCALHOST, port))
}

#[test]
fn the_defaults_are_those_of_the_model() {
    assert_eq!(DEFAULT_MODEL, "embeddinggemma:latest");
    assert_eq!(DEFAULT_KEEP_ALIVE, "30m");
    assert_eq!(DEFAULT_TIMEOUT_MS, 180_000);
    assert_eq!(MAX_CHARS, 8000);
}

#[test]
fn the_limits_of_the_reply_are_these() {
    assert_eq!(HEAD_LINE_MAX, 16_384);
    assert_eq!(HEAD_LINES_MAX, 256);
    assert_eq!(BODY_MAX, 4_194_304);
}

#[test]
fn the_three_loopback_forms_are_accepted() {
    assert_eq!(
        parse_endpoint("http://127.0.0.1:11434"),
        Some(Endpoint {
            addrs: vec![v4(11434)],
            host: "127.0.0.1:11434".to_string()
        })
    );
    assert_eq!(
        parse_endpoint("http://localhost:80"),
        Some(Endpoint {
            addrs: vec![v4(80), v6(80)],
            host: "localhost:80".to_string()
        })
    );
    assert_eq!(
        parse_endpoint("http://[::1]:65535"),
        Some(Endpoint {
            addrs: vec![v6(65535)],
            host: "[::1]:65535".to_string()
        })
    );
    assert_eq!(
        parse_endpoint("http://127.0.0.1:1").map(|e| e.addrs),
        Some(vec![v4(1)])
    );
    assert_eq!(
        parse_endpoint("http://127.0.0.1:007").map(|e| e.addrs),
        Some(vec![v4(7)])
    );
}

#[test]
fn every_other_address_is_refused() {
    for url in [
        "",
        "127.0.0.1:11434",
        "https://127.0.0.1:11434",
        "HTTP://127.0.0.1:11434",
        "http://127.0.0.1",
        "http://127.0.0.1:",
        "http://127.0.0.1:0",
        "http://127.0.0.1:65536",
        "http://127.0.0.1:-1",
        "http://127.0.0.1:+1",
        "http://127.0.0.1: 1",
        "http://127.0.0.1:1x",
        "http://127.0.0.1:1/",
        "http://127.0.0.1:1/api",
        "http://127.0.0.1:1?x=1",
        "http://127.0.0.1:1#x",
        "http://user@127.0.0.1:1",
        "http://127.0.0.2:1",
        "http://127.0.0.1.:1",
        "http://0.0.0.0:1",
        "http://0:1",
        "http://localhost.:1",
        "http://LOCALHOST:1",
        "http://localhost",
        "http://[::1]",
        "http://[::1]:",
        "http://[::2]:1",
        "http://[0:0:0:0:0:0:0:1]:1",
        "http://::1:1",
        "http://[::1:1",
        "http://example.com:1",
        "http://10.0.0.1:1",
        "http://192.168.0.1:1",
        "http://[::ffff:127.0.0.1]:1",
        "http://127.0.0.1:1 ",
        " http://127.0.0.1:1",
    ] {
        assert_eq!(parse_endpoint(url), None, "{url:?}");
    }
}

#[test]
fn an_embedder_needs_a_loopback_address_and_a_deadline_of_at_least_a_millisecond() {
    assert!(Embedder::new("http://127.0.0.1:1", "m", "k", 1).is_some());
    assert!(Embedder::new("http://example.com:1", "m", "k", 1).is_none());
    // 0 is lifted to 1 and a huge number is cut, so that the clock cannot overflow.
    let zero = Embedder::new("http://127.0.0.1:1", "m", "k", 0).expect("embedder");
    assert_eq!(zero.timeout.as_millis(), 1);
    let huge = Embedder::new("http://127.0.0.1:1", "m", "k", u64::MAX).expect("embedder");
    assert_eq!(huge.timeout.as_millis(), 4_294_967_295);
    let exact = Embedder::new("http://127.0.0.1:1", "m", "k", 180_000).expect("embedder");
    assert_eq!(exact.timeout.as_millis(), 180_000);
}

fn quoted(text: &str) -> String {
    let mut out = String::new();
    json_string(text, &mut out);
    out
}

#[test]
fn a_text_is_written_as_python_dumps_it_with_ensure_ascii() {
    assert_eq!(quoted(""), "\"\"");
    assert_eq!(quoted("abc xyz ~"), "\"abc xyz ~\"");
    assert_eq!(quoted("a\"b\\c"), "\"a\\\"b\\\\c\"");
    assert_eq!(quoted("\n\r\t\u{8}\u{c}"), "\"\\n\\r\\t\\b\\f\"");
    assert_eq!(quoted("\u{0}\u{1f}\u{7f}"), "\"\\u0000\\u001f\\u007f\"");
    assert_eq!(quoted("\u{e9}\u{4e2d}"), "\"\\u00e9\\u4e2d\"");
    assert_eq!(quoted("\u{1f600}"), "\"\\ud83d\\ude00\"");
    assert_eq!(quoted("/"), "\"/\"");
    assert_eq!(quoted("\u{ffff}\u{10000}"), "\"\\uffff\\ud800\\udc00\"");
}

#[test]
fn the_body_has_the_keys_of_the_model_in_its_order() {
    assert_eq!(
        request_body("embeddinggemma:latest", "Wie geht\u{27}s?", "30m"),
        "{\"model\": \"embeddinggemma:latest\", \"input\": \"Wie geht's?\", \"keep_alive\": \"30m\"}"
    );
}

#[test]
fn the_question_is_cut_to_8000_code_points() {
    let long = "\u{e9}".repeat(MAX_CHARS + 5);
    let body = request_body("m", &long, "k");
    assert_eq!(body.matches("\\u00e9").count(), MAX_CHARS);
    let exact = "x".repeat(MAX_CHARS);
    assert_eq!(
        request_body("m", &exact, "k").matches('x').count(),
        MAX_CHARS
    );
    let over = "x".repeat(MAX_CHARS + 1);
    assert_eq!(
        request_body("m", &over, "k").matches('x').count(),
        MAX_CHARS
    );
    // Code points, not bytes and not UTF-16 units: an emoji is one.
    let emoji = "\u{1f600}".repeat(MAX_CHARS + 1);
    assert_eq!(
        request_body("m", &emoji, "k")
            .matches("\\ud83d\\ude00")
            .count(),
        MAX_CHARS
    );
}

#[test]
fn the_first_vector_is_a_list_of_numbers_that_is_not_empty() {
    let ok = |text: &str| first_vector(text.as_bytes());
    assert_eq!(
        ok("{\"embeddings\": [[1, 2.5, -3e-1]]}"),
        Some(vec![1.0, 2.5, -0.3])
    );
    assert_eq!(ok("{\"embeddings\": [[0.5], [9, 9]]}"), Some(vec![0.5]));
    assert_eq!(
        ok("{\"embeddings\":[[1]],\"model\":\"m\"}"),
        Some(vec![1.0])
    );
    assert_eq!(
        ok(" {\"x\": 1, \"embeddings\": [[1, 2]]} \n"),
        Some(vec![1.0, 2.0])
    );
    // The last of a repeated key counts; a number is rounded to the nearest double, to the last digit.
    assert_eq!(
        ok("{\"embeddings\": [[1]], \"embeddings\": [[2]]}"),
        Some(vec![2.0])
    );
    assert_eq!(
        ok("{\"embeddings\": [[0.14285714285714285, 8.98846567431158e307, 5e-324]]}"),
        Some(vec![
            0.142_857_142_857_142_85,
            8.988_465_674_311_58e307,
            5e-324
        ])
    );
    // The second entry is not looked at.
    assert_eq!(ok("{\"embeddings\": [[1], \"x\"]}"), Some(vec![1.0]));
    for bad in [
        "",
        "not json",
        "[]",
        "[[1]]",
        "null",
        "{}",
        "{\"embeddings\": null}",
        "{\"embeddings\": []}",
        "{\"embeddings\": {}}",
        "{\"embeddings\": \"x\"}",
        "{\"embeddings\": [[]]}",
        "{\"embeddings\": [null]}",
        "{\"embeddings\": [1]}",
        "{\"embeddings\": [\"x\"]}",
        "{\"embeddings\": [[1, \"2\"]]}",
        "{\"embeddings\": [[1, null]]}",
        "{\"embeddings\": [[true]]}",
        "{\"embeddings\": [[[1]]]}",
        "{\"embeddings\": [[NaN]]}",
        "{\"embeddings\": [[Infinity]]}",
        "{\"embeddings\": [[-Infinity]]}",
        "{\"embeddings\": [[1e999]]}",
        "{\"embeddings\": [[1]]} x",
        "{\"embeddings\": [[1]",
        "{\"vectors\": [[1]]}",
    ] {
        assert_eq!(ok(bad), None, "{bad:?}");
    }
    assert_eq!(first_vector(b"\xff{\"embeddings\": [[1]]}"), None);
}

#[test]
fn the_connect_gets_250_ms_at_most_and_never_more_than_the_rest_of_the_try() {
    let ms = Duration::from_millis;
    // Much is left (the deadline of 180 s, or 3 s): 250 ms.
    assert_eq!(connect_limit(ms(180_000)), ms(250));
    assert_eq!(connect_limit(ms(3000)), ms(250));
    assert_eq!(connect_limit(ms(251)), ms(250));
    assert_eq!(connect_limit(ms(250)), ms(250));
    // Less is left: the rest.
    assert_eq!(connect_limit(ms(249)), ms(249));
    assert_eq!(connect_limit(ms(100)), ms(100));
    assert_eq!(connect_limit(ms(1)), ms(1));
    // Nothing is left: zero, which a socket refuses, so the connect fails at once.
    assert_eq!(connect_limit(Duration::ZERO), Duration::ZERO);
}
