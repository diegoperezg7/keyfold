//! First load from the `.env` files of your projects: find them in folders you chose, list
//! the variables, store only the ones you tick.
//!
//! The scan returns names, files and flags, never a value. The import reads the chosen files
//! again here, so values go from the file to the credential store without passing through
//! the caller.

use crate::keyfold::{validate_name, validate_project, Keyfold, KeyfoldError, SlotInfo};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

/// Folders below the chosen one that are searched. Projects sit one or two levels down.
const MAX_DEPTH: usize = 6;
/// Folder entries looked at per scan, so a home folder by mistake still ends quickly.
const MAX_ENTRIES: usize = 50_000;
/// Bigger files are not `.env` files.
const MAX_FILE_BYTES: u64 = 256 * 1024;
/// Dependency, build and tool folders. Hidden folders (`.git`, `.venv`, `.next`…) are skipped too.
const SKIP_DIRS: &[&str] = &["node_modules", "target", "dist", "build", "out", "vendor", "venv", "__pycache__", "Pods", "DerivedData"];
/// Folders right under the user's home that never hold projects. Library alone is over
/// 60,000 entries: picking the home folder used to spend the whole budget there.
const HOME_SKIP_DIRS: &[&str] = &["Library", "Movies", "Music", "Pictures"];
/// `.env.<suffix>` files that hold examples, not keys.
const TEMPLATE_SUFFIXES: &[&str] = &["example", "sample", "template", "dist", "defaults", "schema"];
/// Name parts of variables that usually hold a secret.
const SECRET_HINTS: &[&str] = &["KEY", "TOKEN", "SECRET", "PASSWORD", "PASSWD", "CREDENTIAL", "PRIVATE", "AUTH"];

/// One variable of one file, as the app shows it. Never carries the value.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvCandidate {
    pub name: String,
    pub file: String,
    /// From the folder's `.keyfold` file or link, else the folder name.
    pub project: Option<String>,
    /// Rows of the same name with equal values share it; a different one means a different value.
    pub value_group: usize,
    /// Another file has this name with a different value: only one of them can be stored.
    pub conflict: bool,
    /// Keyfold already has a value under this name: importing replaces it.
    pub exists: bool,
    /// Looks like an example (`your_key_here`, `${OTHER}`) or text that is not random.
    pub placeholder: bool,
    /// Keyfold names are UPPER_SNAKE_CASE; others cannot be imported.
    pub valid_name: bool,
    /// Ticked when the list opens: a secret-looking, real-looking, new key.
    pub suggested: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvScan {
    pub candidates: Vec<EnvCandidate>,
    /// Every `.env` file read. The import only accepts these.
    pub files: Vec<String>,
    /// The entry limit stopped the search early.
    pub truncated: bool,
}

/// A ticked row: which variable, from which file.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct EnvPick {
    pub file: String,
    pub name: String,
}

impl Keyfold {
    /// Finds the `.env` files under `folders` and lists their variables, without values.
    pub fn scan_env(&self, folders: &[PathBuf]) -> Result<EnvScan, KeyfoldError> {
        let mut files = Vec::new();
        let mut budget = MAX_ENTRIES;
        let home = std::env::var_os("HOME").map(PathBuf::from);
        for folder in folders {
            find_env_files(folder, home.as_deref(), 0, &mut budget, &mut files);
        }
        files.sort();
        files.dedup();
        let stored: BTreeSet<String> = self.list(None)?.into_iter().filter(|k| k.has_value).map(|k| k.name).collect();

        // name -> distinct values in order of appearance; only used to compare, then dropped.
        let mut values: BTreeMap<String, Vec<String>> = BTreeMap::new();
        let mut candidates = Vec::new();
        for file in &files {
            let project = self.project_of(file);
            for (name, value) in read_env_file(file) {
                let seen = values.entry(name.clone()).or_default();
                let value_group = seen.iter().position(|v| *v == value).unwrap_or_else(|| {
                    seen.push(value.clone());
                    seen.len() - 1
                });
                let valid_name = validate_name(&name).is_ok();
                let placeholder = looks_like_placeholder(&value);
                let exists = stored.contains(&name);
                let suggested = valid_name && !placeholder && !exists && looks_secret(&name, &value);
                candidates.push(EnvCandidate {
                    name,
                    file: file.to_string_lossy().into_owned(),
                    project: project.clone(),
                    value_group,
                    conflict: false,
                    exists,
                    placeholder,
                    valid_name,
                    suggested,
                });
            }
        }
        for c in &mut candidates {
            c.conflict = values.get(&c.name).is_some_and(|v| v.len() > 1);
        }
        // With a conflict, only the first value is suggested.
        for c in &mut candidates {
            c.suggested &= c.value_group == 0;
        }
        Ok(EnvScan {
            candidates,
            files: files.iter().map(|f| f.to_string_lossy().into_owned()).collect(),
            truncated: budget == 0,
        })
    }

