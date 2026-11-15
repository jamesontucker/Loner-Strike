//! The Model Context Protocol's JSON-RPC over Streamable HTTP, kept small and written here
//! (no SDK: see docs/MCP.md for why).
//!
//! Dual-era, as the 2026-07-28 revision allows:
//! - **Modern** (2026-07-28): every request carries its version in
//!   `params._meta["io.modelcontextprotocol/protocolVersion"]`, mirrored in the
//!   `MCP-Protocol-Version`, `Mcp-Method` and `Mcp-Name` headers; no sessions, no handshake.
//!   `server/discover` answers what we speak.
//! - **Legacy** (2025-03-26 to 2025-11-25): `initialize` first, which gets an `Mcp-Session-Id`
//!   the client sends back on every request after.
//!
//! A request with neither (no `_meta`, no session, not `initialize`) gets a plain 400 with an
//! ordinary JSON-RPC error, which is what tells a dual-era client to fall back to `initialize`.
//! Answers are always one JSON object (`application/json`); there is no SSE stream, and GET /
//! DELETE on the endpoint answer 405, as both eras allow.

use super::catalog;
use super::Ctx;
use axum::http::{HeaderMap, StatusCode};
use serde_json::{json, Map, Value};
use std::collections::VecDeque;
use std::sync::Mutex;

pub const MODERN: &str = "2026-07-28";
/// Legacy revisions answered through `initialize`, newest first.
pub const LEGACY: &[&str] = &["2025-11-25", "2025-06-18", "2025-03-26"];
const META_VERSION: &str = "io.modelcontextprotocol/protocolVersion";

// JSON-RPC and MCP error codes
pub const PARSE_ERROR: i64 = -32700;
pub const INVALID_REQUEST: i64 = -32600;
pub const METHOD_NOT_FOUND: i64 = -32601;
pub const INVALID_PARAMS: i64 = -32602;
pub const INTERNAL_ERROR: i64 = -32603;
pub const HEADER_MISMATCH: i64 = -32020;
pub const UNSUPPORTED_VERSION: i64 = -32022;
/// (2025-11-25 and earlier) a resource that isn't there
pub const RESOURCE_NOT_FOUND: i64 = -32002;

pub const INSTRUCTIONS: &str = "Loner-Strike's Tools for Counter-Strike 1.6 / Condition Zero maps (GoldSrc). \
Maps are named without .bsp (de_dust2). Coordinates are GoldSrc units, Z up. Read tools work any time; \
tools that change something need \"Allow changes\" in Loner-Strike's Settings, and some ask the person at \
the computer first. The map editor holds one open document, the same one the person sees in Tools > Map editor.";

/// What goes back over HTTP.
#[derive(Debug)]
pub struct Reply {
    pub status: StatusCode,
    /// None: no body (202 for a notification, 405)
    pub body: Option<Value>,
    /// A new legacy session's id, for the `Mcp-Session-Id` header.
    pub session: Option<String>,
}

impl Reply {
    fn ok(id: &Value, result: Value) -> Reply {
        Reply { status: StatusCode::OK, body: Some(json!({"jsonrpc": "2.0", "id": id, "result": result})), session: None }
    }
    pub fn error(status: StatusCode, id: Option<&Value>, code: i64, message: impl Into<String>, data: Option<Value>) -> Reply {
        let mut err = json!({"code": code, "message": message.into()});
        if let Some(d) = data {
            err["data"] = d;
        }
        let mut body = json!({"jsonrpc": "2.0", "error": err});
        // (an error about a request we couldn't read has no id at all)
        if let Some(id) = id {
            body["id"] = id.clone();
        }
        Reply { status, body: Some(body), session: None }
    }
    fn accepted() -> Reply {
        Reply { status: StatusCode::ACCEPTED, body: None, session: None }
    }
}

/// Legacy sessions handed out by `initialize` (the newest 64; an old one simply re-initializes).
#[derive(Default)]
pub struct Sessions(Mutex<VecDeque<String>>);

impl Sessions {
    fn add(&self) -> String {
        let id = super::random_token(24);
        let mut s = self.0.lock().unwrap_or_else(|e| e.into_inner());
        if s.len() >= 64 {
            s.pop_front();
        }
        s.push_back(id.clone());
        id
    }
    fn known(&self, id: &str) -> bool {
        self.0.lock().unwrap_or_else(|e| e.into_inner()).iter().any(|s| s == id)
    }
    /// Forgets every session (a new token: everyone connects again).
    pub fn clear(&self) {
        self.0.lock().unwrap_or_else(|e| e.into_inner()).clear();
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Era {
    Modern,
    Legacy,
}

fn header<'a>(h: &'a HeaderMap, name: &str) -> Option<&'a str> {
    h.get(name).and_then(|v| v.to_str().ok())
}

