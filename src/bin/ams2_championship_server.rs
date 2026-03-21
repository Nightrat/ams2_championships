use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;

fn handle(mut stream: TcpStream, html: &[u8]) {
    // Drain the HTTP request headers (we serve the same HTML for every request)
    let mut buf = [0u8; 4096];
    let _ = stream.read(&mut buf);

    let header = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        html.len()
    );
    let _ = stream.write_all(header.as_bytes());
    let _ = stream.write_all(html);
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

    for stream in listener.incoming() {
        if let Ok(stream) = stream {
            let html = Arc::clone(&html);
            std::thread::spawn(move || handle(stream, &html));
        }
    }
}
