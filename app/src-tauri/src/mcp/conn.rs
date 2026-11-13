//! Where the connector can be reached right now, for the stdio bridge, so an agent's config
//! is just `<app> --mcp-stdio` and never needs a token or port that may change.
//!
//! While the connector listens, `<app dir>/mcp-connection.json` holds `{port, token}`: written
//! atomically and readable by this user only (`sys::write_private`: 0600 on the Mac and Linux;
//! the per-user app data folder on Windows), rewritten when the token changes, and deleted when
//! the connector is turned off (or found left over at start with it off).
//!
//! With "Keep the same token across restarts", the token itself is kept in `<app dir>/mcp-token`
//! the same way, so HTTP clients' configs keep working too.

use serde_json::{json, Value};
use std::path::{Path, PathBuf};

pub fn connection_file() -> PathBuf {
    crate::sys::app_dir().join("mcp-connection.json")
}

pub fn token_file() -> PathBuf {
    crate::sys::app_dir().join("mcp-token")
}

pub fn write(path: &Path, port: u16, token: &str) -> std::io::Result<()> {
    let v = json!({"port": port, "token": token, "pid": std::process::id()});
    crate::sys::write_private(path, v.to_string().as_bytes())
}

pub fn remove(path: &Path) {
    let _ = std::fs::remove_file(path);
}

/// (port, token) from the file, if it's there and whole.
pub fn read(path: &Path) -> Option<(u16, String)> {
    let v: Value = serde_json::from_slice(&std::fs::read(path).ok()?).ok()?;
    let port = u16::try_from(v["port"].as_u64()?).ok().filter(|p| *p > 0)?;
    let token = v["token"].as_str().filter(|t| t.len() >= 16)?.to_string();
    Some((port, token))
}

pub const OFF: &str = "AI agents are off in Loner-Strike, or Loner-Strike isn't running. Open Loner-Strike and turn on Settings > AI agents.";

/// The bridge's port and token: LONER_MCP_TOKEN / LONER_MCP_PORT win, else the file.
pub fn resolve(env_token: Option<String>, env_port: Option<String>, file: &Path) -> Result<(u16, String), String> {
    let env_port = env_port.and_then(|p| p.trim().parse::<u16>().ok()).filter(|p| *p > 0);
    let env_token = env_token.map(|t| t.trim().to_string()).filter(|t| !t.is_empty());
    let saved = read(file);
    match (env_token, env_port, saved) {
        (Some(t), Some(p), _) => Ok((p, t)),
        (Some(t), None, saved) => Ok((saved.map(|s| s.0).unwrap_or(super::PORT), t)),
        (None, p, Some((port, token))) => Ok((p.unwrap_or(port), token)),
        (None, _, None) => Err(OFF.into()),
    }
}

/// A kept token (at least 16 characters), if there is one.
pub fn kept_token(path: &Path) -> Option<String> {
    let t = String::from_utf8(std::fs::read(path).ok()?).ok()?;
    let t = t.trim();
    (t.len() >= 16 && t.chars().all(|c| c.is_ascii_alphanumeric())).then(|| t.to_string())
}

pub fn keep_token(path: &Path, token: &str) -> std::io::Result<()> {
    crate::sys::write_private(path, token.as_bytes())
}

// ---- copy-ready setup for each client ---------------------------------------------------

/// A string as a JSON (and TOML basic) string literal: quotes and backslashes escaped.
fn lit(s: &str) -> String {
    serde_json::to_string(s).unwrap_or_default()
}

