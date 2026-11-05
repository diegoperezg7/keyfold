//! The paste window always runs in a child process (`llavero __prompt NAME ...`), so the
//! CLI and the MCP server stay headless and their stdout is never touched by the UI.
//! The child reports "saved" through a result file; the value never travels back.

use llavero_core::Config;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

pub const CHILD_COMMAND: &str = "__prompt";

#[derive(Debug, Clone)]
pub struct PromptRequest {
    pub name: String,
    pub project: Option<String>,
    pub result_file: PathBuf,
}

pub struct ChildArgs {
    pub config: Config,
    pub request: PromptRequest,
}

impl ChildArgs {
    /// `NAME --result FILE --home DIR --service SVC [--project P]`
    pub fn parse(args: &[String]) -> Option<Self> {
        let name = args.first()?.clone();
        let mut config = Config::from_env();
        let (mut result, mut project) = (None, None);
        let mut it = args[1..].iter();
        while let Some(flag) = it.next() {
            let value = it.next()?.clone();
            match flag.as_str() {
                "--result" => result = Some(PathBuf::from(value)),
                "--home" => config.home = PathBuf::from(value),
                "--service" => config.service = value,
                "--project" => project = Some(value),
                _ => return None,
            }
        }
        Some(Self { config, request: PromptRequest { name, project, result_file: result? } })
    }
}

/// Opens the paste window and blocks until it closes. True if a value was stored.
pub fn request(config: &Config, name: &str, project: Option<&str>) -> bool {
    let Ok(exe) = std::env::current_exe() else { return false };
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let result_file = std::env::temp_dir().join(format!("llavero-prompt-{}-{nanos}.result", std::process::id()));

    let mut child_args: Vec<String> = vec![
        CHILD_COMMAND.into(),
        name.into(),
        "--result".into(),
        result_file.display().to_string(),
        "--home".into(),
        config.home.display().to_string(),
        "--service".into(),
        config.service.clone(),
    ];
    if let Some(p) = project {
        child_args.extend(["--project".into(), p.into()]);
    }

    let mut command = launcher(&exe, &child_args);
    let _ = command.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).status();

    let saved = std::fs::read_to_string(&result_file).map(|s| s.trim() == "saved").unwrap_or(false);
    let _ = std::fs::remove_file(&result_file);
    saved
}

/// Inside a macOS .app, launch through LaunchServices (`open -W`) so the window gets
/// focus even when the caller is a background agent. Otherwise run the binary directly.
fn launcher(exe: &Path, args: &[String]) -> Command {
    #[cfg(target_os = "macos")]
    if let Some(bundle) = app_bundle(exe) {
        let mut command = Command::new("/usr/bin/open");
        command.args(["-n", "-W", "-a"]).arg(bundle).arg("--args").args(args);
        return command;
    }
    let mut command = Command::new(exe);
    command.args(args);
    command
}

#[cfg(target_os = "macos")]
fn app_bundle(exe: &Path) -> Option<PathBuf> {
    let bundle = exe.parent()?.parent()?.parent()?;
    (bundle.extension()? == "app").then(|| bundle.to_path_buf())
}
