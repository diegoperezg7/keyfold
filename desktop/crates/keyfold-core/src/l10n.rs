//! ES/EN text, following the OS language: Spanish when the system is in Spanish,
//! English otherwise (and when the language cannot be detected).

use std::sync::OnceLock;

pub fn is_spanish() -> bool {
    static SPANISH: OnceLock<bool> = OnceLock::new();
    *SPANISH.get_or_init(detect)
}

pub fn t(es: &str, en: &str) -> String {
    if is_spanish() { es } else { en }.to_string()
}

/// Language code passed to the UI so both halves agree.
pub fn lang() -> &'static str {
    if is_spanish() {
        "es"
    } else {
        "en"
    }
}

fn detect() -> bool {
    if let Ok(forced) = std::env::var("KEYFOLD_LANG") {
        return forced.starts_with("es");
    }
    #[cfg(target_os = "macos")]
    if let Some(first) = macos_first_language() {
        return first.starts_with("es");
    }
    #[cfg(windows)]
    return windows_ui_is_spanish();
    #[cfg(not(windows))]
    {
        for key in ["LC_ALL", "LANG"] {
            if let Ok(value) = std::env::var(key) {
                if !value.is_empty() && value != "C" && value != "POSIX" {
                    return value.starts_with("es");
                }
            }
        }
        false
    }
}

/// Windows display language. LANG is usually unset there, so ask the OS.
#[cfg(windows)]
fn windows_ui_is_spanish() -> bool {
    const LANG_SPANISH: u16 = 0x0a;
    let lang_id = unsafe { windows_sys::Win32::Globalization::GetUserDefaultUILanguage() };
    lang_id & 0x3ff == LANG_SPANISH
}

/// First entry of `defaults read -g AppleLanguages`, e.g. "es-ES".
#[cfg(target_os = "macos")]
fn macos_first_language() -> Option<String> {
    let output = std::process::Command::new("/usr/bin/defaults")
        .args(["read", "-g", "AppleLanguages"])
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&output.stdout);
    let line = text.lines().nth(1)?;
    let lang = line.trim().trim_end_matches(',').trim_matches('"');
    (!lang.is_empty()).then(|| lang.to_string())
}
