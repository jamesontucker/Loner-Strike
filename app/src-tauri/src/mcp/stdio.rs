//! `loner-strike --mcp-stdio`: a bridge for agents that only start local programs. It reads
//! one JSON-RPC message per line on stdin, sends each to the running app's MCP endpoint
//! (LONER_MCP_PORT, default 8029, with LONER_MCP_TOKEN) and writes the answer as one line
//! on stdout. Nothing else goes to stdout; problems go to stderr. Plain HTTP/1.1 over a
//! local socket, so it needs nothing beyond the standard library.

use serde_json::{json, Value};
use std::io::{BufRead, Read, Write};
use std::net::TcpStream;
use std::time::Duration;

pub fn wanted() -> bool {
    std::env::args().any(|a| a == "--mcp-stdio")
}

/// Runs until stdin closes. The port and token are looked up for every message (the app may
/// have restarted, or got a new token): LONER_MCP_TOKEN / LONER_MCP_PORT if set, else the file
/// the app writes while AI agents are on (conn.rs).
pub fn run() {
    let file = super::conn::connection_file();
    let mut bridge = Bridge::default();
    let stdin = std::io::stdin();
    let mut out = std::io::stdout();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let where_ = super::conn::resolve(std::env::var("LONER_MCP_TOKEN").ok(), std::env::var("LONER_MCP_PORT").ok(), &file);
        let answer = bridge.pass(line, where_, send);
        if let Some(a) = answer {
            if writeln!(out, "{a}").and_then(|_| out.flush()).is_err() {
                break;
            }
        }
    }
}

type Sender = fn(u16, &str, Option<&str>, &str) -> Result<HttpAnswer, String>;

/// What the bridge remembers between messages: the legacy session, and the client's
/// `initialize`, so a restarted app (whose sessions are gone) is greeted again without the
/// client noticing.
#[derive(Default)]
pub struct Bridge {
    session: Option<String>,
    init: Option<String>,
}

impl Bridge {
    /// One message from the agent: its answer (None for a notification).
    pub fn pass(&mut self, line: &str, where_: Result<(u16, String), String>, send: Sender) -> Option<Value> {
        let msg = serde_json::from_str::<Value>(line).ok();
        let id = msg.as_ref().and_then(|v| v.get("id").cloned()).filter(|i| !i.is_null());
        let fail = |id: Option<&Value>, why: &str| id.map(|id| json!({"jsonrpc": "2.0", "id": id, "error": {"code": -32603, "message": why}}));
        let (port, token) = match where_ {
            Ok(w) => w,
            Err(why) => return fail(id.as_ref(), &why),
        };
        if msg.as_ref().and_then(|m| m["method"].as_str()) == Some("initialize") {
            self.init = Some(line.to_string());
            self.session = None;
        }
        let mut r = send(port, &token, self.session.as_deref(), line);
        // the app restarted under us: start a new session the way the client did, then retry
        if matches!(&r, Ok((404, _, _))) && self.session.is_some() {
            if let Some(init) = self.init.clone() {
                if let Ok((200, h, _)) = send(port, &token, None, &init) {
                    self.session = header(&h, "mcp-session-id");
                    let _ = send(port, &token, self.session.as_deref(), r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#);
                    r = send(port, &token, self.session.as_deref(), line);
                }
            }
        }
        match r {
            Ok((status, headers, body)) => {
                if let Some(s) = header(&headers, "mcp-session-id") {
                    self.session = Some(s);
                }
                answer_for(status, &body, id.as_ref())
            }
            Err(e) => {
                eprintln!("loner-strike --mcp-stdio: {e}");
                fail(id.as_ref(), "Loner-Strike isn't running, or AI agents were turned off. Open Loner-Strike and turn on Settings > AI agents.")
            }
        }
    }
}

/// What goes back to the agent for one HTTP answer: the JSON-RPC body (with the request's
/// id put in when the endpoint couldn't know it), or nothing (202, a notification).
pub fn answer_for(status: u16, body: &[u8], id: Option<&Value>) -> Option<Value> {
    let mut v: Value = match serde_json::from_slice(body) {
        Ok(v) => v,
        Err(_) if body.is_empty() && (200..300).contains(&status) => return None,
        Err(_) => {
            let text = String::from_utf8_lossy(body).trim().to_string();
            json!({"jsonrpc": "2.0", "error": {"code": -32603, "message": format!("Loner-Strike answered {status}: {text}")}})
        }
    };
    if v.get("id").is_none() {
        let id = id?;
        v["id"] = id.clone();
    }
    Some(v)
}

/// The mirrored headers the modern revision wants, worked out from the body.
pub fn mirrored(body: &str) -> Vec<(String, String)> {
    let Ok(v) = serde_json::from_str::<Value>(body) else { return vec![] };
    let mut h = vec![];
    if let Some(ver) = v.pointer("/params/_meta/io.modelcontextprotocol~1protocolVersion").and_then(Value::as_str) {
        h.push(("MCP-Protocol-Version".into(), ver.to_string()));
        if let Some(m) = v["method"].as_str() {
            h.push(("Mcp-Method".into(), m.to_string()));
        }
        let name = v["params"]["name"].as_str().or(v["params"]["uri"].as_str());
        if let Some(n) = name {
            let plain = n.bytes().all(|c| (0x20..0x7f).contains(&c)) && n.trim() == n && !(n.starts_with("=?base64?") && n.ends_with("?="));
            let value = if plain {
                n.to_string()
            } else {
                use base64::Engine;
                format!("=?base64?{}?=", base64::engine::general_purpose::STANDARD.encode(n))
            };
            h.push(("Mcp-Name".into(), value));
        }
    }
    h
}

fn header(headers: &[(String, String)], name: &str) -> Option<String> {
    headers.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.clone())
}

