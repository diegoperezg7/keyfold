use crate::l10n::t;
use crate::registry::{now_iso8601, Registry, RegistryStore, SecretSlot};
use crate::store::{system_store, SecretStore, StoreError, DEFAULT_SERVICE};
use serde::Serialize;
use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};

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
    /// Not an `age1…` public key.
    InvalidRecipient(String),
    /// A share package made for other people's computers.
    NotForThisDevice,
    /// Not a share package, or damaged.
    InvalidPackage,
    /// None of the chosen keys has a value.
    NothingToShare,
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
            Self::InvalidRecipient(r) => t(
                &format!("Clave pública no válida: '{r}'. Debe empezar por age1 (keyfold id la muestra)."),
                &format!("Invalid public key: '{r}'. It must start with age1 (keyfold id shows it)."),
            ),
            Self::NotForThisDevice => t(
                "Este paquete no es para este equipo: se cifró para otra clave pública.",
                "This package is not for this computer: it was encrypted to another public key.",
            ),
            Self::InvalidPackage => t("No es un paquete de Keyfold válido.", "Not a valid Keyfold package."),
            Self::NothingToShare => t("Ninguna de esas claves tiene valor.", "None of those keys has a value."),
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
    pub(crate) store: Box<dyn SecretStore>,
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

    /// Takes a key out of a project. A project left with no keys disappears, like after `delete`.
    /// The key and its value stay. Not assigned to that project: nothing changes.
    pub fn unassign(&self, name: &str, project: &str) -> Result<SlotInfo, KeyfoldError> {
        let mut registry = self.registry.load()?;
        if !registry.secrets.contains_key(name) {
            return Err(KeyfoldError::UnknownSecret(name.into()));
        }
        if let Some(names) = registry.projects.get_mut(project) {
            names.retain(|n| n != name);
            if names.is_empty() {
                registry.projects.remove(project);
            }
            self.registry.save(&registry)?;
        }
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

    /// Links a folder to a project: `keyfold run` there (or below) uses it without `--project`.
    /// Stored as the canonical absolute path, so symlinks and `..` resolve to one entry.
    pub fn link_folder(&self, folder: &Path, project: &str) -> Result<String, KeyfoldError> {
        validate_project(project)?;
        let path = canonical(folder)?;
        let mut registry = self.registry.load()?;
        registry.folders.insert(path.clone(), project.to_string());
        self.registry.save(&registry)?;
        Ok(path)
    }

    /// Removes a folder link. Returns whether there was one. Works for folders that no longer exist.
    pub fn unlink_folder(&self, folder: &Path) -> Result<bool, KeyfoldError> {
        let path = canonical(folder).unwrap_or_else(|_| folder.to_string_lossy().into_owned());
        let mut registry = self.registry.load()?;
        let removed = registry.folders.remove(&path).is_some();
        if removed {
            self.registry.save(&registry)?;
        }
        Ok(removed)
    }

    /// Folder path -> project, sorted by path.
    pub fn folders(&self) -> Result<BTreeMap<String, String>, KeyfoldError> {
        Ok(self.registry.load()?.folders)
    }

    /// The project for a folder: from it up to the root, the nearest `.keyfold` file or linked
    /// folder wins; at the same level the file (shared with the team) wins over the local link.
    pub fn project_for(&self, dir: &Path) -> Result<Option<String>, KeyfoldError> {
        let folders = self.registry.load()?.folders;
        let start = std::fs::canonicalize(dir).unwrap_or_else(|_| dir.to_path_buf());
        for level in start.ancestors() {
            if let Some(project) = project_file_in(level)? {
                return Ok(Some(project));
            }
            if let Some(project) = folders.get(&*level.to_string_lossy()) {
                return Ok(Some(project.clone()));
            }
        }
        Ok(None)
    }

    /// Every project some key is assigned to, sorted.
    pub fn projects(&self) -> Result<Vec<String>, KeyfoldError> {
        Ok(self.registry.load()?.projects.into_keys().collect())
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

/// File that ties a folder to a project: `keyfold run` without `--project` uses it.
pub const PROJECT_FILE: &str = ".keyfold";

/// The project named in the nearest `.keyfold`, looking in `start` and then each parent.
/// The file holds the project name on its first line that is not blank or a `#` comment.
/// A file with no name is skipped; a file with an invalid name is an error, not a silent miss.
pub fn project_for_dir(start: &Path) -> Result<Option<String>, KeyfoldError> {
    for dir in start.ancestors() {
        if let Some(project) = project_file_in(dir)? {
            return Ok(Some(project));
        }
    }
    Ok(None)
}

/// The project named by `dir/.keyfold`, if that file exists and names one.
fn project_file_in(dir: &Path) -> Result<Option<String>, KeyfoldError> {
    let path = dir.join(PROJECT_FILE);
    if !path.is_file() {
        return Ok(None);
    }
    let text = std::fs::read_to_string(&path)?;
    let Some(name) = text.lines().map(str::trim).find(|l| !l.is_empty() && !l.starts_with('#')) else {
        return Ok(None);
    };
    validate_project(name)?;
    Ok(Some(name.to_string()))
}

fn canonical(folder: &Path) -> Result<String, KeyfoldError> {
    Ok(std::fs::canonicalize(folder)?.to_string_lossy().into_owned())
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
    fn projects_lists_every_assigned_project() {
        let (l, dir) = keyfold();
        assert!(l.projects().unwrap().is_empty());
        l.create_slot("A_KEY", Some("web")).unwrap();
        l.create_slot("B_KEY", Some("api")).unwrap();
        l.assign("A_KEY", "api").unwrap();
        assert_eq!(l.projects().unwrap(), vec!["api", "web"]);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn unassign_removes_the_key_from_one_project_only() {
        let (l, dir) = keyfold();
        l.create_slot("A_KEY", Some("web")).unwrap();
        l.create_slot("B_KEY", Some("web")).unwrap();
        l.assign("A_KEY", "wbe").unwrap();
        l.set_value("A_KEY", "secret-a").unwrap();

        // A typo project with one key disappears once the key leaves it.
        assert_eq!(l.unassign("A_KEY", "wbe").unwrap().projects, vec!["web"]);
        assert_eq!(l.projects().unwrap(), vec!["web"]);
        assert_eq!(l.unassign("A_KEY", "web").unwrap().projects, Vec::<String>::new());
        assert_eq!(l.list(Some("web")).unwrap().iter().map(|s| s.name.as_str()).collect::<Vec<_>>(), vec!["B_KEY"]);
        // The key and its value stay; unknown project is a no-op; unknown key is an error.
        assert_eq!(l.reveal("A_KEY").unwrap().as_deref(), Some("secret-a"));
        assert!(l.unassign("A_KEY", "nowhere").unwrap().projects.is_empty());
        assert_eq!(l.unassign("MISSING", "web"), Err(KeyfoldError::UnknownSecret("MISSING".into())));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn linked_folders_resolve_from_subfolders_and_the_file_wins_at_the_same_level() {
        let (l, dir) = keyfold();
        let repo = dir.join("repo");
        let sub = repo.join("src");
        std::fs::create_dir_all(&sub).unwrap();
        assert_eq!(l.project_for(&sub).unwrap(), None);

        let stored = l.link_folder(&sub.join(".."), "web").unwrap();
        assert_eq!(stored, std::fs::canonicalize(&repo).unwrap().to_string_lossy());
        assert_eq!(l.folders().unwrap().get(&stored).map(String::as_str), Some("web"));
        assert_eq!(l.project_for(&sub).unwrap().as_deref(), Some("web"));

        // A nearer link wins; at the same level the shared .keyfold file wins.
        l.link_folder(&sub, "api").unwrap();
        assert_eq!(l.project_for(&sub).unwrap().as_deref(), Some("api"));
        std::fs::write(sub.join(PROJECT_FILE), "shared\n").unwrap();
        assert_eq!(l.project_for(&sub).unwrap().as_deref(), Some("shared"));

        assert!(l.unlink_folder(&repo).unwrap());
        assert!(!l.unlink_folder(&repo).unwrap());
        assert_eq!(l.project_for(&repo).unwrap(), None);
        assert_eq!(l.link_folder(&repo, "bad/name"), Err(KeyfoldError::InvalidProject("bad/name".into())));
        assert!(matches!(l.link_folder(&dir.join("missing"), "web"), Err(KeyfoldError::Io(_))));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn project_file_is_found_here_or_in_a_parent() {
        let (_, dir) = keyfold();
        let sub = dir.join("repo/src/deep");
        std::fs::create_dir_all(&sub).unwrap();
        assert_eq!(project_for_dir(&sub).unwrap(), None);

        std::fs::write(dir.join("repo").join(PROJECT_FILE), "# Keyfold project\n\n  search-bot  \n").unwrap();
        assert_eq!(project_for_dir(&sub).unwrap().as_deref(), Some("search-bot"));
        assert_eq!(project_for_dir(&dir.join("repo")).unwrap().as_deref(), Some("search-bot"));

        // The nearest file wins; one with no name is skipped.
        std::fs::write(sub.join(PROJECT_FILE), "web\n").unwrap();
        assert_eq!(project_for_dir(&sub).unwrap().as_deref(), Some("web"));
        std::fs::write(sub.join(PROJECT_FILE), "# nothing yet\n").unwrap();
        assert_eq!(project_for_dir(&sub).unwrap().as_deref(), Some("search-bot"));

        std::fs::write(sub.join(PROJECT_FILE), "bad/name\n").unwrap();
        assert_eq!(project_for_dir(&sub), Err(KeyfoldError::InvalidProject("bad/name".into())));
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