    /// Stores the picked variables, reading their files again. `allowed` is the file list of
    /// the scan the picks come from: any other file is refused. Each name takes one value and
    /// is assigned to the project of every file it was picked from.
    pub fn import_env(&self, allowed: &[String], picks: &[EnvPick]) -> Result<Vec<SlotInfo>, KeyfoldError> {
        let mut chosen: BTreeMap<String, (String, Vec<Option<String>>)> = BTreeMap::new();
        for pick in picks {
            if !allowed.contains(&pick.file) {
                return Err(KeyfoldError::Io(format!("{} was not in the scan", pick.file)));
            }
            validate_name(&pick.name)?;
            let value = read_env_file(Path::new(&pick.file))
                .into_iter()
                .find(|(n, _)| *n == pick.name)
                .map(|(_, v)| v)
                .ok_or_else(|| KeyfoldError::Io(format!("{} is no longer in {}", pick.name, pick.file)))?;
            let project = self.project_of(Path::new(&pick.file));
            match chosen.get_mut(&pick.name) {
                Some((first, projects)) => {
                    if *first != value {
                        return Err(KeyfoldError::EnvConflict(pick.name.clone()));
                    }
                    projects.push(project);
                }
                None => {
                    chosen.insert(pick.name.clone(), (value, vec![project]));
                }
            }
        }
        let mut imported = Vec::new();
        for (name, (value, projects)) in chosen {
            self.create_slot(&name, None)?;
            self.set_value(&name, &value)?;
            for project in projects.into_iter().flatten() {
                self.assign(&name, &project)?;
            }
            imported.push(self.slot(&name)?);
        }
        Ok(imported)
    }

    fn project_of(&self, file: &Path) -> Option<String> {
        let dir = file.parent()?;
        if let Ok(Some(project)) = self.project_for(dir) {
            return Some(project);
        }
        let name: String = dir
            .file_name()?
            .to_string_lossy()
            .trim()
            .chars()
            .map(|c| if c.is_whitespace() { '-' } else { c })
            .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
            .collect();
        validate_project(&name).ok().map(|_| name)
    }
}

fn find_env_files(dir: &Path, home: Option<&Path>, depth: usize, budget: &mut usize, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    let at_home = home.is_some_and(|h| h == dir);
    for entry in entries.flatten() {
        if *budget == 0 {
            return;
        }
        *budget -= 1;
        // file_type does not follow symlinks: linked files and folders are skipped.
        let Ok(kind) = entry.file_type() else { continue };
        let name = entry.file_name().to_string_lossy().into_owned();
        if kind.is_dir() {
            let skip = name.starts_with('.') || SKIP_DIRS.contains(&name.as_str()) || (at_home && HOME_SKIP_DIRS.contains(&name.as_str()));
            if depth < MAX_DEPTH && !skip {
                find_env_files(&entry.path(), home, depth + 1, budget, out);
            }
        } else if kind.is_file() && is_env_file(&name) && entry.metadata().is_ok_and(|m| m.len() <= MAX_FILE_BYTES) {
            out.push(entry.path());
        }
    }
}

