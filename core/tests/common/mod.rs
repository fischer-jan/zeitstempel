//! A tiny HTTP server on 127.0.0.1 that serves the recorded responses in
//! `tests/fixtures/http/`, so the integration tests never touch the live
//! Bitcoin APIs or calendar servers.
//!
//! The binary under test is pointed at it through the
//! `ZEITSTEMPEL_BITCOIN_API` and `ZEITSTEMPEL_CALENDARS` overrides. The
//! server records every request it answers, so a test can assert that
//! the fixture was really used.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;

/// Block 358391 holds the hello-world proof (`tests/fixtures/hello-world.txt.ots`).
pub const HELLO_WORLD_BLOCK_HASH: &str =
    "000000000000000003e892881a8cdcdc117c06d444057c98b6f04a9ee75a2319";

pub struct FixtureServer {
    pub base_url: String,
    requests: Arc<Mutex<Vec<String>>>,
}

impl FixtureServer {
    /// Start a server with the recorded routes, on a free port.
    pub fn start() -> FixtureServer {
        let fixture = |name: &str| {
            std::fs::read(format!("tests/fixtures/http/{}", name))
                .unwrap_or_else(|e| panic!("missing fixture {}: {}", name, e))
        };
        let routes: Vec<(String, Vec<u8>)> = vec![
            (
                "GET /block-height/358391".into(),
                fixture("block-height-358391.txt"),
            ),
            (
                format!("GET /block/{}", HELLO_WORLD_BLOCK_HASH),
                fixture("block-358391.json"),
            ),
            // Calendars only check that the body is a valid timestamp
            // tree, so one recorded response fits any submitted digest.
            (
                "POST /digest".into(),
                fixture("calendar-digest-response.bin"),
            ),
        ];

        let listener = TcpListener::bind("127.0.0.1:0").expect("bind fixture server");
        let base_url = format!("http://{}", listener.local_addr().unwrap());
        let requests = Arc::new(Mutex::new(Vec::new()));
        let log = Arc::clone(&requests);
        thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                serve(stream, &routes, &log);
            }
        });
        FixtureServer { base_url, requests }
    }

    /// Requests answered so far, as "METHOD /path".
    pub fn requests(&self) -> Vec<String> {
        self.requests.lock().unwrap().clone()
    }
}

/// Answer one request, then close the connection.
fn serve(stream: TcpStream, routes: &[(String, Vec<u8>)], log: &Mutex<Vec<String>>) {
    let mut reader = BufReader::new(&stream);
    let mut request_line = String::new();
    if reader.read_line(&mut request_line).is_err() {
        return;
    }
    // Read the headers, then the full body, before replying — answering
    // a POST before its body is read can reset the connection.
    let mut content_length = 0usize;
    loop {
        let mut header = String::new();
        if reader.read_line(&mut header).is_err() || header == "\r\n" || header.is_empty() {
            break;
        }
        if let Some((name, value)) = header.split_once(':')
            && name.eq_ignore_ascii_case("content-length")
        {
            content_length = value.trim().parse().unwrap_or(0);
        }
    }
    let mut body = vec![0u8; content_length];
    if reader.read_exact(&mut body).is_err() {
        return;
    }

    let mut parts = request_line.split_whitespace();
    let key = format!(
        "{} {}",
        parts.next().unwrap_or(""),
        parts.next().unwrap_or("")
    );
    log.lock().unwrap().push(key.clone());

    let (status, payload): (&str, &[u8]) = match routes.iter().find(|(route, _)| *route == key) {
        Some((_, payload)) => ("200 OK", payload),
        None => ("404 Not Found", b"no fixture for this request"),
    };
    let mut stream = stream;
    let header = format!(
        "HTTP/1.1 {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        status,
        payload.len()
    );
    let _ = stream.write_all(header.as_bytes());
    let _ = stream.write_all(payload);
    let _ = stream.flush();
}
