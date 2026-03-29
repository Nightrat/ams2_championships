use super::*;

fn req(raw: &[u8]) -> Request {
    parse_request(raw)
}

#[test]
fn test_parse_get_root() {
    let r = req(b"GET / HTTP/1.1\r\nHost: localhost\r\n\r\n");
    assert_eq!(r.method, "GET");
    assert_eq!(r.path, "/");
    assert!(r.body.is_empty());
}

#[test]
fn test_parse_get_api_path() {
    let r = req(b"GET /api/championships HTTP/1.1\r\n\r\n");
    assert_eq!(r.method, "GET");
    assert_eq!(r.path, "/api/championships");
}

#[test]
fn test_parse_get_live() {
    let r = req(b"GET /live HTTP/1.1\r\n\r\n");
    assert_eq!(r.path, "/live");
}

#[test]
fn test_parse_delete_with_id() {
    let r = req(b"DELETE /api/championships/12345 HTTP/1.1\r\n\r\n");
    assert_eq!(r.method, "DELETE");
    assert_eq!(r.path, "/api/championships/12345");
}

#[test]
fn test_parse_delete_session_assignment() {
    let r = req(b"DELETE /api/championships/abc/sessions/xyz HTTP/1.1\r\n\r\n");
    assert_eq!(r.method, "DELETE");
    assert_eq!(r.path, "/api/championships/abc/sessions/xyz");
}

#[test]
fn test_parse_post_with_json_body() {
    let body = b"{\"name\":\"Test Champ\"}";
    let header = format!(
        "POST /api/championships HTTP/1.1\r\nContent-Length: {}\r\n\r\n",
        body.len()
    );
    let mut raw = header.into_bytes();
    raw.extend_from_slice(body);

    let r = req(&raw);
    assert_eq!(r.method, "POST");
    assert_eq!(r.path, "/api/championships");
    assert_eq!(r.body, body);
}

#[test]
fn test_parse_patch_with_body() {
    let body = b"{\"status\":\"Finished\"}";
    let header = format!(
        "PATCH /api/championships/99 HTTP/1.1\r\nContent-Length: {}\r\n\r\n",
        body.len()
    );
    let mut raw = header.into_bytes();
    raw.extend_from_slice(body);

    let r = req(&raw);
    assert_eq!(r.method, "PATCH");
    assert_eq!(r.path, "/api/championships/99");
    assert_eq!(r.body, body);
}

#[test]
fn test_parse_empty_buffer_defaults() {
    let r = req(b"");
    assert_eq!(r.method, "GET");
    assert_eq!(r.path, "/");
    assert!(r.body.is_empty());
}

#[test]
fn test_parse_no_body_after_headers() {
    let r = req(b"GET /api/sessions HTTP/1.1\r\nHost: localhost\r\n\r\n");
    assert!(r.body.is_empty());
}

#[test]
fn test_parse_path_segments_round_session_route() {
    let r = req(b"POST /api/championships/42/rounds/0/sessions/7 HTTP/1.1\r\n\r\n");
    let segs: Vec<&str> = r.path.trim_start_matches('/').split('/').collect();
    assert_eq!(segs, ["api", "championships", "42", "rounds", "0", "sessions", "7"]);
    assert_eq!(segs.len(), 7);
}

#[test]
fn test_parse_path_segments_add_round_route() {
    let r = req(b"POST /api/championships/42/rounds HTTP/1.1\r\n\r\n");
    let segs: Vec<&str> = r.path.trim_start_matches('/').split('/').collect();
    assert_eq!(segs, ["api", "championships", "42", "rounds"]);
    assert_eq!(segs.len(), 4);
}

#[test]
fn test_parse_headers_field_captured() {
    let r = req(b"GET /ws HTTP/1.1\r\nUpgrade: websocket\r\nSec-WebSocket-Key: abc123\r\n\r\n");
    assert!(r.headers.contains("Upgrade: websocket"));
    assert!(r.headers.contains("Sec-WebSocket-Key: abc123"));
}

// ── sha1 ──────────────────────────────────────────────────────────────────────

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[test]
fn test_sha1_empty() {
    // Well-known SHA-1 of empty string
    assert_eq!(hex(&sha1(b"")), "da39a3ee5e6b4b0d3255bfef95601890afd80709");
}

#[test]
fn test_sha1_abc() {
    assert_eq!(hex(&sha1(b"abc")), "a9993e364706816aba3e25717850c26c9cd0d89d");
}

#[test]
fn test_sha1_longer_message() {
    // "The quick brown fox jumps over the lazy dog"
    assert_eq!(
        hex(&sha1(b"The quick brown fox jumps over the lazy dog")),
        "2fd4e1c67a2d28fced849ee1bb76e7391b93eb12"
    );
}

#[test]
fn test_sha1_multichunk() {
    // Input longer than 64 bytes (two SHA-1 blocks)
    let input = b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq";
    assert_eq!(hex(&sha1(input)), "84983e441c3bd26ebaae4aa1f95129e5e54670f1");
}

// ── base64_encode ─────────────────────────────────────────────────────────────

#[test]
fn test_base64_empty() {
    assert_eq!(base64_encode(b""), "");
}

#[test]
fn test_base64_one_byte() {
    assert_eq!(base64_encode(b"f"), "Zg==");
}

#[test]
fn test_base64_two_bytes() {
    assert_eq!(base64_encode(b"fo"), "Zm8=");
}

#[test]
fn test_base64_three_bytes() {
    assert_eq!(base64_encode(b"foo"), "Zm9v");
}

#[test]
fn test_base64_foobar() {
    assert_eq!(base64_encode(b"foobar"), "Zm9vYmFy");
}

#[test]
fn test_base64_man() {
    assert_eq!(base64_encode(b"Man"), "TWFu");
}

// ── ws_accept_key ─────────────────────────────────────────────────────────────

#[test]
fn test_ws_accept_key_rfc6455_example() {
    // Example from RFC 6455 Section 1.3
    assert_eq!(
        ws_accept_key("dGhlIHNhbXBsZSBub25jZQ=="),
        "s3pPLMBiTxaQ9kYGzzhZRbK+xOo="
    );
}

// ── track_slug ────────────────────────────────────────────────────────────────

#[test]
fn test_track_slug_simple() {
    assert_eq!(track_slug("Silverstone"), "silverstone");
}

#[test]
fn test_track_slug_spaces_collapsed() {
    assert_eq!(track_slug("Le Mans"), "le_mans");
}

#[test]
fn test_track_slug_special_chars_collapsed() {
    assert_eq!(track_slug("Spa \u{2013} GP"), "spa_gp");
}

#[test]
fn test_track_slug_multiple_separators() {
    assert_eq!(track_slug("Jerez de la Frontera"), "jerez_de_la_frontera");
}

#[test]
fn test_track_slug_empty() {
    assert_eq!(track_slug(""), "");
}

#[test]
fn test_track_slug_numbers_preserved() {
    assert_eq!(track_slug("Circuit 1"), "circuit_1");
}