/// `.env`, `.env.local`, `.env.production`…, but not `.env.example` and other templates.
fn is_env_file(name: &str) -> bool {
    match name.strip_prefix(".env") {
        Some("") => true,
        Some(rest) => rest
            .strip_prefix('.')
            .is_some_and(|suffix| !suffix.is_empty() && !suffix.split('.').any(|part| TEMPLATE_SUFFIXES.contains(&part.to_ascii_lowercase().as_str()))),
        None => false,
    }
}

fn read_env_file(path: &Path) -> Vec<(String, String)> {
    fs::read_to_string(path).map(|text| parse(&text)).unwrap_or_default()
}

/// `NAME=value` lines: `export` prefix, `#` comments, single and double quotes. Lines it does
/// not understand (multi-line values, no `=`) are skipped; empty values too. The last
/// definition of a name wins, as in dotenv.
pub(crate) fn parse(text: &str) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let line = line.strip_prefix("export ").map(str::trim_start).unwrap_or(line);
        let Some((name, raw)) = line.split_once('=') else { continue };
        let name = name.trim();
        let identifier = name.chars().next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
            && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
        if !identifier {
            continue;
        }
        let Some(value) = parse_value(raw.trim_start()) else { continue };
        if value.is_empty() {
            continue;
        }
        out.retain(|(n, _)| n != name);
        out.push((name.to_string(), value));
    }
    out
}

fn parse_value(raw: &str) -> Option<String> {
    if let Some(rest) = raw.strip_prefix('\'') {
        return rest.find('\'').map(|end| rest[..end].to_string());
    }
    if let Some(rest) = raw.strip_prefix('"') {
        let mut value = String::new();
        let mut chars = rest.chars();
        while let Some(c) = chars.next() {
            match c {
                '"' => return Some(value),
                '\\' => match chars.next()? {
                    'n' => value.push('\n'),
                    other => value.push(other),
                },
                c => value.push(c),
            }
        }
        // No closing quote on this line: a multi-line value, not supported.
        return None;
    }
    let value = match raw.find(" #") {
        Some(i) => &raw[..i],
        None => raw,
    };
    Some(value.trim_end().to_string())
}

fn looks_secret(name: &str, value: &str) -> bool {
    SECRET_HINTS.iter().any(|hint| name.contains(hint)) || url_with_password(value)
}

/// `scheme://user:password@host…`
fn url_with_password(value: &str) -> bool {
    let Some((_, rest)) = value.split_once("://") else { return false };
    let authority = rest.split('/').next().unwrap_or("");
    authority.split_once('@').is_some_and(|(user, _)| user.contains(':'))
}

/// Same rule as `looksLikePlaceholder` in the app (src/logic.ts): example markers, or text
/// that is not random (under 3 bits per character).
pub(crate) fn looks_like_placeholder(value: &str) -> bool {
    let v = value.trim();
    if v.is_empty() {
        return false;
    }
    let lower = v.to_lowercase();
    let words = ["1234", "abcd", "example", "sample", "dummy", "placeholder", "redacted", "changeme", "your", "${", "{{"];
    words.iter().any(|w| lower.contains(w))
        || run_of(v, 'x', 4)
        || run_of(v, 'X', 4)
        || run_of(v, '*', 3)
        || run_of(v, '0', 6)
        || v.contains("...")
        || (v.find('<').is_some_and(|i| v[i..].contains('>')))
        || shannon_entropy(v) < 3.0
}

fn run_of(s: &str, c: char, n: usize) -> bool {
    s.contains(&c.to_string().repeat(n))
}

