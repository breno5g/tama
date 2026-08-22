//! HTTP ingestion: any project on the LAN POSTs one flat JSON object (same
//! schema as the pipe) and it lands in the same channel the FIFO feeds. A
//! message with `actions` long-polls until the user answers in the TUI.
//!
//! Hand-rolled HTTP/1.1 over std::net — one dependency stays one dependency.

use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::Sender;
use std::time::Duration;

use crate::assistant::{self, Msg};
use crate::i18n;

const BODY_CAP: usize = 64 * 1024;
const DEFAULT_ADDR: &str = "0.0.0.0:8262"; // 8262 = "TAMA" num teclado numérico
const READ_TIMEOUT: Duration = Duration::from_secs(10);
const ASK_DEFAULT_TTL: u64 = 300; // asks sem `expires` nunca seguram uma thread pra sempre

static ASK_SEQ: AtomicU64 = AtomicU64::new(0);

pub struct Request {
    pub method: String,
    pub path: String,
    pub auth: Option<String>,
    pub body: String,
}

// Minimal request reader: request line, Content-Length/Authorization headers
// (case-insensitive), then exactly Content-Length body bytes. None on
// anything malformed or oversized.
pub fn parse_request(r: &mut impl BufRead) -> Option<Request> {
    let mut line = String::new();
    r.read_line(&mut line).ok()?;
    let mut parts = line.split_whitespace();
    let method = parts.next()?.to_string();
    let path = parts.next()?.to_string();
    let mut len = 0usize;
    let mut auth = None;
    loop {
        let mut h = String::new();
        if r.read_line(&mut h).ok()? == 0 {
            return None; // connection closed mid-headers
        }
        let h = h.trim_end();
        if h.is_empty() {
            break;
        }
        let Some((k, v)) = h.split_once(':') else { continue };
        match k.to_ascii_lowercase().as_str() {
            "content-length" => len = v.trim().parse().ok()?,
            "authorization" => auth = Some(v.trim().to_string()),
            _ => {}
        }
    }
    if len > BODY_CAP {
        return None;
    }
    let mut body = vec![0u8; len];
    r.read_exact(&mut body).ok()?;
    Some(Request { method, path, auth, body: String::from_utf8(body).ok()? })
}

