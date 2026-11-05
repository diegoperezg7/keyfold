//! Registers Keyfold's MCP server with a coding agent (`keyfold connect claude|codex|cursor`,
//! or the menu bar). It writes the path to this binary and the `mcp` argument, nothing else:
//! never a key. Running it twice is harmless.

use keyfold_core::l10n::t;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Agent {
    Claude,
    Codex,
    Cursor,
}

impl Agent {
    pub const ALL: [Agent; 3] = [Agent::Claude, Agent::Codex, Agent::Cursor];

    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "claude" => Some(Agent::Claude),
            "codex" => Some(Agent::Codex),
            "cursor" => Some(Agent::Cursor),
            _ => None,
        }
    }

    pub fn id(self) -> &'static str {
        match self {
            Agent::Claude => "claude",
            Agent::Codex => "codex",
            Agent::Cursor => "cursor",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Agent::Claude => "Claude Code",
            Agent::Codex => "Codex",
            Agent::Cursor => "Cursor",
        }
    }
}

pub enum Outcome {
    Connected(String),
    AlreadyConnected(String),
}

impl Outcome {
    pub fn message(&self) -> &str {
        match self {
            Outcome::Connected(m) | Outcome::AlreadyConnected(m) => m,
        }
    }
}

/// The binary agents should start: this one.
pub fn keyfold_exe() -> Result<PathBuf, String> {
    std::env::current_exe().map_err(|e| e.to_string())
}

pub fn connect(agent: Agent) -> Result<Outcome, String> {
    let exe = keyfold_exe()?;
    let home = home_dir().ok_or_else(|| t("No se encuentra la carpeta de usuario.", "Home folder not found."))?;
    match agent {
        Agent::Claude => connect_claude(&exe, &home),
        Agent::Codex => write_config(&home.join(".codex/config.toml"), agent, |old| Ok(codex_config(old, &exe))),
        Agent::Cursor => write_config(&home.join(".cursor/mcp.json"), agent, |old| cursor_config(old, &exe)),
    }
}

/// Claude Code keeps its config in a file it also writes while running, so go through its CLI.
fn connect_claude(exe: &Path, home: &Path) -> Result<Outcome, String> {
    let manual = format!("claude mcp add --scope user keyfold -- {} mcp", exe.display());
    let Some(claude) = find_claude(home) else {
        return Err(t(
            &format!("No se encuentra el comando claude. Ejecuta: {manual}"),
            &format!("The claude command was not found. Run: {manual}"),
        ));
    };
    let run = |args: &[&str]| Command::new(&claude).args(args).output();
    if run(&["mcp", "get", "keyfold"]).map(|o| o.status.success()).unwrap_or(false) {
        return Ok(Outcome::AlreadyConnected(already(Agent::Claude)));
    }
    let exe = exe.display().to_string();
    let out = run(&["mcp", "add", "--scope", "user", "keyfold", "--", &exe, "mcp"]).map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(Outcome::Connected(connected(Agent::Claude)))
    } else {
        Err(t(&format!("claude mcp add falló. Ejecuta: {manual}"), &format!("claude mcp add failed. Run: {manual}")))
    }
}

/// Apps opened from Finder get a minimal PATH, so also look where the installer puts claude.
fn find_claude(home: &Path) -> Option<PathBuf> {
    let from_path = std::env::var_os("PATH").into_iter().flat_map(|p| std::env::split_paths(&p).collect::<Vec<_>>());
    let known = [home.join(".local/bin"), home.join(".claude/local"), "/opt/homebrew/bin".into(), "/usr/local/bin".into()];
    from_path.chain(known).map(|dir| dir.join(if cfg!(windows) { "claude.exe" } else { "claude" })).find(|p| p.is_file())
}

fn write_config(
    path: &Path,
    agent: Agent,
    update: impl Fn(Option<&str>) -> Result<Option<String>, String>,
) -> Result<Outcome, String> {
    let old = std::fs::read_to_string(path).ok();
    match update(old.as_deref())? {
        None => Ok(Outcome::AlreadyConnected(already(agent))),
        Some(new) => {
            if let Some(dir) = path.parent() {
                std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
            }
            std::fs::write(path, new).map_err(|e| e.to_string())?;
            Ok(Outcome::Connected(connected(agent)))
        }
    }
}

