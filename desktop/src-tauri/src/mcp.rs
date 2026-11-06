//! Minimal MCP server over stdio (newline-delimited JSON-RPC 2.0).
//! Same tools and contract as the Swift version: no tool ever returns a value.

use crate::prompt;
use keyfold_core::{Config, Keyfold, KeyfoldError, SlotInfo, GENERATE_DEFAULT, GENERATE_MAX, GENERATE_MIN};
use serde_json::{json, Map, Value};
use std::io::{BufRead, Write};

const SUPPORTED_VERSIONS: [&str; 4] = ["2025-11-25", "2025-06-18", "2025-03-26", "2024-11-05"];

const USAGE_NOTE: &str = "Never ask the user to paste a key into the chat or into a file. \
To run the project with its keys, use the shell: `keyfold run --project <project> -- <command>`. \
Do not write values into .env files; the keys are injected at run time. \
To tie a folder to its project, write a `.keyfold` file at the repo root whose only line is the project name \
(shared with the team), or call link_folder (this machine only); \
then `keyfold run -- <command>` works there without `--project`.";

fn tools() -> Value {
    json!([
        {
            "name": "list_secrets",
            "description": "List stored API key names, whether each has a value, and the projects they are assigned to. Never returns values.",
            "inputSchema": { "type": "object", "properties": { "project": { "type": "string", "description": "Only keys assigned to this project." } } },
            "annotations": { "readOnlyHint": true }
        },
        {
            "name": "create_secret",
            "description": format!("Create a slot for an API key (e.g. EXA_API_KEY), optionally assign it to a project, and open a native window where the user pastes the value. Blocks until the user saves or cancels. Returns only the reference keyfold://NAME. {USAGE_NOTE}"),
            "inputSchema": {
                "type": "object",
                "properties": {
                    "name": { "type": "string", "description": "UPPER_SNAKE_CASE env var name, e.g. EXA_API_KEY." },
                    "project": { "type": "string", "description": "Project name (letters, digits, . _ -)." },
                    "prompt": { "type": "boolean", "description": "Open the paste window if the slot has no value. Default true." }
                },
                "required": ["name"]
            }
        },
        {
            "name": "generate_secret",
            "description": format!("Generate a random secret on this computer and store it, for secrets the user owns: JWT_SECRET, ENCRYPTION_KEY, database or admin passwords. Never for third-party API keys (use create_secret: the user pastes those). Letters and digits only. The value is never returned, only keyfold://NAME. Refuses to overwrite a key that already has a value. {USAGE_NOTE}"),
            "inputSchema": {
                "type": "object",
                "properties": {
                    "name": { "type": "string", "description": "UPPER_SNAKE_CASE env var name, e.g. JWT_SECRET." },
                    "project": { "type": "string", "description": "Project name (letters, digits, . _ -)." },
                    "length": { "type": "integer", "minimum": GENERATE_MIN, "maximum": GENERATE_MAX, "description": format!("Characters. Default {GENERATE_DEFAULT}.") }
                },
                "required": ["name"]
            }
        },
        {
            "name": "request_secret_value",
            "description": "Open the native window so the user pastes or replaces the value of an existing key. Blocks until saved or cancelled. Never returns the value.",
            "inputSchema": { "type": "object", "properties": { "name": { "type": "string" } }, "required": ["name"] }
        },
        {
            "name": "assign_secret",
            "description": format!("Assign an existing key to a project so `keyfold run --project <project>` injects it. {USAGE_NOTE}"),
            "inputSchema": {
                "type": "object",
                "properties": { "name": { "type": "string" }, "project": { "type": "string" } },
                "required": ["name", "project"]
            }
        },
        {
            "name": "link_folder",
            "description": "Link a local folder (absolute path, usually the repo root) to a project on this machine, so `keyfold run -- <command>` inside it needs no `--project`. Prefer a committed `.keyfold` file when the whole team should share it.",
            "inputSchema": {
                "type": "object",
                "properties": { "path": { "type": "string" }, "project": { "type": "string" } },
                "required": ["path", "project"]
            }
        },
        {
            "name": "unlink_folder",
            "description": "Remove the link between a local folder (absolute path) and its project.",
            "inputSchema": { "type": "object", "properties": { "path": { "type": "string" } }, "required": ["path"] }
        },
        {
            "name": "unassign_secret",
            "description": "Remove a key from a project, e.g. one assigned to a misspelled project. The key and its value stay; a project left with no keys disappears.",
            "inputSchema": {
                "type": "object",
                "properties": { "name": { "type": "string" }, "project": { "type": "string" } },
                "required": ["name", "project"]
            }
        }
    ])
}

enum ToolError {
    Message(String),
    Keyfold(KeyfoldError),
}

impl From<KeyfoldError> for ToolError {
    fn from(e: KeyfoldError) -> Self {
        Self::Keyfold(e)
    }
}

fn slot_json(info: &SlotInfo) -> Value {
    json!({ "name": info.name, "reference": info.reference, "hasValue": info.has_value, "projects": info.projects })
}

fn string_arg<'a>(args: &'a Map<String, Value>, key: &str) -> Result<&'a str, ToolError> {
    args.get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| ToolError::Message(format!("Missing argument: {key}")))
}