fn shannon_entropy(s: &str) -> f64 {
    let mut counts: BTreeMap<char, usize> = BTreeMap::new();
    let mut total = 0usize;
    for c in s.chars() {
        *counts.entry(c).or_default() += 1;
        total += 1;
    }
    counts.values().fold(0.0, |h, &n| {
        let p = n as f64 / total as f64;
        h - p * p.log2()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::RegistryStore;
    use crate::store::MemoryStore;

    // Made-up values with enough entropy to not look like placeholders.
    const REAL_A: &str = "q8Zr2LwT9vKx4MnB7cYd";
    const REAL_B: &str = "H3jP6sWf1gQe5uRk8tLz";

    fn temp(tag: &str) -> PathBuf {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("keyfold-dotenv-{tag}-{}-{n}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn keyfold(dir: &Path) -> Keyfold {
        Keyfold::new(Box::new(MemoryStore::default()), RegistryStore::new(&dir.join("home")))
    }

    /// `rel` uses `/`; each part is joined on its own so the path has this OS's separators,
    /// like the ones the scan returns.
    fn write(dir: &Path, rel: &str, text: &str) -> String {
        let path = rel.split('/').fold(dir.to_path_buf(), |p, part| p.join(part));
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, text).unwrap();
        path.to_string_lossy().into_owned()
    }

    #[test]
    fn parses_quotes_comments_and_export() {
        let text = "# c\nexport A_KEY=plain # note\nB_KEY='single # kept'\nC_KEY=\"dq \\\"x\\\" \\n\"\nEMPTY=\nlower_name=v\nnot a line\nMULTI=\"open\nA_KEY=last";
        assert_eq!(
            parse(text),
            vec![
                ("B_KEY".into(), "single # kept".into()),
                ("C_KEY".into(), "dq \"x\" \n".into()),
                ("lower_name".into(), "v".into()),
                ("A_KEY".into(), "last".into()),
            ]
        );
    }

    #[test]
    fn env_files_but_not_templates() {
        assert!(is_env_file(".env") && is_env_file(".env.local") && is_env_file(".env.production"));
        assert!(!is_env_file(".env.example") && !is_env_file(".env.local.sample") && !is_env_file(".envrc") && !is_env_file("env"));
    }

    #[test]
    fn placeholders_and_secret_names() {
        assert!(looks_like_placeholder("your_api_key_here") && looks_like_placeholder("${OTHER}") && looks_like_placeholder("sk-xxxxxxxx"));
        assert!(looks_like_placeholder("true") && !looks_like_placeholder(REAL_A));
        assert!(looks_secret("OPENAI_API_KEY", "x") && looks_secret("DATABASE_URL", "postgres://u:p4ss@db/x"));
        assert!(!looks_secret("PORT", "3000") && !looks_secret("SITE_URL", "https://example.com/a@b"));
    }

    #[test]
    fn scan_skips_dependencies_hidden_folders_and_templates_and_never_returns_values() {
        let dir = temp("scan");
        write(&dir, "web/.env", &format!("API_KEY={REAL_A}\nPORT=3000\n"));
        write(&dir, "web/.env.example", "API_KEY=your_key_here\n");
        write(&dir, "web/node_modules/pkg/.env", &format!("DEP_KEY={REAL_B}\n"));
        write(&dir, "web/.git/.env", &format!("GIT_KEY={REAL_B}\n"));
        let l = keyfold(&dir);
        let scan = l.scan_env(&[dir.clone()]).unwrap();
        assert_eq!(scan.files.len(), 1);
        let names: Vec<_> = scan.candidates.iter().map(|c| (c.name.as_str(), c.suggested)).collect();
        assert_eq!(names, vec![("API_KEY", true), ("PORT", false)]);
        assert_eq!(scan.candidates[0].project.as_deref(), Some("web"));
        let json = serde_json::to_string(&scan).unwrap();
        assert!(!json.contains(REAL_A) && !json.contains(REAL_B), "a value reached the scan result");
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn finds_dot_env_files_but_not_inside_hidden_folders_and_skips_home_media_folders() {
        let dir = temp("home");
        write(&dir, ".env", "A_KEY=1\n");
        write(&dir, "app/.env", "A_KEY=1\n");
        write(&dir, ".config/tool/.env", "A_KEY=1\n");
        write(&dir, "Library/app/.env", "A_KEY=1\n");
        write(&dir, "Pictures/.env", "A_KEY=1\n");
        write(&dir, "work/Library/.env", "A_KEY=1\n");
        let found = |home: Option<&Path>| {
            let (mut out, mut budget) = (Vec::new(), MAX_ENTRIES);
            find_env_files(&dir, home, 0, &mut budget, &mut out);
            let mut rel: Vec<String> = out.iter().map(|p| p.strip_prefix(&dir).unwrap().to_string_lossy().replace('\\', "/")).collect();
            rel.sort();
            rel
        };
        // Picked as the home folder: Library and Pictures right under it are skipped, not deeper ones.
        assert_eq!(found(Some(&dir)), vec![".env", "app/.env", "work/Library/.env"]);
        // Any other folder: they are ordinary folders.
        assert_eq!(found(None), vec![".env", "Library/app/.env", "Pictures/.env", "app/.env", "work/Library/.env"]);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn a_scan_that_hits_the_entry_limit_says_so() {
        let dir = temp("budget");
        write(&dir, "a/.env", "A_KEY=1\n");
        let mut out = Vec::new();
        let mut budget = 1;
        find_env_files(&dir, None, 0, &mut budget, &mut out);
        assert_eq!(budget, 0);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn same_value_in_two_projects_is_one_key_and_different_values_conflict() {
        let dir = temp("dup");
        let a = write(&dir, "alpha/.env", &format!("SHARED_KEY={REAL_A}\nCLASH_TOKEN={REAL_A}\n"));
        let b = write(&dir, "beta/.env", &format!("SHARED_KEY={REAL_A}\nCLASH_TOKEN={REAL_B}\n"));
        let l = keyfold(&dir);
        let scan = l.scan_env(&[dir.clone()]).unwrap();
        let clash: Vec<_> = scan.candidates.iter().filter(|c| c.name == "CLASH_TOKEN").map(|c| (c.value_group, c.conflict, c.suggested)).collect();
        assert_eq!(clash, vec![(0, true, true), (1, true, false)]);
        assert!(scan.candidates.iter().filter(|c| c.name == "SHARED_KEY").all(|c| c.value_group == 0 && !c.conflict));

        let pick = |file: &str, name: &str| EnvPick { file: file.into(), name: name.into() };
        let both = [pick(&a, "CLASH_TOKEN"), pick(&b, "CLASH_TOKEN")];
        assert_eq!(l.import_env(&scan.files, &both), Err(KeyfoldError::EnvConflict("CLASH_TOKEN".into())));

        let imported = l.import_env(&scan.files, &[pick(&a, "SHARED_KEY"), pick(&b, "SHARED_KEY"), pick(&b, "CLASH_TOKEN")]).unwrap();
        assert_eq!(imported.iter().map(|k| (k.name.as_str(), k.projects.clone())).collect::<Vec<_>>(), vec![
            ("CLASH_TOKEN", vec!["beta".to_string()]),
            ("SHARED_KEY", vec!["alpha".to_string(), "beta".to_string()]),
        ]);
        assert_eq!(l.reveal("CLASH_TOKEN").unwrap().as_deref(), Some(REAL_B));
        // Only what was picked is stored.
        assert_eq!(l.list(None).unwrap().len(), 2);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn import_refuses_files_outside_the_scan_and_marks_existing_keys() {
        let dir = temp("refuse");
        let inside = write(&dir, "app/.env", &format!("APP_KEY={REAL_A}\n"));
        let outside = write(&dir, "other/secret.txt", &format!("APP_KEY={REAL_B}\n"));
        let l = keyfold(&dir);
        l.create_slot("APP_KEY", None).unwrap();
        l.set_value("APP_KEY", "old-value-1").unwrap();
        let scan = l.scan_env(&[dir.join("app")]).unwrap();
        assert!(scan.candidates[0].exists && !scan.candidates[0].suggested);
        let picked = [EnvPick { file: outside, name: "APP_KEY".into() }];
        assert!(l.import_env(&scan.files, &picked).is_err());
        assert_eq!(l.reveal("APP_KEY").unwrap().as_deref(), Some("old-value-1"));
        l.import_env(&scan.files, &[EnvPick { file: inside, name: "APP_KEY".into() }]).unwrap();
        assert_eq!(l.reveal("APP_KEY").unwrap().as_deref(), Some(REAL_A));
        let _ = fs::remove_dir_all(dir);
    }
}
