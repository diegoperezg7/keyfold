//! Desktop UI. Two modes:
//! - Keys: the main window listing keys (`keyfold` / `keyfold app`).
//! - Prompt: only the paste window, run as a child by the CLI or the MCP server.
//! The webview gets names, status and projects. A value goes webview -> Rust (`save_secret`)
//! straight into the credential store, and is never logged. The one way back is the reveal
//! button of the keys window (`reveal_value`), on an explicit click.

use crate::prompt::PromptRequest;
use crate::tray;
use crate::unlock::{self, SessionUnlock};
use keyfold_core::{l10n, Config, Keyfold, SlotInfo};
use serde::Serialize;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager, RunEvent, State, WebviewUrl, WebviewWindow, WebviewWindowBuilder};
use tauri_plugin_updater::{Update, UpdaterExt};

pub enum Mode {
    Keys,
    Prompt(PromptRequest),
}

struct AppState {
    config: Config,
    /// Set only in prompt mode: where to report that the value was saved.
    prompt: Option<PromptRequest>,
    /// Touch ID / password before the first reveal while the keys window is open.
    unlock: SessionUnlock,
    /// The newer release found by `check_update`, ready for `install_update`.
    update: Mutex<Option<Update>>,
}

impl AppState {
    fn keyfold(&self) -> Keyfold {
        Keyfold::open(&self.config)
    }
}

/// Injected before the page loads: which view to show and in which language.
#[derive(Serialize)]
struct Boot<'a> {
    view: &'a str,
    name: Option<&'a str>,
    project: Option<&'a str>,
    lang: &'a str,
    /// "new": open the New key dialog (menu bar, when the window had to be created).
    action: Option<&'a str>,
    /// A message to show once (the result of "Connect agent").
    notice: Option<&'a Notice>,
}

#[derive(Serialize, Clone)]
pub struct Notice {
    pub ok: bool,
    pub text: String,
}

/// Shows the keys window, creating it if it was closed. With `action` "new" it opens the
/// New key dialog; `notice` shows a one-line message.
pub fn show_main(app: &AppHandle, action: Option<&str>, notice: Option<&Notice>) -> tauri::Result<()> {
    if let Some(window) = app.get_webview_window("main") {
        window.show()?;
        window.unminimize()?;
        window.set_focus()?;
        if action == Some("new") {
            app.emit_to("main", "open-new-key", ())?;
        }
        if let Some(notice) = notice {
            app.emit_to("main", "notice", notice)?;
        }
        return Ok(());
    }
    let boot = Boot { view: "keys", name: None, project: None, lang: l10n::lang(), action, notice };
    let builder = WebviewWindowBuilder::new(app, "main", WebviewUrl::App("index.html".into()))
        .initialization_script(boot_script(&boot))
        .title("Keyfold")
        .inner_size(980.0, 640.0)
        .min_inner_size(600.0, 420.0)
        .center()
        .focused(true);
    // Center the window controls in the 72 px top row (.topbar). macOS measures this y a little
    // above the buttons, so 38 lands their center at 36.
    #[cfg(target_os = "macos")]
    let builder = builder.traffic_light_position(tauri::LogicalPosition::new(20.0, 38.0));
    let window = unified_title_bar(builder).build()?;
    // Closing the window ends the session: the next reveal asks for Touch ID again.
    let handle = app.clone();
    window.on_window_event(move |event| {
        if matches!(event, tauri::WindowEvent::Destroyed) {
            handle.state::<AppState>().unlock.lock();
        }
    });
    window.set_focus()
}

/// macOS: content runs under a transparent title bar, so the header and the traffic lights
/// share one surface (the system draws the window controls, with Liquid Glass on macOS 26+).
fn unified_title_bar<'a>(builder: WebviewWindowBuilder<'a, tauri::Wry, AppHandle>) -> WebviewWindowBuilder<'a, tauri::Wry, AppHandle> {
    #[cfg(target_os = "macos")]
    return builder.title_bar_style(tauri::TitleBarStyle::Overlay).hidden_title(true);
    #[cfg(not(target_os = "macos"))]
    builder
}

/// Opens the paste window for one key (keys window buttons and the menu bar).
pub fn open_paste_window(app: &AppHandle, name: &str) -> Result<(), String> {
    open_prompt(app.clone(), name.to_string())
}

pub fn list_for_tray(app: &AppHandle) -> Vec<SlotInfo> {
    app.state::<AppState>().keyfold().list(None).unwrap_or_default()
}

fn boot_script(boot: &Boot) -> String {
    format!("window.__KEYFOLD__ = {};", serde_json::to_string(boot).unwrap_or_else(|_| "{}".into()))
}

#[tauri::command]
fn list_keys(state: State<AppState>) -> Result<Vec<SlotInfo>, String> {
    state.keyfold().list(None).map_err(|e| e.to_string())
}