fn call_tool(config: &Config, l: &Keyfold, name: &str, args: &Map<String, Value>) -> Result<Value, ToolError> {
    match name {
        "list_secrets" => {
            let project = args.get("project").and_then(Value::as_str);
            Ok(json!({ "secrets": l.list(project)?.iter().map(slot_json).collect::<Vec<_>>() }))
        }
        "create_secret" => {
            let secret = string_arg(args, "name")?;
            let project = args.get("project").and_then(Value::as_str);
            let mut info = l.create_slot(secret, project)?;
            if !info.has_value && args.get("prompt").and_then(Value::as_bool).unwrap_or(true) {
                prompt::request(config, secret, project);
                info = l.slot(secret)?;
            }
            Ok(slot_json(&info))
        }
        "generate_secret" => {
            let secret = string_arg(args, "name")?;
            let project = args.get("project").and_then(Value::as_str);
            let length = match args.get("length") {
                None | Some(Value::Null) => GENERATE_DEFAULT,
                Some(v) => v.as_u64().map(|n| n as usize).ok_or(KeyfoldError::InvalidLength(0))?,
            };
            Ok(slot_json(&l.generate(secret, project, length)?))
        }
        "request_secret_value" => {
            let secret = string_arg(args, "name")?;
            l.slot(secret)?;
            let saved = prompt::request(config, secret, None);
            let mut result = slot_json(&l.slot(secret)?);
            result["saved"] = json!(saved);
            Ok(result)
        }
        "assign_secret" => {
            let secret = string_arg(args, "name")?;
            Ok(slot_json(&l.assign(secret, string_arg(args, "project")?)?))
        }
        "link_folder" => {
            let path = l.link_folder(std::path::Path::new(string_arg(args, "path")?), string_arg(args, "project")?)?;
            Ok(json!({ "path": path, "project": string_arg(args, "project")? }))
        }
        "unlink_folder" => {
            let removed = l.unlink_folder(std::path::Path::new(string_arg(args, "path")?))?;
            Ok(json!({ "removed": removed }))
        }
        "unassign_secret" => {
            let secret = string_arg(args, "name")?;
            Ok(slot_json(&l.unassign(secret, string_arg(args, "project")?)?))
        }
        other => Err(ToolError::Message(format!("Unknown tool: {other}"))),
    }
}

fn tool_result(structured: Value, is_error: bool) -> Value {
    json!({
        "content": [{ "type": "text", "text": structured.to_string() }],
        "structuredContent": structured,
        "isError": is_error
    })
}

fn handle(config: &Config, l: &Keyfold, method: &str, params: &Map<String, Value>) -> Result<Value, (i64, String)> {
    match method {
        "initialize" => {
            let requested = params.get("protocolVersion").and_then(Value::as_str).unwrap_or("");
            let version = if SUPPORTED_VERSIONS.contains(&requested) { requested } else { SUPPORTED_VERSIONS[1] };
            // The agent's shell often has no `keyfold` on PATH (an alias is not expanded in a
            // non-interactive shell), so tell it where this binary is.
            let path_note = match std::env::current_exe() {
                Ok(exe) => format!(" If `keyfold` is not on PATH, call it by its full path: {}", exe.display()),
                Err(_) => String::new(),
            };
            Ok(json!({
                "protocolVersion": version,
                "capabilities": { "tools": {} },
                "serverInfo": { "name": "keyfold", "version": env!("CARGO_PKG_VERSION") },
                "instructions": format!("Local API key vault. Keys are referenced by name; values stay in the OS credential store and are never returned. {USAGE_NOTE}{path_note}")
            }))
        }
        "ping" => Ok(json!({})),
        "tools/list" => Ok(json!({ "tools": tools() })),
        "tools/call" => {
            let name = params.get("name").and_then(Value::as_str).unwrap_or("");
            let empty = Map::new();
            let args = params.get("arguments").and_then(Value::as_object).unwrap_or(&empty);
            Ok(match call_tool(config, l, name, args) {
                Ok(v) => tool_result(v, false),
                Err(ToolError::Message(m)) => tool_result(json!({ "error": m }), true),
                Err(ToolError::Keyfold(e)) => tool_result(json!({ "error": e.to_string() }), true),
            })
        }
        other => Err((-32601, format!("Method not found: {other}"))),
    }
}

/// Reads requests from stdin until EOF.
pub fn serve(config: &Config) -> i32 {
    let l = Keyfold::open(config);
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        let response = match serde_json::from_str::<Value>(&line) {
            Ok(Value::Object(request)) if request.get("method").and_then(Value::as_str).is_some() => {
                // Notifications (no id) get no response.
                let Some(id) = request.get("id").cloned() else { continue };
                let method = request["method"].as_str().unwrap_or_default();
                let empty = Map::new();
                let params = request.get("params").and_then(Value::as_object).unwrap_or(&empty);
                match handle(config, &l, method, params) {
                    Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
                    Err((code, message)) => json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } }),
                }
            }
            _ => json!({ "jsonrpc": "2.0", "id": null, "error": { "code": -32700, "message": "Parse error" } }),
        };
        if writeln!(stdout, "{response}").and_then(|_| stdout.flush()).is_err() {
            break;
        }
    }
    0
}