pub fn response(status: &str, body: &str) -> String {
    format!(
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
}

fn json_err(text: &str) -> String {
    format!("{{\"error\":\"{}\"}}", assistant::json_escape(text))
}

// Splices `,"key":raw` before the final `}` — safe because the wire is flat
// single-object lines only.
pub fn inject(line: &str, key: &str, raw: &str) -> String {
    match line.trim_end().strip_suffix('}') {
        Some(head) => format!("{head},\"{key}\":{raw}}}"),
        None => line.to_string(),
    }
}

// POST /: forwards the body to the app. Say/command answer immediately; an
// ask long-polls the output file exactly like `tama ask` does.
fn post(body: String, tx: &Sender<String>) -> String {
    let now = assistant::now_epoch();
    let msgs = assistant::parse_msgs(&body, now);
    if msgs.is_empty() {
        return response("400 Bad Request", &json_err(i18n::t().http_err_bad));
    }
    if !msgs.iter().any(|m| matches!(m, Msg::Ask { .. })) {
        if tx.send(body).is_err() {
            return response("500 Internal Server Error", &json_err(i18n::t().http_err_bad));
        }
        return response("200 OK", "{\"ok\":true}");
    }

    // Ask: guarantee a unique id (the parser default `ask-{now}` collides
    // within a second) and an expiry (so this thread can't leak) by splicing
    // the fields into the forwarded line when the caller omitted them.
    let mut line = body;
    let fields = assistant::json_fields(&line).unwrap_or_default();
    let has = |k: &str| fields.iter().any(|(key, _)| key == k);
    let id = match fields.iter().find(|(k, _)| k == "id") {
        Some((_, v)) => v.clone(),
        None => {
            let id = format!("http-{now}-{}", ASK_SEQ.fetch_add(1, Ordering::Relaxed));
            line = inject(&line, "id", &format!("\"{}\"", assistant::json_escape(&id)));
            id
        }
    };
    let expires = match fields.iter().find(|(k, _)| k == "expires").and_then(|(_, v)| v.parse().ok()) {
        Some(e) => e,
        None => {
            let e = now + ASK_DEFAULT_TTL;
            if !has("expires") {
                line = inject(&line, "expires", &e.to_string());
            }
            e
        }
    };
    let offset = std::fs::metadata(crate::state::output_path()).map(|m| m.len() as usize).unwrap_or(0);
    if tx.send(line).is_err() {
        return response("500 Internal Server Error", &json_err(i18n::t().http_err_bad));
    }
    match assistant::wait_answer(&id, offset, Some(expires)) {
        Some(answer) => {
            response("200 OK", &format!("{{\"answer\":\"{}\"}}", assistant::json_escape(&answer)))
        }
        None => response("408 Request Timeout", "{\"answer\":null}"),
    }
}

fn handle(mut stream: TcpStream, tx: &Sender<String>) {
    let _ = stream.set_read_timeout(Some(READ_TIMEOUT));
    let Ok(clone) = stream.try_clone() else { return };
    let mut reader = BufReader::new(clone);
    let reply = match parse_request(&mut reader) {
        None => response("400 Bad Request", &json_err(i18n::t().http_err_bad)),
        Some(req) => {
            let authorized = match std::env::var("TAMA_TOKEN") {
                Ok(token) => req.auth.as_deref() == Some(&format!("Bearer {token}")),
                Err(_) => true, // sem TAMA_TOKEN o endpoint é aberto (LAN doméstica)
            };
            if !authorized {
                response("401 Unauthorized", &json_err(i18n::t().http_err_token))
            } else {
                match (req.method.as_str(), req.path.as_str()) {
                    ("GET", "/") => {
                        let pet = crate::state::load().map(|p| p.name).unwrap_or_default();
                        response("200 OK", &format!("{{\"ok\":true,\"pet\":\"{}\"}}", assistant::json_escape(&pet)))
                    }
                    ("POST", "/") => post(req.body, tx),
                    _ => response("404 Not Found", &json_err(i18n::t().http_err_not_found)),
                }
            }
        }
    };
    let _ = stream.write_all(reply.as_bytes());
}

pub fn serve(listener: TcpListener, tx: Sender<String>) {
    for stream in listener.incoming() {
        let Ok(stream) = stream else { continue };
        let tx = tx.clone();
        // ponytail: thread por conexão sem pool — long-polls seguram conexões;
        // pooling só se um flood na LAN importar um dia
        std::thread::spawn(move || handle(stream, &tx));
    }
}

// Binds and serves in the background; returns the ready-to-log status line
// (listening / disabled / failed) so a broken bind is never silent.
pub fn spawn_http(tx: Sender<String>) -> String {
    let addr = std::env::var("TAMA_HTTP").unwrap_or_else(|_| DEFAULT_ADDR.to_string());
    if addr == "off" {
        return i18n::t().msg_http_off.to_string();
    }
    match TcpListener::bind(&addr) {
        Ok(listener) => {
            std::thread::spawn(move || serve(listener, tx));
            i18n::msg_http_on(&addr)
        }
        Err(e) => i18n::msg_http_fail(&addr, &e.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Cursor, Read};
    use std::sync::mpsc::channel;

    fn req(raw: &str) -> Option<Request> {
        parse_request(&mut Cursor::new(raw.as_bytes().to_vec()))
    }

    #[test]
    fn parse_request_reads_body_and_headers_case_insensitively() {
        let r = req("POST / HTTP/1.1\r\ncOnTeNt-LeNgTh: 4\r\nAuthorization: Bearer x\r\n\r\nabcd").unwrap();
        assert_eq!((r.method.as_str(), r.path.as_str()), ("POST", "/"));
        assert_eq!(r.auth.as_deref(), Some("Bearer x"));
        assert_eq!(r.body, "abcd");
    }

    #[test]
    fn parse_request_handles_get_oversize_and_garbage() {
        let r = req("GET / HTTP/1.1\r\n\r\n").unwrap();
        assert_eq!(r.method, "GET");
        assert_eq!(r.body, "");
        assert!(req(&format!("POST / HTTP/1.1\r\nContent-Length: {}\r\n\r\n", BODY_CAP + 1)).is_none());
        assert!(req("\r\n").is_none());
    }

    #[test]
    fn response_carries_status_and_content_length() {
        let r = response("200 OK", "{\"ok\":true}");
        assert!(r.starts_with("HTTP/1.1 200 OK\r\n"));
        assert!(r.contains("Content-Length: 11\r\n"));
        assert!(r.ends_with("{\"ok\":true}"));
    }

    #[test]
    fn inject_splices_before_the_closing_brace() {
        assert_eq!(inject("{\"a\":\"b\"}", "id", "\"x\""), "{\"a\":\"b\",\"id\":\"x\"}");
        assert_eq!(inject("{\"a\":\"b\"}\n", "expires", "7"), "{\"a\":\"b\",\"expires\":7}");
        // round-trips through the parser
        let out = inject("{\"message\":\"oi\"}", "expires", "7");
        assert!(assistant::json_fields(&out).unwrap().iter().any(|(k, v)| k == "expires" && v == "7"));
    }

    #[test]
    fn serve_forwards_say_and_rejects_garbage() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let (tx, rx) = channel();
        std::thread::spawn(move || serve(listener, tx));

        let mut s = TcpStream::connect(addr).unwrap();
        let body = "{\"from\":\"ci\",\"message\":\"build ok\"}";
        write!(s, "POST / HTTP/1.1\r\nContent-Length: {}\r\n\r\n{body}", body.len()).unwrap();
        let mut reply = String::new();
        s.read_to_string(&mut reply).unwrap();
        assert!(reply.starts_with("HTTP/1.1 200"), "{reply}");
        assert_eq!(rx.recv_timeout(Duration::from_secs(2)).unwrap(), body);

        let mut s = TcpStream::connect(addr).unwrap();
        write!(s, "POST / HTTP/1.1\r\nContent-Length: 4\r\n\r\nlixo").unwrap();
        let mut reply = String::new();
        s.read_to_string(&mut reply).unwrap();
        assert!(reply.starts_with("HTTP/1.1 400"), "{reply}");
    }
}