/// "New key" in the keys window: an empty slot. The value goes through the paste window.
#[tauri::command]
fn create_key(state: State<AppState>, name: String, project: Option<String>) -> Result<SlotInfo, String> {
    let project = project.as_deref().map(str::trim).filter(|p| !p.is_empty());
    state.keyfold().create_slot(name.trim(), project).map_err(|e| e.to_string())
}

#[tauri::command]
fn assign_key(state: State<AppState>, name: String, project: String) -> Result<SlotInfo, String> {
    state.keyfold().assign(&name, project.trim()).map_err(|e| e.to_string())
}

#[tauri::command]
fn unassign_key(state: State<AppState>, name: String, project: String) -> Result<SlotInfo, String> {
    state.keyfold().unassign(&name, &project).map_err(|e| e.to_string())
}

/// Deletes the key and its value. Keys window only: the paste window never deletes.
#[tauri::command]
fn delete_key(window: WebviewWindow, state: State<AppState>, name: String) -> Result<(), String> {
    if state.prompt.is_some() || window.label() != "main" {
        return Err("not available in this window".into());
    }
    state.keyfold().delete(&name).map_err(|e| e.to_string())
}

/// Folder path -> project, for the keys window.
#[tauri::command]
fn list_folders(state: State<AppState>) -> Result<std::collections::BTreeMap<String, String>, String> {
    state.keyfold().folders().map_err(|e| e.to_string())
}

#[tauri::command]
fn unlink_folder(state: State<AppState>, path: String) -> Result<bool, String> {
    state.keyfold().unlink_folder(std::path::Path::new(&path)).map_err(|e| e.to_string())
}

