use crate::l10n::t;
use crate::registry::{now_iso8601, Registry, RegistryStore, SecretSlot};
use crate::store::{system_store, SecretStore, StoreError, DEFAULT_SERVICE};
use serde::Serialize;
use std::fmt;
use std::path::PathBuf;

/// What anyone (UI, CLI, agent) may see about a key: never the value.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SlotInfo {
    pub name: String,
    pub reference: String,
    pub has_value: bool,
    pub projects: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum KeyfoldError {
    InvalidName(String),
    InvalidProject(String),
    UnknownSecret(String),
    EmptyValue,
    Store(String),
    Io(String),
}

impl fmt::Display for KeyfoldError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = match self {
            Self::InvalidName(n) => t(
                &format!("Nombre no válido: {n}. Usa MAYUSCULAS_CON_GUIONES."),
                &format!("Invalid name: {n}. Use UPPER_SNAKE_CASE."),
            ),
            Self::InvalidProject(p) => t(
                &format!("Proyecto no válido: '{p}'. Usa letras, números, punto, guion o guion bajo."),
                &format!("Invalid project: '{p}'. Use letters, digits, dot, dash or underscore."),
            ),
            Self::UnknownSecret(n) => t(
                &format!("No existe la clave {n}. Créala con: keyfold create {n}"),
                &format!("No key named {n}. Create it with: keyfold create {n}"),
            ),
            Self::EmptyValue => t("El valor está vacío.", "The value is empty."),
            // Store and I/O errors carry OS codes and paths only, never a value.
            Self::Store(m) | Self::Io(m) => m.clone(),
        };
        f.write_str(&text)
    }
}

impl std::error::Error for KeyfoldError {}

impl From<StoreError> for KeyfoldError {
    fn from(e: StoreError) -> Self {
        Self::Store(e.0)
    }
}

impl From<std::io::Error> for KeyfoldError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e.to_string())
    }
}

/// Where the registry lives and which credential-store service to use.
#[derive(Debug, Clone, PartialEq)]
pub struct Config {
    pub home: PathBuf,
    pub service: String,
}

impl Config {
    /// `KEYFOLD_HOME` / `KEYFOLD_SERVICE` override the defaults (tests).
    pub fn from_env() -> Self {
        Self {
            home: RegistryStore::default_directory(),
            service: std::env::var("KEYFOLD_SERVICE").ok().filter(|s| !s.is_empty()).unwrap_or_else(|| DEFAULT_SERVICE.into()),
        }
    }
}

pub struct Keyfold {
    store: Box<dyn SecretStore>,
    registry: RegistryStore,
}

impl Keyfold {
    pub fn new(store: Box<dyn SecretStore>, registry: RegistryStore) -> Self {
        Self { store, registry }
    }

    pub fn open(config: &Config) -> Self {
        Self::new(system_store(&config.service), RegistryStore::new(&config.home))
    }

    /// Creates an empty slot and, if given, assigns it to a project. Existing slots keep their value.
    pub fn create_slot(&self, name: &str, project: Option<&str>) -> Result<SlotInfo, KeyfoldError> {
        validate_name(name)?;
        if let Some(p) = project {
            validate_project(p)?;
        }
        let mut registry = self.registry.load()?;
        registry.secrets.entry(name.to_string()).or_insert_with(|| SecretSlot { created_at: now_iso8601() });
        if let Some(p) = project {
            add(name, p, &mut registry);
        }
        self.registry.save(&registry)?;
        Ok(self.info(name, &registry))
    }

    pub fn set_value(&self, name: &str, value: &str) -> Result<(), KeyfoldError> {
        let registry = self.registry.load()?;
        if !registry.secrets.contains_key(name) {
            return Err(KeyfoldError::UnknownSecret(name.into()));
        }
        if value.is_empty() {
            return Err(KeyfoldError::EmptyValue);
        }
        Ok(self.store.set(name, value)?)
    }

    pub fn assign(&self, name: &str, project: &str) -> Result<SlotInfo, KeyfoldError> {
        validate_project(project)?;
        let mut registry = self.registry.load()?;
        if !registry.secrets.contains_key(name) {
            return Err(KeyfoldError::UnknownSecret(name.into()));
        }
        add(name, project, &mut registry);
        self.registry.save(&registry)?;
        Ok(self.info(name, &registry))
    }

    pub fn list(&self, project: Option<&str>) -> Result<Vec<SlotInfo>, KeyfoldError> {
        let registry = self.registry.load()?;
        let mut names: Vec<String> = match project {
            Some(p) => registry.projects.get(p).cloned().unwrap_or_default(),
            None => registry.secrets.keys().cloned().collect(),
        };
        names.sort();
        Ok(names.iter().map(|n| self.info(n, &registry)).collect())
    }

    pub fn slot(&self, name: &str) -> Result<SlotInfo, KeyfoldError> {
        let registry = self.registry.load()?;
        if !registry.secrets.contains_key(name) {
            return Err(KeyfoldError::UnknownSecret(name.into()));
        }
        Ok(self.info(name, &registry))
    }

    pub fn delete(&self, name: &str) -> Result<(), KeyfoldError> {
        let mut registry = self.registry.load()?;
        if !registry.secrets.contains_key(name) {
            return Err(KeyfoldError::UnknownSecret(name.into()));
        }
        self.store.delete(name)?;
        registry.secrets.remove(name);
        for names in registry.projects.values_mut() {
            names.retain(|n| n != name);
        }
        registry.projects.retain(|_, names| !names.is_empty());
        self.registry.save(&registry)?;
        Ok(())
    }