/// New contents for `~/.codex/config.toml`, or None when Keyfold is already there.
/// Appends a table instead of rewriting the file, so comments and order survive.
pub fn codex_config(existing: Option<&str>, exe: &Path) -> Option<String> {
    let existing = existing.unwrap_or("");
    if existing.lines().any(|l| l.trim() == "[mcp_servers.keyfold]") {
        return None;
    }
    let command = serde_json::to_string(&exe.display().to_string()).unwrap_or_default(); // TOML basic string
    let separator = if existing.is_empty() || existing.ends_with("\n\n") { "" } else if existing.ends_with('\n') { "\n" } else { "\n\n" };
    Some(format!("{existing}{separator}[mcp_servers.keyfold]\ncommand = {command}\nargs = [\"mcp\"]\n"))
}

/// New contents for `~/.cursor/mcp.json`, or None when it already points here.
/// Refuses to touch a file that is not valid JSON.
pub fn cursor_config(existing: Option<&str>, exe: &Path) -> Result<Option<String>, String> {
    let mut config: Value = match existing.map(str::trim).filter(|s| !s.is_empty()) {
        Some(text) => serde_json::from_str(text).map_err(|_| t("~/.cursor/mcp.json no es JSON válido; no lo toco.", "~/.cursor/mcp.json is not valid JSON; leaving it alone."))?,
        None => json!({}),
    };
    let entry = json!({ "command": exe.display().to_string(), "args": ["mcp"] });
    let Some(object) = config.as_object_mut() else {
        return Err(t("~/.cursor/mcp.json no es un objeto JSON; no lo toco.", "~/.cursor/mcp.json is not a JSON object; leaving it alone."));
    };
    let servers = object.entry("mcpServers").or_insert_with(|| json!({}));
    let Some(servers) = servers.as_object_mut() else {
        return Err(t("mcpServers no es un objeto; no lo toco.", "mcpServers is not an object; leaving it alone."));
    };
    if servers.get("keyfold") == Some(&entry) {
        return Ok(None);
    }
    servers.insert("keyfold".into(), entry);
    serde_json::to_string_pretty(&config).map(|s| Some(s + "\n")).map_err(|e| e.to_string())
}

fn connected(agent: Agent) -> String {
    t(
        &format!("{} conectado. Reinícialo para que cargue Keyfold.", agent.label()),
        &format!("{} connected. Restart it to load Keyfold.", agent.label()),
    )
}

fn already(agent: Agent) -> String {
    t(&format!("{} ya estaba conectado.", agent.label()), &format!("{} was already connected.", agent.label()))
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).map(PathBuf::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXE: &str = "/Applications/Keyfold.app/Contents/MacOS/keyfold";

    #[test]
    fn codex_appends_once_and_keeps_the_rest() {
        let old = "model = \"o4\"\n";
        let new = codex_config(Some(old), Path::new(EXE)).unwrap();
        assert!(new.starts_with(old));
        assert!(new.contains("[mcp_servers.keyfold]\ncommand = \"/Applications/Keyfold.app/Contents/MacOS/keyfold\"\nargs = [\"mcp\"]\n"));
        assert_eq!(codex_config(Some(&new), Path::new(EXE)), None);
        assert!(codex_config(None, Path::new(EXE)).unwrap().starts_with("[mcp_servers.keyfold]"));
    }

    #[test]
    fn cursor_merges_into_existing_servers() {
        let old = r#"{ "mcpServers": { "other": { "command": "x" } }, "keep": 1 }"#;
        let new = cursor_config(Some(old), Path::new(EXE)).unwrap().unwrap();
        let parsed: Value = serde_json::from_str(&new).unwrap();
        assert_eq!(parsed["keep"], 1);
        assert_eq!(parsed["mcpServers"]["other"]["command"], "x");
        assert_eq!(parsed["mcpServers"]["keyfold"]["command"], EXE);
        assert_eq!(cursor_config(Some(&new), Path::new(EXE)).unwrap(), None);
    }

    #[test]
    fn cursor_leaves_invalid_files_alone() {
        assert!(cursor_config(Some("{ not json"), Path::new(EXE)).is_err());
        assert!(cursor_config(Some("[1, 2]"), Path::new(EXE)).is_err());
        assert!(cursor_config(None, Path::new(EXE)).unwrap().is_some());
    }

    #[test]
    fn config_never_contains_more_than_the_binary_path() {
        let codex = codex_config(None, Path::new(EXE)).unwrap();
        let cursor = cursor_config(None, Path::new(EXE)).unwrap().unwrap();
        for text in [codex, cursor] {
            assert!(!text.to_lowercase().contains("key ="), "{text}");
            assert!(!text.contains("env"), "{text}");
        }
    }
}