/// Stores the value. The value is not logged and the reply carries no value.
#[tauri::command]
fn save_secret(app: AppHandle, state: State<AppState>, name: String, value: String) -> Result<(), String> {
    let value = value.trim();
    state.keyfold().set_value(&name, value).map_err(|e| e.to_string())?;
    clear_clipboard_if_holds(value);
    if let Some(request) = &state.prompt {
        std::fs::write(&request.result_file, "saved").map_err(|e| e.to_string())?;
    } else {
        // Paste window opened from the keys window: tell it which key was saved (name only).
        let _ = app.emit("key-saved", &name);
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

/// Reveal button of the keys window. The only path that sends a value to the webview, and
/// only on a click there: refused in prompt mode, and the CLI and MCP have no equivalent.
/// While a value is shown the window is left out of screen capture, so an agent running
/// `screencapture` cannot read it. `hide_value` lifts that.
/// The first reveal while the window is open asks for Touch ID or the password; "locked"
/// means the user cancelled. Async so the wait does not block the UI thread.
#[tauri::command]
async fn reveal_value(window: WebviewWindow, state: State<'_, AppState>, name: String) -> Result<Option<String>, String> {
    if state.prompt.is_some() || window.label() != "main" {
        return Err("not available in this window".into());
    }
    let reason = l10n::t(&format!("mostrar el valor de {name}"), &format!("show the value of {name}"));
    state.unlock.ensure(unlock::is_today, || unlock::authenticate(&reason)).map_err(|_| "locked".to_string())?;
    window.set_content_protected(true).map_err(|e| e.to_string())?;
    state.keyfold().reveal(&name).map_err(|e| e.to_string())
}

/// This computer's public key (`age1…`), to give to whoever shares keys with you.
#[tauri::command]
fn public_key(state: State<AppState>) -> Result<String, String> {
    state.keyfold().public_key().map_err(|e| e.to_string())
}

/// Encrypts `names` (all keys with a value when empty) to `recipients`. Reads values, so it
/// asks for Touch ID every time, whatever the reveal session says; "locked" when cancelled.
/// Only the encrypted package reaches the webview.
#[tauri::command]
async fn share_keys(window: WebviewWindow, state: State<'_, AppState>, names: Vec<String>, recipients: Vec<String>) -> Result<String, String> {
    if state.prompt.is_some() || window.label() != "main" {
        return Err("not available in this window".into());
    }
    let what = if names.is_empty() { l10n::t("todas tus claves", "all your keys") } else { names.join(", ") };
    let reason = l10n::t(&format!("compartir {what} cifradas"), &format!("share {what}, encrypted"));
    if !unlock::authenticate(&reason) {
        return Err("locked".into());
    }
    let keyfold = state.keyfold();
    let package = if names.is_empty() { keyfold.share_all(&recipients) } else { keyfold.share(&names, &recipients) };
    package.map_err(|e| e.to_string())
}

/// Opens a package made for this computer and stores its keys. Returns names and projects only.
#[tauri::command]
fn import_package(app: AppHandle, state: State<AppState>, package: String) -> Result<Vec<SlotInfo>, String> {
    let imported = state.keyfold().import(&package).map_err(|e| e.to_string())?;
    let _ = app.emit("keys-changed", ());
    Ok(imported)
}

#[tauri::command]
fn hide_value(window: WebviewWindow) {
    let _ = window.set_content_protected(false);
}

#[derive(Serialize, serde::Deserialize)]
struct UpdateInfo {
    version: String,
    /// Release notes ("notes" in latest.json). Empty when the release has none.
    notes: String,
}

/// Left by `install_update` just before the restart, so the new version can say it was
/// installed and show what changed. Read once by `take_installed_update`.
fn installed_marker(app: &AppHandle) -> Option<std::path::PathBuf> {
    app.path().app_config_dir().ok().map(|dir| dir.join("installed-update.json"))
}

#[derive(Serialize, Clone)]
struct UpdateProgress {
    downloaded: u64,
    total: Option<u64>,
}

/// Asks the release endpoint (tauri.conf.json > plugins.updater) for a newer version.
/// None when this is the latest one.
#[tauri::command]
async fn check_update(app: AppHandle, state: State<'_, AppState>) -> Result<Option<UpdateInfo>, String> {
    let update = app.updater().map_err(|e| e.to_string())?.check().await.map_err(|e| e.to_string())?;
    let info = update.as_ref().map(|u| UpdateInfo { version: u.version.clone(), notes: u.body.clone().unwrap_or_default() });
    *state.update.lock().unwrap() = update;
    Ok(info)
}

/// Downloads the update (reporting `update-progress` to the keys window), checks its
/// signature against the public key in tauri.conf.json, replaces the app and restarts it.
#[tauri::command]
async fn install_update(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    let update = state.update.lock().unwrap().clone().ok_or("no update")?;
    let mut downloaded = 0u64;
    update
        .download_and_install(
            |chunk, total| {
                downloaded += chunk as u64;
                let _ = app.emit_to("main", "update-progress", UpdateProgress { downloaded, total });
            },
            || {},
        )
        .await
        .map_err(|e| e.to_string())?;
    if let Some(path) = installed_marker(&app) {
        let info = UpdateInfo { version: update.version.clone(), notes: update.body.clone().unwrap_or_default() };
        let _ = std::fs::create_dir_all(path.parent().unwrap_or(&path));
        let _ = std::fs::write(&path, serde_json::to_string(&info).unwrap_or_default());
    }
    app.restart()
}

/// After the restart: the update that was just installed, once. None on a normal launch, or
/// when the marker is from another version (the install did not take).
#[tauri::command]
fn take_installed_update(app: AppHandle) -> Option<UpdateInfo> {
    let path = installed_marker(&app)?;
    let text = std::fs::read_to_string(&path).ok()?;
    let _ = std::fs::remove_file(&path);
    let info: UpdateInfo = serde_json::from_str(&text).ok()?;
    (info.version == app.package_info().version.to_string()).then_some(info)
}

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
    let boot = Boot { view: "prompt", name: Some(&name), project: None, lang: l10n::lang(), action: None, notice: None };
    prompt_window(&app, &label, &boot).map(|_| ()).map_err(|e| e.to_string())
}

fn prompt_window(app: &AppHandle, label: &str, boot: &Boot) -> tauri::Result<WebviewWindow> {
    let builder = WebviewWindowBuilder::new(app, label, WebviewUrl::App("index.html".into()))
        .initialization_script(boot_script(boot))
        .title("Keyfold")
        .inner_size(460.0, 360.0)
        .resizable(false)
        .minimizable(false)
        .maximizable(false)
        .always_on_top(true)
        .center()
        .focused(true);
    let window = unified_title_bar(builder).build()?;
    window.set_focus()?;
    Ok(window)
}

pub fn run(config: Config, mode: Mode) -> i32 {
    let prompt = match &mode {
        Mode::Prompt(request) => Some(request.clone()),
        Mode::Keys => None,
    };
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(AppState { config, prompt: prompt.clone(), unlock: SessionUnlock::default(), update: Mutex::new(None) })
        .invoke_handler(tauri::generate_handler![
            list_keys,
            create_key,
            assign_key,
            unassign_key,
            delete_key,
            list_folders,
            unlink_folder,
            save_secret,
            reveal_value,
            hide_value,
            public_key,
            share_keys,
            import_package,
            finish_prompt,
            open_prompt,
            check_update,
            install_update,
            take_installed_update
        ])
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
                        action: None,
                        notice: None,
                    };
                    prompt_window(handle, "prompt", &boot)?;
                }
                None => {
                    show_main(handle, None, None)?;
                    tray::install(handle)?;
                }
            }
            Ok(())
        })
        .build(tauri::generate_context!());

    let keys_mode = matches!(mode, Mode::Keys);
    match app {
        Ok(app) => {
            app.run(move |app, event| match event {
                // The keys app lives in the menu bar: closing its window does not quit it.
                // "Quit Keyfold" in the menu exits with a code, so it is not stopped here.
                RunEvent::ExitRequested { api, code: None, .. } if keys_mode => api.prevent_exit(),
                #[cfg(target_os = "macos")]
                RunEvent::Reopen { .. } if keys_mode => {
                    let _ = show_main(app, None, None);
                }
                _ => {}
            });
            0
        }
        Err(e) => {
            eprintln!("Keyfold: {e}");
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
