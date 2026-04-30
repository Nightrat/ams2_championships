use std::io::{Read, Write};
use std::net::TcpStream;

pub struct Request {
    pub method: String,
    pub path: String,
    pub body: Vec<u8>,
    pub headers: String,
}

pub fn parse_request(buf: &[u8]) -> Request {
    let line_end = buf
        .iter()
        .position(|&b| b == b'\r' || b == b'\n')
        .unwrap_or(buf.len());
    let first_line = std::str::from_utf8(&buf[..line_end]).unwrap_or("");
    let mut parts = first_line.split_ascii_whitespace();
    let method = parts.next().unwrap_or("GET").to_owned();
    let path = parts.next().unwrap_or("/").to_owned();
    let header_end = buf.windows(4).position(|w| w == b"\r\n\r\n").map(|p| p + 4).unwrap_or(buf.len());
    let headers = std::str::from_utf8(&buf[..header_end]).unwrap_or("").to_owned();
    let body = buf.get(header_end..).unwrap_or(&[]).to_vec();
    Request { method, path, body, headers }
}

pub fn send_response(stream: &mut TcpStream, status: &str, content_type: &str, body: &[u8]) {
    let header = format!(
        "HTTP/1.1 {}\r\nContent-Type: {}\r\nContent-Length: {}\r\n\
         Cache-Control: no-store\r\nAccess-Control-Allow-Origin: *\r\n\
         Connection: close\r\n\r\n",
        status,
        content_type,
        body.len()
    );
    let _ = stream.write_all(header.as_bytes());
    let _ = stream.write_all(body);
}

pub fn json_ok(stream: &mut TcpStream, body: &[u8]) {
    send_response(stream, "200 OK", "application/json", body);
}

pub fn json_err(stream: &mut TcpStream, status: &str, msg: &str) {
    let body = format!("{{\"error\":\"{msg}\"}}");
    send_response(stream, status, "application/json", body.as_bytes());
}

pub fn read_full_request(stream: &mut TcpStream) -> Request {
    let mut raw: Vec<u8> = Vec::new();
    let mut tmp = [0u8; 8192];

    // Read until we have the full headers (\r\n\r\n).
    loop {
        let n = stream.read(&mut tmp).unwrap_or(0);
        if n == 0 { break; }
        raw.extend_from_slice(&tmp[..n]);
        if raw.windows(4).any(|w| w == b"\r\n\r\n") { break; }
    }

    // Parse method and path from the first line.
    let line_end = raw.iter().position(|&b| b == b'\r' || b == b'\n').unwrap_or(raw.len());
    let first_line = std::str::from_utf8(&raw[..line_end]).unwrap_or("");
    let mut parts = first_line.split_ascii_whitespace();
    let method = parts.next().unwrap_or("GET").to_owned();
    let path   = parts.next().unwrap_or("/").to_owned();

    // Find where the body starts.
    let header_end = raw.windows(4).position(|w| w == b"\r\n\r\n")
        .map(|p| p + 4)
        .unwrap_or(raw.len());

    // Parse Content-Length from headers.
    let headers_str = std::str::from_utf8(&raw[..header_end]).unwrap_or("");
    let content_length: usize = headers_str
        .lines()
        .find(|l| l.to_ascii_lowercase().starts_with("content-length:"))
        .and_then(|l| l.split_once(':').map(|x| x.1))
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(0);

    // Read remaining body bytes until Content-Length is satisfied.
    let mut body = raw[header_end..].to_vec();
    while body.len() < content_length {
        let n = stream.read(&mut tmp).unwrap_or(0);
        if n == 0 { break; }
        body.extend_from_slice(&tmp[..n]);
    }
    body.truncate(content_length);

    Request { method, path, body, headers: headers_str.to_owned() }
}

/// Turns a track name into a safe filename stem (e.g. "Spa – GP" → "spa_gp").
pub fn track_slug(name: &str) -> String {
    name.chars()
        .map(|c| if c.is_alphanumeric() { c.to_ascii_lowercase() } else { '_' })
        .collect::<String>()
        .split('_')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("_")
}
