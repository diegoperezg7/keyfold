//! Menu bar icon of the keys app. Quick access, plus what helps agents most:
//! keys an agent created that still wait for a value, and "Connect agent" to register
//! Keyfold's MCP server with Claude Code, Codex or Cursor. Shows names only, never values.

use crate::connect::{self, Agent};
use crate::gui::{list_for_tray, open_paste_window, show_main, Notice};
use keyfold_core::l10n::t;
use std::time::Duration;
use tauri::image::Image;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Wry};

const TRAY_ID: &str = "keyfold";

pub fn install(app: &AppHandle) -> tauri::Result<()> {
    let (initial, mut shown) = build_menu(app)?;
    TrayIconBuilder::with_id(TRAY_ID)
        .icon(Image::from_bytes(include_bytes!("../icons/tray.png"))?)
        .icon_as_template(true)
        .tooltip("Keyfold")
        .menu(&initial)
        .on_menu_event(|app, event| on_menu(app, event.id().as_ref()))
        .build(app)?;

    // Agents create keys over MCP while the app is open: keep "Waiting for a value" current.
    // The menu is only replaced when that list changes, so an open menu is not disturbed.
    let app = app.clone();
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_secs(3));
        if pending(&app) == shown {
            continue;
        }
        if let (Some(tray), Ok((menu, now))) = (app.tray_by_id(TRAY_ID), build_menu(&app)) {
            let _ = tray.set_menu(Some(menu));
            shown = now;
        }
    });
    Ok(())
}

fn pending(app: &AppHandle) -> Vec<String> {
    list_for_tray(app).into_iter().filter(|k| !k.has_value).map(|k| k.name).collect()
}

fn build_menu(app: &AppHandle) -> tauri::Result<(Menu<Wry>, Vec<String>)> {
    let names = pending(app);
    let waiting = Submenu::with_id(
        app,
        "pending",
        t(&format!("Esperando valor ({})", names.len()), &format!("Waiting for a value ({})", names.len())),
        !names.is_empty(),
    )?;
    for name in &names {
        waiting.append(&MenuItem::with_id(app, format!("pending:{name}"), name, true, None::<&str>)?)?;
    }
    let agents = Submenu::with_id(app, "connect", t("Conectar agente", "Connect agent"), true)?;
    for agent in Agent::ALL {
        agents.append(&MenuItem::with_id(app, format!("connect:{}", agent.id()), agent.label(), true, None::<&str>)?)?;
    }
    let menu = Menu::with_items(
        app,
        &[
            &MenuItem::with_id(app, "open", t("Abrir Keyfold", "Open Keyfold"), true, None::<&str>)?,
            &MenuItem::with_id(app, "new", t("Nueva clave…", "New key…"), true, None::<&str>)?,
            &PredefinedMenuItem::separator(app)?,
            &waiting,
            &agents,
            &PredefinedMenuItem::separator(app)?,
            &MenuItem::with_id(app, "quit", t("Salir de Keyfold", "Quit Keyfold"), true, None::<&str>)?,
        ],
    )?;
    Ok((menu, names))
}

fn on_menu(app: &AppHandle, id: &str) {
    match id {
        "open" => drop(show_main(app, None, None)),
        "new" => drop(show_main(app, Some("new"), None)),
        "quit" => app.exit(0),
        _ => {
            if let Some(name) = id.strip_prefix("pending:") {
                let _ = open_paste_window(app, name);
            } else if let Some(agent) = id.strip_prefix("connect:").and_then(Agent::parse) {
                // `claude mcp add` can take a moment: off the UI thread.
                let app = app.clone();
                std::thread::spawn(move || {
                    let notice = match connect::connect(agent) {
                        Ok(outcome) => Notice { ok: true, text: outcome.message().to_string() },
                        Err(text) => Notice { ok: false, text },
                    };
                    let _ = show_main(&app, None, Some(&notice));
                });
            }
        }
    }
}