type HttpAnswer = (u16, Vec<(String, String)>, Vec<u8>);

fn send(port: u16, token: &str, session: Option<&str>, body: &str) -> Result<HttpAnswer, String> {
    let mut s = TcpStream::connect(("127.0.0.1", port)).map_err(|e| format!("couldn't reach 127.0.0.1:{port}: {e}"))?;
    // (a tool can wait on the person answering a question in the app)
    s.set_read_timeout(Some(Duration::from_secs(600))).ok();
    let mut req = format!(
        "POST /mcp HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nContent-Type: application/json\r\nAccept: application/json, text/event-stream\r\nAuthorization: Bearer {token}\r\nContent-Length: {}\r\nConnection: close\r\n",
        body.len()
    );
    if let Some(sid) = session {
        req.push_str(&format!("Mcp-Session-Id: {sid}\r\n"));
    }
    for (k, v) in mirrored(body) {
        req.push_str(&format!("{k}: {v}\r\n"));
    }
    req.push_str("\r\n");
    req.push_str(body);
    s.write_all(req.as_bytes()).map_err(|e| e.to_string())?;
    let mut raw = vec![];
    s.read_to_end(&mut raw).map_err(|e| e.to_string())?;
    parse_response(&raw)
}

/// Status, headers and body of an HTTP/1.1 answer (Content-Length, chunked or until close).
pub fn parse_response(raw: &[u8]) -> Result<HttpAnswer, String> {
    let split = raw.windows(4).position(|w| w == b"\r\n\r\n").ok_or("the answer was cut short")?;
    let head = String::from_utf8_lossy(&raw[..split]).to_string();
    let mut lines = head.split("\r\n");
    let status: u16 = lines.next().and_then(|l| l.split_whitespace().nth(1)).and_then(|c| c.parse().ok()).ok_or("not an HTTP answer")?;
    let headers: Vec<(String, String)> = lines.filter_map(|l| l.split_once(':')).map(|(k, v)| (k.trim().to_string(), v.trim().to_string())).collect();
    let rest = &raw[split + 4..];
    let body = if header(&headers, "transfer-encoding").is_some_and(|t| t.eq_ignore_ascii_case("chunked")) {
        let mut out = vec![];
        let mut at = 0;
        loop {
            let end = rest[at..].windows(2).position(|w| w == b"\r\n").ok_or("bad chunk")? + at;
            let size = usize::from_str_radix(String::from_utf8_lossy(&rest[at..end]).split(';').next().unwrap_or("").trim(), 16).map_err(|_| "bad chunk size")?;
            at = end + 2;
            if size == 0 {
                break;
            }
            out.extend_from_slice(rest.get(at..at + size).ok_or("chunk cut short")?);
            at += size + 2;
        }
        out
    } else if let Some(n) = header(&headers, "content-length").and_then(|n| n.parse::<usize>().ok()) {
        rest.get(..n).ok_or("the answer was cut short")?.to_vec()
    } else {
        rest.to_vec()
    };
    Ok((status, headers, body))
}
