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
    let header_end = buf
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .map(|p| p + 4)
        .unwrap_or(buf.len());
    let headers = std::str::from_utf8(&buf[..header_end])
        .unwrap_or("")
        .to_owned();
    let body = buf.get(header_end..).unwrap_or(&[]).to_vec();
    Request {
        method,
        path,
        body,
        headers,
    }
}

pub fn send_response(stream: &mut TcpStream, status: &str, content_type: &str, body: &[u8]) {
    send_with_cache(stream, status, content_type, "no-store", body);
}

/// [`send_response`] with the caller's own caching rule.
///
/// Everything else this server answers is live state, which is why the default is `no-store`.
/// A picture decoded out of a file in the game's install is the exception: nothing about it
/// changes between requests, and re-fetching twenty of them every time a table repaints is work
/// nobody asked for.
pub fn send_with_cache(
    stream: &mut TcpStream,
    status: &str,
    content_type: &str,
    cache_control: &str,
    body: &[u8],
) {
    write_response(stream, status, content_type, cache_control, "", body);
}

/// The one place a response is put together. `extra_headers` is either empty or whole
/// `Name: value\r\n` lines.
///
/// No `Access-Control-Allow-Origin` by default: the app's own page is served from here, so it
/// needs no permission to read what it asks for, and granting it to every site let any page the
/// user had open read the career too. [`json_ok_any_origin`] is the exception.
fn write_response(
    stream: &mut TcpStream,
    status: &str,
    content_type: &str,
    cache_control: &str,
    extra_headers: &str,
    body: &[u8],
) {
    let header = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\n\
         Cache-Control: {cache_control}\r\n{extra_headers}Connection: close\r\n\r\n",
        body.len(),
    );
    let _ = stream.write_all(header.as_bytes());
    let _ = stream.write_all(body);
}

/// Answers with a file for the browser to save rather than show, under `file_name`.
pub fn send_download(stream: &mut TcpStream, content_type: &str, file_name: &str, body: &[u8]) {
    // The name is ours (a date and a fixed stem), but a quote in it would end the header value.
    let file_name = file_name.replace('"', "");
    let header = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\n\
         Content-Disposition: attachment; filename=\"{file_name}\"\r\n\
         Cache-Control: no-store\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let _ = stream.write_all(header.as_bytes());
    let _ = stream.write_all(body);
}

pub fn json_ok(stream: &mut TcpStream, body: &[u8]) {
    send_response(stream, "200 OK", "application/json", body);
}

/// [`json_ok`] that a page on any site may read — for the raw `/live` snapshot only, which the
/// README offers to overlays and other tools and which changes nothing when read.
pub fn json_ok_any_origin(stream: &mut TcpStream, body: &[u8]) {
    write_response(
        stream,
        "200 OK",
        "application/json",
        "no-store",
        "Access-Control-Allow-Origin: *\r\n",
        body,
    );
}

/// The value of the request header called `name`, matched in any case, trimmed; `None` when the
/// request did not send one. `headers` is [`Request::headers`], request line included.
pub fn header<'a>(headers: &'a str, name: &str) -> Option<&'a str> {
    headers.lines().skip(1).find_map(|line| {
        let (key, value) = line.split_once(':')?;
        key.trim().eq_ignore_ascii_case(name).then(|| value.trim())
    })
}

/// Whether a browser says this request was sent by a page on another site.
///
/// Any page the user has open can aim a form or a script at this server, and a POST whose body
/// is plain text goes out without the browser asking anyone first — so a page elsewhere could
/// sign contracts, switch careers or reset rosters here. `Origin` is written by the browser, not
/// by the page, and is sent with every request a page makes to another site and with every
/// same-site request that is not a GET. It is compared with `Host`, the address the browser was
/// given for this server — `127.0.0.1`, `localhost` or a LAN address alike — so the app works
/// from whichever of them it was opened at.
///
/// No `Origin` at all is not a page: curl, scripts and the tests send none, and are let through.
/// `Origin: null` — a sandboxed frame, a file opened from disk — is not this app's page either.
pub fn from_another_site(headers: &str) -> bool {
    let Some(origin) = header(headers, "origin") else {
        return false;
    };
    match header(headers, "host") {
        Some(host) => !origin.eq_ignore_ascii_case(&format!("http://{host}")),
        None => true,
    }
}

/// Answers `{"error": msg}`. The message is escaped as a JSON string, so a Windows path (all
/// backslashes) or a quote in it cannot break the body — callers still strip quotes out of habit,
/// which is now harmless.
pub fn json_err(stream: &mut TcpStream, status: &str, msg: &str) {
    let body = serde_json::json!({ "error": msg }).to_string();
    send_response(stream, status, "application/json", body.as_bytes());
}

pub fn read_full_request(stream: &mut TcpStream) -> Request {
    let mut raw: Vec<u8> = Vec::new();
    let mut tmp = [0u8; 8192];

    // Read until we have the full headers (\r\n\r\n).
    loop {
        let n = stream.read(&mut tmp).unwrap_or(0);
        if n == 0 {
            break;
        }
        raw.extend_from_slice(&tmp[..n]);
        if raw.windows(4).any(|w| w == b"\r\n\r\n") {
            break;
        }
    }

    // Parse method and path from the first line.
    let line_end = raw
        .iter()
        .position(|&b| b == b'\r' || b == b'\n')
        .unwrap_or(raw.len());
    let first_line = std::str::from_utf8(&raw[..line_end]).unwrap_or("");
    let mut parts = first_line.split_ascii_whitespace();
    let method = parts.next().unwrap_or("GET").to_owned();
    let path = parts.next().unwrap_or("/").to_owned();

    // Find where the body starts.
    let header_end = raw
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
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
        if n == 0 {
            break;
        }
        body.extend_from_slice(&tmp[..n]);
    }
    body.truncate(content_length);

    Request {
        method,
        path,
        body,
        headers: headers_str.to_owned(),
    }
}

/// Percent-decodes one URL path segment (e.g. "GT3%20Career" → "GT3 Career").
/// Invalid escapes are left as-is.
pub fn url_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or("");
            if let Ok(b) = u8::from_str_radix(hex, 16) {
                out.push(b);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8(out).unwrap_or_else(|_| s.to_string())
}

/// Turns a track name into a safe filename stem (e.g. "Spa – GP" → "spa_gp").
pub fn track_slug(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect::<String>()
        .split('_')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("_")
}
