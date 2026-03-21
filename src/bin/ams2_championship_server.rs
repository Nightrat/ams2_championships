use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;

use ams2_championship::ams2_shared_memory::read_live_session;

/// Extract the request path from a raw HTTP request buffer.
fn parse_path(buf: &[u8]) -> String {
    let line_end = buf
        .iter()
        .position(|&b| b == b'\r' || b == b'\n')
        .unwrap_or(buf.len());
    let line = std::str::from_utf8(&buf[..line_end]).unwrap_or("");
    // "GET /path HTTP/1.1" → "/path"
    line.split_ascii_whitespace()
        .nth(1)
        .unwrap_or("/")
        .to_owned()
}

fn send_response(stream: &mut TcpStream, status: &str, content_type: &str, body: &[u8]) {
    let header = format!(
        "HTTP/1.1 {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nAccess-Control-Allow-Origin: *\r\nConnection: close\r\n\r\n",
        status,
        content_type,
        body.len()
    );
    let _ = stream.write_all(header.as_bytes());
    let _ = stream.write_all(body);
}

fn handle(mut stream: TcpStream, html: Arc<Vec<u8>>) {
    let mut buf = [0u8; 4096];
    let n = stream.read(&mut buf).unwrap_or(0);
    let path = parse_path(&buf[..n]);

    if path == "/live" {
        let data = read_live_session();
        let json = serde_json::to_vec(&data).unwrap_or_else(|_| b"{}".to_vec());
        send_response(&mut stream, "200 OK", "application/json", &json);
    } else {
        send_response(&mut stream, "200 OK", "text/html; charset=utf-8", &html);
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <path/to/Championships.xml> [port]", args[0]);
        std::process::exit(1);
    }

    let xml_path = &args[1];
    let port: u16 = args.get(2).and_then(|p| p.parse().ok()).unwrap_or(8080);

    let html = match ams2_championship::build_html_from_xml(xml_path) {
        Ok(h) => Arc::new(h.into_bytes()),
        Err(e) => {
            eprintln!("Error: {e}");
            std::process::exit(1);
        }
    };

    let listener = match TcpListener::bind(format!("127.0.0.1:{port}")) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("Failed to bind to port {port}: {e}");
            std::process::exit(1);
        }
    };

    println!("Serving at http://127.0.0.1:{port}/  (Ctrl+C to stop)");
    println!("Live session endpoint: http://127.0.0.1:{port}/live");

    for stream in listener.incoming() {
        if let Ok(stream) = stream {
            let html = Arc::clone(&html);
            std::thread::spawn(move || handle(stream, html));
        }
    }
}
