//! Names and project assignments only. Never holds a value.
//! Same JSON as the Swift version (`registry.json`), so both read each other's file.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Registry {
    /// project name -> secret names assigned to it
    #[serde(default)]
    pub projects: BTreeMap<String, Vec<String>>,
    #[serde(default)]
    pub secrets: BTreeMap<String, SecretSlot>,
    /// Absolute folder path -> project, for `keyfold run` without `--project`. Written only when
    /// there is at least one link, so registries without links keep the exact Swift-era shape.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub folders: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SecretSlot {
    #[serde(rename = "createdAt")]
    pub created_at: String,
}

pub struct RegistryStore {
    path: PathBuf,
}

impl RegistryStore {
    pub fn new(directory: &Path) -> Self {
        Self { path: directory.join("registry.json") }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// `KEYFOLD_HOME` overrides the location (tests). Otherwise the per-user app data folder.
    pub fn default_directory() -> PathBuf {
        if let Some(home) = std::env::var_os("KEYFOLD_HOME").filter(|v| !v.is_empty()) {
            return PathBuf::from(home);
        }
        #[cfg(target_os = "macos")]
        if let Some(home) = std::env::var_os("HOME") {
            // Folder name of the original prototype, kept so existing registries keep working.
            return PathBuf::from(home).join("Library/Application Support/Llavero");
        }
        #[cfg(windows)]
        if let Some(appdata) = std::env::var_os("APPDATA") {
            return PathBuf::from(appdata).join("Keyfold");
        }
        if let Some(config) = std::env::var_os("XDG_CONFIG_HOME") {
            return PathBuf::from(config).join("keyfold");
        }
        PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".config/keyfold")
    }

    pub fn load(&self) -> io::Result<Registry> {
        match fs::read(&self.path) {
            Ok(bytes) => serde_json::from_slice(&bytes).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e)),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Registry::default()),
            Err(e) => Err(e),
        }
    }

    /// Atomic write (temp file + rename), readable only by the user on Unix.
    pub fn save(&self, registry: &Registry) -> io::Result<()> {
        if let Some(dir) = self.path.parent() {
            fs::create_dir_all(dir)?;
            // Key and project names are not secret, but they are nobody else's business.
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(dir, fs::Permissions::from_mode(0o700))?;
            }
        }
        let json = serde_json::to_vec_pretty(registry).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        // Unique temp name: the app, the MCP server and the CLI may save at the same time.
        static WRITES: AtomicU64 = AtomicU64::new(0);
        let n = WRITES.fetch_add(1, Ordering::Relaxed);
        let tmp = self.path.with_extension(format!("json.{}.{n}.tmp", std::process::id()));
        fs::write(&tmp, json)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&tmp, fs::Permissions::from_mode(0o600))?;
        }
        fs::rename(&tmp, &self.path)
    }
}

/// UTC timestamp like `2026-11-05T12:08:48Z` (what Swift's `.iso8601` reads and writes).
pub fn now_iso8601() -> String {
    let secs = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
    format_iso8601(secs)
}

fn format_iso8601(secs: i64) -> String {
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    format!("{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z", rem / 3600, (rem % 3600) / 60, rem % 60)
}

/// Days since 1970-01-01 to (year, month, day). Howard Hinnant's algorithm.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = (if z >= 0 { z } else { z - 146_096 }) / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let year = yoe as i64 + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_iso8601() {
        assert_eq!(format_iso8601(0), "1970-01-01T00:00:00Z");
        assert_eq!(format_iso8601(951_782_400), "2000-02-29T00:00:00Z");
        assert_eq!(format_iso8601(1_793_880_528), "2026-11-05T12:08:48Z");
    }

    #[test]
    fn reads_registry_written_by_swift_version() {
        let swift = r#"{
          "projects" : { "demo" : [ "EXA_API_KEY" ] },
          "secrets" : { "EXA_API_KEY" : { "createdAt" : "2026-11-05T12:08:48Z" } }
        }"#;
        let registry: Registry = serde_json::from_str(swift).unwrap();
        assert_eq!(registry.projects["demo"], vec!["EXA_API_KEY"]);
        assert_eq!(registry.secrets["EXA_API_KEY"].created_at, "2026-11-05T12:08:48Z");
    }

    #[test]
    fn folders_are_optional_and_only_written_when_present() {
        let mut registry: Registry = serde_json::from_str(r#"{ "projects": {}, "secrets": {} }"#).unwrap();
        assert!(registry.folders.is_empty());
        assert!(!serde_json::to_string(&registry).unwrap().contains("folders"));
        registry.folders.insert("/repo".into(), "web".into());
        let back: Registry = serde_json::from_str(&serde_json::to_string(&registry).unwrap()).unwrap();
        assert_eq!(back.folders["/repo"], "web");
    }

    #[test]
    fn save_and_load_round_trip() {
        let dir = std::env::temp_dir().join(format!("keyfold-registry-{}", std::process::id()));
        let store = RegistryStore::new(&dir);
        assert_eq!(store.load().unwrap(), Registry::default());
        let mut registry = Registry::default();
        registry.secrets.insert("A_KEY".into(), SecretSlot { created_at: now_iso8601() });
        store.save(&registry).unwrap();
        assert_eq!(store.load().unwrap(), registry);

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = |p: &Path| fs::metadata(p).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode(&dir), 0o700, "registry folder must be private");
            assert_eq!(mode(store.path()), 0o600, "registry file must be private");
        }
        let leftovers = fs::read_dir(&dir).unwrap().filter(|e| e.as_ref().unwrap().path().extension() == Some("tmp".as_ref())).count();
        assert_eq!(leftovers, 0, "no temp files left behind");
        let _ = fs::remove_dir_all(dir);
    }
}