/// A mirrored header's value, `=?base64?...?=` decoded.
fn decoded(v: &str) -> Option<String> {
    use base64::Engine;
    match v.strip_prefix("=?base64?").and_then(|r| r.strip_suffix("?=")) {
        Some(b) => base64::engine::general_purpose::STANDARD.decode(b).ok().and_then(|b| String::from_utf8(b).ok()),
        None => Some(v.to_string()),
    }
}

/// Checks the modern revision's mirrored headers against the body.
fn check_headers(h: &HeaderMap, id: Option<&Value>, method: &str, params: &Value, version: &str) -> Result<(), Reply> {
    let bad = |msg: String| Err(Reply::error(StatusCode::BAD_REQUEST, id, HEADER_MISMATCH, format!("Header mismatch: {msg}"), None));
    match header(h, "mcp-protocol-version") {
        None => return bad("MCP-Protocol-Version header missing".into()),
        Some(v) if v != version => return bad(format!("MCP-Protocol-Version header value '{v}' does not match body value '{version}'")),
        _ => {}
    }
    match header(h, "mcp-method") {
        None => return bad("Mcp-Method header missing".into()),
        Some(v) if v != method => return bad(format!("Mcp-Method header value '{v}' does not match body value '{method}'")),
        _ => {}
    }
    let named = match method {
        "tools/call" | "prompts/get" => params.get("name").and_then(Value::as_str),
        "resources/read" => params.get("uri").and_then(Value::as_str),
        _ => None,
    };
    if let Some(name) = named {
        match header(h, "mcp-name").map(decoded) {
            None => return bad("Mcp-Name header missing".into()),
            Some(None) => return bad("Mcp-Name header is not valid base64".into()),
            Some(Some(v)) if v != name => return bad(format!("Mcp-Name header value '{v}' does not match body value '{name}'")),
            _ => {}
        }
    }
    Ok(())
}

/// One POST to the MCP endpoint (already past the token, host and origin checks).
pub async fn handle(ctx: &Ctx, sessions: &Sessions, h: &HeaderMap, body: &[u8]) -> Reply {
    let msg: Value = match serde_json::from_slice(body) {
        Ok(v) => v,
        Err(_) => return Reply::error(StatusCode::BAD_REQUEST, None, PARSE_ERROR, "Parse error: the body isn't JSON.", None),
    };
    if msg.is_array() {
        return Reply::error(StatusCode::BAD_REQUEST, None, INVALID_REQUEST, "Batches aren't supported: send one message per request.", None);
    }
    let Some(obj) = msg.as_object() else {
        return Reply::error(StatusCode::BAD_REQUEST, None, INVALID_REQUEST, "Invalid request: expected a JSON-RPC object.", None);
    };
    let id = obj.get("id").filter(|v| v.is_string() || v.is_number()).cloned();
    if obj.get("jsonrpc").and_then(Value::as_str) != Some("2.0") {
        return Reply::error(StatusCode::BAD_REQUEST, id.as_ref(), INVALID_REQUEST, "Invalid request: jsonrpc must be \"2.0\".", None);
    }
    // a response from the client: nothing of ours asks for one
    if obj.get("method").is_none() && (obj.contains_key("result") || obj.contains_key("error")) {
        return Reply::accepted();
    }
    let Some(method) = obj.get("method").and_then(Value::as_str) else {
        return Reply::error(StatusCode::BAD_REQUEST, id.as_ref(), INVALID_REQUEST, "Invalid request: no method.", None);
    };
    let params = obj.get("params").cloned().unwrap_or_else(|| json!({}));
    if !params.is_object() {
        return Reply::error(StatusCode::BAD_REQUEST, id.as_ref(), INVALID_PARAMS, "params must be an object.", None);
    }
    let meta_version = params.get("_meta").and_then(|m| m.get(META_VERSION)).and_then(Value::as_str).map(String::from);

    // a notification: nothing to answer (initialized, cancelled, ...)
    let Some(id) = id else {
        return if obj.contains_key("id") {
            Reply::error(StatusCode::BAD_REQUEST, None, INVALID_REQUEST, "Invalid request: id must be a string or a number.", None)
        } else {
            Reply::accepted()
        };
    };

    let era = if method == "initialize" {
        Era::Legacy
    } else if let Some(v) = &meta_version {
        if v != MODERN {
            let supported: Vec<&str> = std::iter::once(MODERN).chain(LEGACY.iter().copied()).collect();
            return Reply::error(StatusCode::BAD_REQUEST, Some(&id), UNSUPPORTED_VERSION, "Unsupported protocol version", Some(json!({"supported": supported, "requested": v})));
        }
        if let Err(r) = check_headers(h, Some(&id), method, &params, v) {
            return r;
        }
        Era::Modern
    } else {
        match header(h, "mcp-session-id") {
            Some(s) if sessions.known(s) => Era::Legacy,
            // the app restarted, or the session is too old: the client starts a new one
            Some(_) => return Reply::error(StatusCode::NOT_FOUND, Some(&id), INVALID_REQUEST, "Unknown session: initialize again.", None),
            None => {
                return Reply::error(
                    StatusCode::BAD_REQUEST,
                    Some(&id),
                    INVALID_REQUEST,
                    format!("Send initialize first, or protocol version {MODERN} in params._meta."),
                    None,
                )
            }
        }
    };

    let result = dispatch(ctx, era, method, &params).await;
    match result {
        Ok(mut v) => {
            if era == Era::Modern {
                v["resultType"] = json!("complete");
            }
            let mut r = Reply::ok(&id, v);
            if method == "initialize" {
                r.session = Some(sessions.add());
            }
            r
        }
        Err((code, msg)) => {
            // (modern: an unknown method is a 404 so it isn't mistaken for a legacy server)
            let status = if era == Era::Modern && code == METHOD_NOT_FOUND { StatusCode::NOT_FOUND } else { StatusCode::OK };
            Reply::error(status, Some(&id), code, msg, None)
        }
    }
}

