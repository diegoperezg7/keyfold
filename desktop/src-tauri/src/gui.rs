//! Desktop UI. Two modes:
//! - Keys: the main window listing keys (`llavero` / `llavero app`).
//! - Prompt: only the paste window, run as a child by the CLI or the MCP server.
//! The webview gets names, status and projects. A value only goes webview -> Rust
//! (`save_secret`), straight into the credential store, and is never logged or returned.

use crate::prompt::PromptRequest;
use llavero_core::{l10n, Config, Llavero, SlotInfo};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, RunEvent, State, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

pub enum Mode {
    Keys,
    Prompt(PromptRequest),
}

struct AppState {
    config: Config,
    /// Set only in prompt mode: where to report that the value was saved.
    prompt: Option<PromptRequest>,
}

impl AppState {
    fn llavero(&self) -> Llavero {
        Llavero::open(&self.config)
    }
}

/// Injected before the page loads: which view to show and in which language.
#[derive(Serialize)]
struct Boot<'a> {
    view: &'a str,
    name: Option<&'a str>,
    project: Option<&'a str>,
    lang: &'a str,
}

fn boot_script(boot: &Boot) -> String {
    format!("window.__LLAVERO__ = {};", serde_json::to_string(boot).unwrap_or_else(|_| "{}".into()))
}

#[tauri::command]
fn list_keys(state: State<AppState>) -> Result<Vec<SlotInfo>, String> {
    state.llavero().list(None).map_err(|e| e.to_string())
}

#[tauri::command]
fn assign_key(state: State<AppState>, name: String, project: String) -> Result<SlotInfo, String> {
    state.llavero().assign(&name, project.trim()).map_err(|e| e.to_string())
}

/// Stores the value. The value is not logged and the reply carries no value.
#[tauri::command]
fn save_secret(state: State<AppState>, name: String, value: String) -> Result<(), String> {
    let value = value.trim();
    state.llavero().set_value(&name, value).map_err(|e| e.to_string())?;
    clear_clipboard_if_holds(value);
    if let Some(request) = &state.prompt {
        std::fs::write(&request.result_file, "saved").map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// The user copied the key to paste it. Once stored, don't leave it in the clipboard,
/// where any process of the same user (an agent running `pbpaste`) could read it.
/// Only clears when the clipboard still holds exactly this value.
#[cfg(target_os = "macos")]
fn clear_clipboard_if_holds(value: &str) {
    use std::process::{Command, Stdio};
    let Ok(out) = Command::new("/usr/bin/pbpaste").stderr(Stdio::null()).output() else { return };
    if String::from_utf8_lossy(&out.stdout).trim() != value {
        return;
    }
    if let Ok(mut child) = Command::new("/usr/bin/pbcopy").stdin(Stdio::piped()).spawn() {
        drop(child.stdin.take()); // empty input -> empty clipboard
        let _ = child.wait();
    }
}

#[cfg(not(target_os = "macos"))]
fn clear_clipboard_if_holds(_value: &str) {}

/// Closes the paste window. In prompt mode that ends the process.
#[tauri::command]
fn finish_prompt(app: AppHandle, window: WebviewWindow, state: State<AppState>) {
    if state.prompt.is_some() {
        app.exit(0);
    } else {
        let _ = window.close();
        let _ = app.emit("keys-changed", ());
    }
}

/// From the keys window: open the paste window for one key.
#[tauri::command]
fn open_prompt(app: AppHandle, name: String) -> Result<(), String> {
    let label = format!("prompt-{}", name.to_lowercase().replace('_', "-"));
    if let Some(existing) = app.get_webview_window(&label) {
        return existing.set_focus().map_err(|e| e.to_string());
    }
    let boot = Boot { view: "prompt", name: Some(&name), project: None, lang: l10n::lang() };
    prompt_window(&app, &label, &boot).map(|_| ()).map_err(|e| e.to_string())
}

fn prompt_window(app: &AppHandle, label: &str, boot: &Boot) -> tauri::Result<WebviewWindow> {
    let window = WebviewWindowBuilder::new(app, label, WebviewUrl::App("index.html".into()))
        .initialization_script(boot_script(boot))
        .title("Llavero")
        .inner_size(460.0, 330.0)
        .resizable(false)
        .minimizable(false)
        .maximizable(false)
        .always_on_top(true)
        .center()
        .focused(true)
        .build()?;
    window.set_focus()?;
    Ok(window)
}

pub fn run(config: Config, mode: Mode) -> i32 {
    let prompt = match &mode {
        Mode::Prompt(request) => Some(request.clone()),
        Mode::Keys => None,
    };
    let app = tauri::Builder::default()
        .manage(AppState { config, prompt: prompt.clone() })
        .invoke_handler(tauri::generate_handler![list_keys, assign_key, save_secret, finish_prompt, open_prompt])
        .setup(move |app| {
            let handle = &app.handle().clone();
            match &prompt {
                Some(request) => {
                    // Regular (not Accessory): macOS only activates regular apps launched
                    // through LaunchServices, and the paste window needs keyboard focus.
                    #[cfg(target_os = "macos")]
                    app.set_activation_policy(tauri::ActivationPolicy::Regular);
                    let boot = Boot {
                        view: "prompt",
                        name: Some(&request.name),
                        project: request.project.as_deref(),
                        lang: l10n::lang(),
                    };
                    prompt_window(handle, "prompt", &boot)?;
                }
                None => {
                    let boot = Boot { view: "keys", name: None, project: None, lang: l10n::lang() };
                    let window = WebviewWindowBuilder::new(handle, "main", WebviewUrl::App("index.html".into()))
                        .initialization_script(boot_script(&boot))
                        .title("Llavero")
                        .inner_size(980.0, 620.0)
                        .min_inner_size(760.0, 420.0)
                        .center()
                        .focused(true)
                        .build()?;
                    window.set_focus()?;
                }
            }
            Ok(())
        })
        .build(tauri::generate_context!());

    match app {
        Ok(app) => {
            app.run(|_, _: RunEvent| {});
            0
        }
        Err(e) => {
            eprintln!("Llavero: {e}");
            1
        }
    }
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::clear_clipboard_if_holds;
    use std::io::Write;
    use std::process::{Command, Stdio};

    fn copy(text: &str) {
        let mut child = Command::new("/usr/bin/pbcopy").stdin(Stdio::piped()).spawn().unwrap();
        child.stdin.take().unwrap().write_all(text.as_bytes()).unwrap();
        child.wait().unwrap();
    }

    fn paste() -> String {
        String::from_utf8(Command::new("/usr/bin/pbpaste").output().unwrap().stdout).unwrap()
    }

    /// Touches the real clipboard, so it only runs on demand: `cargo test -- --ignored`.
    #[test]
    #[ignore]
    fn clears_clipboard_only_when_it_holds_the_saved_value() {
        copy("something-else");
        clear_clipboard_if_holds("the-key");
        assert_eq!(paste(), "something-else", "must not touch unrelated clipboard content");

        copy("the-key\n");
        clear_clipboard_if_holds("the-key");
        assert_eq!(paste(), "", "the pasted key must not stay in the clipboard");
    }
}