/// Setups for the clients people use: the stdio bridge where the client can start programs
/// (valid forever), and the HTTP form with the address and token.
pub fn snippets(exe: &str, url: Option<&str>, token: &str) -> Vec<Value> {
    let e = lit(exe);
    let mut v = vec![
        json!({"client": "claude-code", "label": "Claude Code", "kind": "stdio", "where": "Run in a terminal",
               "text": format!("claude mcp add --scope user --transport stdio loner-strike -- {e} --mcp-stdio")}),
        json!({"client": "codex", "label": "Codex", "kind": "stdio", "where": "~/.codex/config.toml",
               "text": format!("[mcp_servers.loner-strike]\ncommand = {e}\nargs = [\"--mcp-stdio\"]\n")}),
        json!({"client": "opencode", "label": "OpenCode", "kind": "stdio", "where": "opencode.json (merge into \"mcp\")",
               "text": format!("{{\n  \"$schema\": \"https://opencode.ai/config.json\",\n  \"mcp\": {{\n    \"servers\": {{\n      \"loner-strike\": {{\n        \"type\": \"local\",\n        \"command\": [{e}, \"--mcp-stdio\"]\n      }}\n    }}\n  }}\n}}\n")}),
        json!({"client": "cursor", "label": "Cursor", "kind": "stdio", "where": "~/.cursor/mcp.json",
               "text": format!("{{\n  \"mcpServers\": {{\n    \"loner-strike\": {{\n      \"type\": \"stdio\",\n      \"command\": {e},\n      \"args\": [\"--mcp-stdio\"]\n    }}\n  }}\n}}\n")}),
        json!({"client": "claude-desktop", "label": "Claude Desktop", "kind": "stdio", "where": "claude_desktop_config.json (Settings > Developer > Edit Config)",
               "text": format!("{{\n  \"mcpServers\": {{\n    \"loner-strike\": {{\n      \"command\": {e},\n      \"args\": [\"--mcp-stdio\"]\n    }}\n  }}\n}}\n")}),
        json!({"client": "vscode", "label": "VS Code", "kind": "stdio", "where": ".vscode/mcp.json (or MCP: Open User Configuration)",
               "text": format!("{{\n  \"servers\": {{\n    \"loner-strike\": {{\n      \"type\": \"stdio\",\n      \"command\": {e},\n      \"args\": [\"--mcp-stdio\"]\n    }}\n  }}\n}}\n")}),
        json!({"client": "other", "label": "Other clients", "kind": "stdio", "where": "Most clients take this shape",
               "text": format!("{{\n  \"mcpServers\": {{\n    \"loner-strike\": {{\n      \"command\": {e},\n      \"args\": [\"--mcp-stdio\"]\n    }}\n  }}\n}}\n")}),
    ];
    if let Some(u) = url {
        let (u, bearer) = (lit(u), lit(&format!("Bearer {token}")));
        v.extend([
            json!({"client": "claude-code", "label": "Claude Code", "kind": "http", "where": "Run in a terminal",
                   "text": format!("claude mcp add --scope user --transport http loner-strike {} --header \"Authorization: Bearer {token}\"", u.trim_matches('"'))}),
            json!({"client": "codex", "label": "Codex", "kind": "http", "where": "~/.codex/config.toml",
                   "text": format!("[mcp_servers.loner-strike]\nurl = {u}\nhttp_headers = {{ \"Authorization\" = {bearer} }}\n")}),
            json!({"client": "opencode", "label": "OpenCode", "kind": "http", "where": "opencode.json (merge into \"mcp\")",
                   "text": format!("{{\n  \"$schema\": \"https://opencode.ai/config.json\",\n  \"mcp\": {{\n    \"servers\": {{\n      \"loner-strike\": {{\n        \"type\": \"remote\",\n        \"url\": {u},\n        \"oauth\": false,\n        \"headers\": {{ \"Authorization\": {bearer} }}\n      }}\n    }}\n  }}\n}}\n")}),
            json!({"client": "cursor", "label": "Cursor", "kind": "http", "where": "~/.cursor/mcp.json",
                   "text": format!("{{\n  \"mcpServers\": {{\n    \"loner-strike\": {{\n      \"url\": {u},\n      \"headers\": {{ \"Authorization\": {bearer} }}\n    }}\n  }}\n}}\n")}),
            json!({"client": "vscode", "label": "VS Code", "kind": "http", "where": ".vscode/mcp.json (or MCP: Open User Configuration)",
                   "text": format!("{{\n  \"servers\": {{\n    \"loner-strike\": {{\n      \"type\": \"http\",\n      \"url\": {u},\n      \"headers\": {{ \"Authorization\": {bearer} }}\n    }}\n  }}\n}}\n")}),
            json!({"client": "other", "label": "Other clients", "kind": "http", "where": "Streamable HTTP",
                   "text": format!("URL: {}\nHeader: Authorization: Bearer {token}", u.trim_matches('"'))}),
        ]);
    }
    v
}