    /// (name, value) for the keys assigned to `project` that have a value. Only for `run`,
    /// whose child output is redacted before it reaches the caller.
    pub fn values_for(&self, project: &str) -> Result<Vec<(String, String)>, KeyfoldError> {
        let registry = self.registry.load()?;
        let mut names = registry.projects.get(project).cloned().unwrap_or_default();
        names.sort();
        let mut out = Vec::new();
        for name in names {
            if let Some(value) = self.store.get(&name)? {
                out.push((name, value));
            }
        }
        Ok(out)
    }

    /// The stored value, for the reveal button of the desktop app only (an explicit click
    /// by the user). The CLI and the MCP server must never call this.
    pub fn reveal(&self, name: &str) -> Result<Option<String>, KeyfoldError> {
        let registry = self.registry.load()?;
        if !registry.secrets.contains_key(name) {
            return Err(KeyfoldError::UnknownSecret(name.into()));
        }
        Ok(self.store.get(name)?)
    }

    fn info(&self, name: &str, registry: &Registry) -> SlotInfo {
        let projects = registry
            .projects
            .iter()
            .filter(|(_, names)| names.iter().any(|n| n == name))
            .map(|(p, _)| p.clone())
            .collect();
        SlotInfo {
            name: name.into(),
            reference: format!("keyfold://{name}"),
            has_value: self.store.has_value(name),
            projects,
        }
    }
}

fn add(name: &str, project: &str, registry: &mut Registry) {
    let names = registry.projects.entry(project.to_string()).or_default();
    if !names.iter().any(|n| n == name) {
        names.push(name.to_string());
        names.sort();
    }
}

/// `^[A-Z_][A-Z0-9_]*$`
fn validate_name(name: &str) -> Result<(), KeyfoldError> {
    let mut chars = name.chars();
    let ok = matches!(chars.next(), Some(c) if c.is_ascii_uppercase() || c == '_')
        && chars.all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_');
    if ok { Ok(()) } else { Err(KeyfoldError::InvalidName(name.into())) }
}

/// `^[A-Za-z0-9._-]+$`
fn validate_project(project: &str) -> Result<(), KeyfoldError> {
    let ok = !project.is_empty() && project.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'));
    if ok { Ok(()) } else { Err(KeyfoldError::InvalidProject(project.into())) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::MemoryStore;

    fn keyfold() -> (Keyfold, PathBuf) {
        // Tests run in parallel: the clock alone can repeat, the counter cannot.
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("keyfold-core-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        (Keyfold::new(Box::new(MemoryStore::default()), RegistryStore::new(&dir)), dir)
    }

    #[test]
    fn slot_without_value_lists_name_only() {
        let (l, dir) = keyfold();
        let info = l.create_slot("EXA_API_KEY", Some("demo")).unwrap();
        assert_eq!(info.reference, "keyfold://EXA_API_KEY");
        assert!(!info.has_value);
        assert_eq!(l.list(None).unwrap(), vec![info]);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn project_gets_only_assigned_values_and_registry_has_no_value() {
        let (l, dir) = keyfold();
        l.create_slot("EXA_API_KEY", Some("alpha")).unwrap();
        l.create_slot("OTHER_KEY", Some("beta")).unwrap();
        l.set_value("EXA_API_KEY", "secret-exa").unwrap();
        l.set_value("OTHER_KEY", "secret-other").unwrap();
        assert_eq!(l.values_for("alpha").unwrap(), vec![("EXA_API_KEY".into(), "secret-exa".into())]);
        assert_eq!(l.values_for("beta").unwrap(), vec![("OTHER_KEY".into(), "secret-other".into())]);
        assert!(l.values_for("gamma").unwrap().is_empty());
        let file = std::fs::read_to_string(dir.join("registry.json")).unwrap();
        assert!(!file.contains("secret-exa") && !file.contains("secret-other"));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn reveal_returns_the_stored_value_or_none() {
        let (l, dir) = keyfold();
        l.create_slot("A_KEY", None).unwrap();
        assert_eq!(l.reveal("A_KEY").unwrap(), None);
        l.set_value("A_KEY", "secret-a").unwrap();
        assert_eq!(l.reveal("A_KEY").unwrap().as_deref(), Some("secret-a"));
        assert_eq!(l.reveal("MISSING"), Err(KeyfoldError::UnknownSecret("MISSING".into())));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn validation_and_errors() {
        let (l, dir) = keyfold();
        assert_eq!(l.create_slot("bad name", None), Err(KeyfoldError::InvalidName("bad name".into())));
        assert_eq!(l.set_value("MISSING", "x"), Err(KeyfoldError::UnknownSecret("MISSING".into())));
        l.create_slot("A_KEY", None).unwrap();
        assert_eq!(l.set_value("A_KEY", ""), Err(KeyfoldError::EmptyValue));
        assert_eq!(l.assign("A_KEY", "bad/project").unwrap_err(), KeyfoldError::InvalidProject("bad/project".into()));
        assert_eq!(l.assign("A_KEY", "web").unwrap().projects, vec!["web"]);
        l.delete("A_KEY").unwrap();
        assert!(l.list(None).unwrap().is_empty());
        let _ = std::fs::remove_dir_all(dir);
    }
}