fn capabilities() -> Value {
    json!({"tools": {"listChanged": false}, "resources": {"listChanged": false, "subscribe": false}, "prompts": {"listChanged": false}})
}

fn server_info() -> Value {
    json!({"name": "loner-strike", "title": "Loner-Strike Tools", "version": env!("CARGO_PKG_VERSION")})
}

type Answer = Result<Value, (i64, String)>;

async fn dispatch(ctx: &Ctx, era: Era, method: &str, params: &Value) -> Answer {
    match method {
        "initialize" => {
            let asked = params.get("protocolVersion").and_then(Value::as_str).unwrap_or("");
            let version = LEGACY.iter().find(|v| **v == asked).copied().unwrap_or(LEGACY[0]);
            Ok(json!({"protocolVersion": version, "capabilities": capabilities(), "serverInfo": server_info(), "instructions": INSTRUCTIONS}))
        }
        "server/discover" => Ok(json!({
            "supportedVersions": [MODERN],
            "capabilities": capabilities(),
            "_meta": {"io.modelcontextprotocol/serverInfo": server_info()},
            "instructions": INSTRUCTIONS,
        })),
        "ping" => Ok(json!({})),
        "tools/list" => Ok(json!({"tools": catalog::tool_list()})),
        "tools/call" => {
            let name = params.get("name").and_then(Value::as_str).ok_or((INVALID_PARAMS, "tools/call needs a tool name.".to_string()))?;
            let args = params.get("arguments").cloned().unwrap_or_else(|| json!({}));
            if !args.is_object() {
                return Err((INVALID_PARAMS, "arguments must be an object.".into()));
            }
            if !catalog::has_tool(name) {
                return Err((INVALID_PARAMS, format!("Unknown tool: {name}")));
            }
            Ok(catalog::call_tool(ctx, name, args).await)
        }
        "resources/list" => Ok(json!({"resources": catalog::resource_list()})),
        "resources/templates/list" => Ok(json!({"resourceTemplates": catalog::resource_templates()})),
        "resources/read" => {
            let uri = params.get("uri").and_then(Value::as_str).ok_or((INVALID_PARAMS, "resources/read needs a uri.".to_string()))?;
            match catalog::read_resource(ctx, uri).await {
                Ok(text) => Ok(json!({"contents": [{"uri": uri, "mimeType": "application/json", "text": text}]})),
                Err(catalog::ResourceError::Unknown) => Err((if era == Era::Modern { INVALID_PARAMS } else { RESOURCE_NOT_FOUND }, format!("Resource not found: {uri}"))),
                Err(catalog::ResourceError::Failed(e)) => Err((INTERNAL_ERROR, e)),
            }
        }
        "prompts/list" => Ok(json!({"prompts": catalog::prompt_list()})),
        "prompts/get" => {
            let name = params.get("name").and_then(Value::as_str).unwrap_or("");
            let args = params.get("arguments").and_then(Value::as_object).cloned().unwrap_or_else(Map::new);
            catalog::get_prompt(name, &args).ok_or((INVALID_PARAMS, format!("Unknown prompt: {name}")))
        }
        // (accepted and ignored: there are no log messages to send)
        "logging/setLevel" => Ok(json!({})),
        _ => Err((METHOD_NOT_FOUND, format!("Method not found: {method}"))),
    }
}
