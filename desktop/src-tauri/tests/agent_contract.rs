//! Drives the real `keyfold` binary the way a person or an agent would (CLI and MCP),
//! against the real OS credential store, isolated by a throwaway service and registry.
//! Contract: no output ever contains a stored value.

use serde_json::{json, Value};
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

struct Sandbox {
    home: PathBuf,
    service: String,
    outputs: Vec<String>,
}

struct Output {
    status: i32,
    out: String,
    err: String,
}

impl Sandbox {
    fn new(tag: &str) -> Self {
        let id = format!("{tag}-{}-{}", std::process::id(), unique());
        Self {
            home: std::env::temp_dir().join(format!("keyfold-it-{id}")),
            service: format!("com.diego.llavero.tests.{id}"),
            outputs: vec![],
        }
    }

    fn cli(&mut self, args: &[&str], stdin: Option<&str>) -> Output {
        self.cli_in(&std::env::current_dir().unwrap(), args, stdin)
    }

    /// Same as `cli`, run from `dir` (for the `.keyfold` lookup).
    fn cli_in(&mut self, dir: &std::path::Path, args: &[&str], stdin: Option<&str>) -> Output {
        let mut child = Command::new(env!("CARGO_BIN_EXE_keyfold"))
            .args(args)
            .current_dir(dir)
            .env("KEYFOLD_HOME", &self.home)
            .env("KEYFOLD_SERVICE", &self.service)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn keyfold");
        if let Some(input) = stdin {
            child.stdin.as_mut().unwrap().write_all(input.as_bytes()).unwrap();
        }
        drop(child.stdin.take());
        let output = child.wait_with_output().unwrap();
        let result = Output {
            status: output.status.code().unwrap_or(-1),
            out: String::from_utf8_lossy(&output.stdout).into_owned(),
            err: String::from_utf8_lossy(&output.stderr).into_owned(),
        };
        self.outputs.push(result.out.clone());
        self.outputs.push(result.err.clone());
        result
    }

    fn mcp(&mut self, messages: &[Value]) -> Vec<Value> {
        let input: String = messages.iter().map(|m| format!("{m}\n")).collect();
        let result = self.cli(&["mcp"], Some(&input));
        assert_eq!(result.status, 0, "{}", result.err);
        result.out.lines().map(|l| serde_json::from_str(l).expect("json line")).collect()
    }

    fn registry(&self) -> String {
        std::fs::read_to_string(self.home.join("registry.json")).unwrap_or_default()
    }

    fn assert_never_printed(&self, secret: &str) {
        for output in &self.outputs {
            assert!(!output.contains(secret), "value leaked in: {output}");
        }
        assert!(!self.registry().contains(secret), "value leaked into the registry file");
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        for name in ["EXA_API_KEY", "OTHER_KEY", "JWT_SECRET"] {
            let _ = Command::new(env!("CARGO_BIN_EXE_keyfold"))
                .args(["delete", name])
                .env("KEYFOLD_HOME", &self.home)
                .env("KEYFOLD_SERVICE", &self.service)
                .output();
        }
        // The identity `keyfold id` created in this throwaway service.
        #[cfg(target_os = "macos")]
        let _ = Command::new("/usr/bin/security")
            .args(["delete-generic-password", "-s", &self.service, "-a", "keyfold.identity"])
            .output();
        let _ = std::fs::remove_dir_all(&self.home);
    }
}

fn unique() -> u128 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
}

fn secret(tag: &str) -> String {
    format!("{tag}-test-{}", unique())
}

/// sh on Unix, cmd on Windows: compare $EXA_API_KEY with stdin without printing it.
#[cfg(unix)]
fn check_env_equals_stdin(var: &str) -> Vec<String> {
    vec!["/bin/sh".into(), "-c".into(), format!(r#"read v; test "$v" = "${var}""#)]
}

/// Reads a stored value back through `keyfold run` into a file in the sandbox (never to stdout),
/// so the test can check that no output carried it.
#[cfg(unix)]
fn stored_value(s: &mut Sandbox, var: &str, project: &str) -> String {
    let file = s.home.join(format!("{var}.check"));
    let script = format!(r#"printf %s "${var}" > '{}'"#, file.display());
    let ran = s.cli(&["run", "--project", project, "--", "/bin/sh", "-c", &script], None);
    assert_eq!(ran.status, 0, "{}", ran.err);
    std::fs::read_to_string(&file).unwrap()
}

#[cfg(unix)]
#[test]
fn generate_stores_a_random_secret_and_only_returns_the_reference() {
    let mut s = Sandbox::new("generate");
    let made = s.cli(&["generate", "JWT_SECRET", "--project", "demo", "--length", "32"], None);
    assert_eq!(made.status, 0, "{}", made.err);
    assert!(made.out.contains("keyfold://JWT_SECRET"));
    let value = stored_value(&mut s, "JWT_SECRET", "demo");
    assert_eq!(value.len(), 32);
    assert!(value.chars().all(|c| c.is_ascii_alphanumeric()));

    // Never overwrites: the second run fails and the value stays.
    let again = s.cli(&["generate", "JWT_SECRET"], None);
    assert_ne!(again.status, 0);
    assert_eq!(stored_value(&mut s, "JWT_SECRET", "demo"), value);
    assert_ne!(s.cli(&["generate", "OTHER_KEY", "--length", "8"], None).status, 0);

    // Same through MCP.
    let call = |id: i64, args: Value| json!({"jsonrpc":"2.0","id":id,"method":"tools/call","params":{"name":"generate_secret","arguments":args}});
    let replies = s.mcp(&[
        call(1, json!({"name": "OTHER_KEY", "project": "demo"})),
        call(2, json!({"name": "OTHER_KEY"})),
        call(3, json!({"name": "EXA_API_KEY", "length": 8})),
    ]);
    let made = &replies[0]["result"]["structuredContent"];
    assert_eq!(made["reference"], "keyfold://OTHER_KEY");
    assert_eq!(made["hasValue"], json!(true));
    assert_eq!(replies[1]["result"]["isError"], json!(true), "must not overwrite");
    assert_eq!(replies[2]["result"]["isError"], json!(true), "length below the minimum");
    let other = stored_value(&mut s, "OTHER_KEY", "demo");
    assert_eq!(other.len(), 48);
    assert_ne!(other, value);

    s.assert_never_printed(&value);
    s.assert_never_printed(&other);
}

#[test]
fn create_registers_slot_without_value() {
    let mut s = Sandbox::new("create");
    let created = s.cli(&["create", "EXA_API_KEY", "--project", "demo", "--no-prompt"], None);
    assert_eq!(created.status, 0, "{}", created.err);
    assert!(created.out.contains("keyfold://EXA_API_KEY"));
    assert!(s.registry().contains("EXA_API_KEY"));

    let listed = s.cli(&["list"], None);
    assert!(listed.out.contains("EXA_API_KEY") && listed.out.contains("demo"));

    let slot = s.mcp(&[json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"list_secrets","arguments":{}}})]);
    assert_eq!(slot[0]["result"]["structuredContent"]["secrets"][0]["hasValue"], json!(false), "slot must not have a value");
}

#[cfg(unix)]
#[test]
fn no_cli_output_contains_the_value_and_it_survives_restart() {
    let mut s = Sandbox::new("leak");
    let value = secret("exa");
    s.cli(&["create", "EXA_API_KEY", "--project", "demo", "--no-prompt"], None);
    let set = s.cli(&["set", "EXA_API_KEY", "--stdin"], Some(&format!("{value}\n")));
    assert_eq!(set.status, 0, "{}", set.err);
    s.cli(&["assign", "EXA_API_KEY", "--project", "other"], None);
    s.cli(&["list"], None);
    s.cli(&["list", "--project", "demo"], None);
    s.cli(&["create", "EXA_API_KEY", "--project", "demo", "--no-prompt"], None);
    s.cli(&["set", "MISSING", "--stdin"], Some(&value));
    s.cli(&["bogus"], None);

    // A child that tries to print the value on both streams.
    let leak = s.cli(
        &["run", "--project", "demo", "--", "/bin/sh", "-c", r#"printenv EXA_API_KEY; printf 'x=%s' "$EXA_API_KEY" >&2; echo "a $EXA_API_KEY b""#],
        None,
    );
    assert_eq!(leak.status, 0, "{}", leak.err);
    assert!(leak.out.contains("[keyfold:EXA_API_KEY]"), "child must receive the value, masked: {}", leak.out);
    assert!(leak.err.contains("x=[keyfold:EXA_API_KEY]"));

    // Each call is a new process: this one only has the credential store to go on.
    let args = check_env_equals_stdin("EXA_API_KEY");
    let mut run = vec!["run", "--project", "demo", "--"];
    run.extend(args.iter().map(String::as_str));
    assert_eq!(s.cli(&run, Some(&format!("{value}\n"))).status, 0, "value did not survive a new process");

    s.assert_never_printed(&value);
}

#[cfg(unix)]
#[test]
fn project_receives_only_assigned_keys() {
    let mut s = Sandbox::new("projects");
    s.cli(&["create", "EXA_API_KEY", "--project", "alpha", "--no-prompt"], None);
    s.cli(&["create", "OTHER_KEY", "--project", "beta", "--no-prompt"], None);
    s.cli(&["set", "EXA_API_KEY", "--stdin"], Some(&secret("exa")));
    s.cli(&["set", "OTHER_KEY", "--stdin"], Some(&secret("other")));

    let sh = |script: &str| vec!["/bin/sh".to_string(), "-c".to_string(), script.to_string()];
    for (project, script) in [
        ("alpha", r#"test -n "$EXA_API_KEY" && test -z "${OTHER_KEY+set}""#),
        ("beta", r#"test -n "$OTHER_KEY" && test -z "${EXA_API_KEY+set}""#),
        ("gamma", r#"test -z "${EXA_API_KEY+set}" && test -z "${OTHER_KEY+set}""#),
    ] {
        let cmd = sh(script);
        let mut args = vec!["run", "--project", project, "--"];
        args.extend(cmd.iter().map(String::as_str));
        assert_eq!(s.cli(&args, None).status, 0, "project {project} got the wrong keys");
    }
}

#[cfg(unix)]
#[test]
fn run_uses_the_folder_project_file_and_warns_on_unknown_projects() {
    let mut s = Sandbox::new("folder");
    s.cli(&["create", "EXA_API_KEY", "--project", "alpha", "--no-prompt"], None);
    s.cli(&["set", "EXA_API_KEY", "--stdin"], Some(&secret("exa")));

    let repo = s.home.join("repo");
    let sub = repo.join("src");
    std::fs::create_dir_all(&sub).unwrap();
    std::fs::write(repo.join(".keyfold"), "alpha\n").unwrap();

    let has_key = ["run", "--", "/bin/sh", "-c", r#"test -n "$EXA_API_KEY""#];
    let result = s.cli_in(&sub, &has_key, None);
    assert_eq!(result.status, 0, "the .keyfold project did not get its key: {}", result.err);
    assert!(result.err.is_empty(), "known project must not warn: {}", result.err);

    // --project wins over the file, and an unknown project warns but still runs.
    let result = s.cli_in(&sub, &["run", "--project", "alpah", "--", "/bin/sh", "-c", r#"test -z "${EXA_API_KEY+set}""#], None);
    assert_eq!(result.status, 0);
    assert!(result.err.contains("alpah") && result.err.contains("alpha"), "missing warning: {}", result.err);

    // No --project and no .keyfold: usage, nothing runs.
    let outside = s.home.join("outside");
    std::fs::create_dir_all(&outside).unwrap();
    assert_eq!(s.cli_in(&outside, &["run", "--", "/bin/sh", "-c", "exit 0"], None).status, 64);

    // A linked folder (this machine only) works the same way, and unlink undoes it.
    assert_eq!(s.cli_in(&outside, &["link", "--project", "alpha"], None).status, 0);
    assert!(s.registry().contains("\"folders\""));
    let result = s.cli_in(&outside, &has_key, None);
    assert_eq!(result.status, 0, "the linked folder did not get its key: {}", result.err);
    assert!(s.cli_in(&outside, &["folders"], None).out.contains("alpha"));
    assert_eq!(s.cli_in(&outside, &["unlink"], None).status, 0);
    assert_eq!(s.cli_in(&outside, &["run", "--", "/bin/sh", "-c", "exit 0"], None).status, 64);
    assert!(!s.registry().contains("\"folders\""), "an empty folders map must not be written");

    // The agent links through MCP with an absolute path.
    let path = outside.to_string_lossy().into_owned();
    let call = |id: i64, name: &str, args: Value| json!({"jsonrpc":"2.0","id":id,"method":"tools/call","params":{"name":name,"arguments":args}});
    let replies = s.mcp(&[
        call(1, "link_folder", json!({"path": path, "project": "alpha"})),
        call(2, "unlink_folder", json!({"path": path})),
        call(3, "link_folder", json!({"path": "/no/such/folder", "project": "alpha"})),
    ]);
    assert_eq!(replies[0]["result"]["structuredContent"]["project"], "alpha");
    assert_eq!(replies[1]["result"]["structuredContent"]["removed"], json!(true));
    assert_eq!(replies[2]["result"]["isError"], json!(true));
}

#[test]
fn mcp_tools_never_return_the_value() {
    let mut s = Sandbox::new("mcp");
    let value = secret("mcp");
    let first = s.mcp(&[
        json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"test","version":"0"}}}),
        json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
        json!({"jsonrpc":"2.0","id":2,"method":"tools/list"}),
        json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"create_secret","arguments":{"name":"EXA_API_KEY","project":"demo","prompt":false}}}),
    ]);
    assert_eq!(first.len(), 3, "a notification must get no response");
    assert_eq!(first[0]["result"]["protocolVersion"], "2025-06-18");
    let instructions = first[0]["result"]["instructions"].as_str().unwrap();
    let exe = std::fs::canonicalize(env!("CARGO_BIN_EXE_keyfold")).unwrap();
    assert!(
        instructions.contains(&exe.display().to_string()) || instructions.contains(env!("CARGO_BIN_EXE_keyfold")),
        "the agent must learn where the binary is: {instructions}"
    );
    let mut tools: Vec<&str> = first[1]["result"]["tools"].as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap()).collect();
    tools.sort();
    assert_eq!(tools, ["assign_secret", "create_secret", "generate_secret", "link_folder", "list_secrets", "request_secret_value", "unassign_secret", "unlink_folder"]);
    assert_eq!(first[2]["result"]["structuredContent"]["reference"], "keyfold://EXA_API_KEY");

    assert_eq!(s.cli(&["set", "EXA_API_KEY", "--stdin"], Some(&value)).status, 0);

    let call = |id: i64, name: &str, args: Value| json!({"jsonrpc":"2.0","id":id,"method":"tools/call","params":{"name":name,"arguments":args}});
    let second = s.mcp(&[
        call(1, "list_secrets", json!({})),
        call(2, "list_secrets", json!({"project":"demo"})),
        call(3, "assign_secret", json!({"name":"EXA_API_KEY","project":"other"})),
        call(4, "create_secret", json!({"name":"EXA_API_KEY","project":"demo"})),
        call(5, "create_secret", json!({"name":"bad name"})),
        call(6, "request_secret_value", json!({"name":"MISSING"})),
        json!({"jsonrpc":"2.0","id":7,"method":"nope/method"}),
        call(8, "unassign_secret", json!({"name":"EXA_API_KEY","project":"other"})),
    ]);
    assert_eq!(second.len(), 8);
    let listed = &second[0]["result"]["structuredContent"]["secrets"][0];
    assert_eq!(listed["hasValue"], json!(true));
    let mut fields: Vec<&String> = listed.as_object().unwrap().keys().collect();
    fields.sort();
    assert_eq!(fields, ["hasValue", "name", "projects", "reference"]);
    assert_eq!(second[2]["result"]["structuredContent"]["projects"], json!(["demo", "other"]));
    // Existing value: create_secret must not reopen the window, just report it.
    assert_eq!(second[3]["result"]["structuredContent"]["hasValue"], json!(true));
    assert_eq!(second[4]["result"]["isError"], json!(true));
    assert_eq!(second[5]["result"]["isError"], json!(true));
    assert_eq!(second[6]["error"]["code"], json!(-32601));
    assert_eq!(second[7]["result"]["structuredContent"]["projects"], json!(["demo"]));
    assert!(!s.registry().contains("\"other\""), "an empty project must disappear");

    s.assert_never_printed(&value);
}

/// A package encrypted to this computer's `keyfold id` imports into the credential store and
/// reaches `run`; another install cannot open it; no output or file shows the value.
/// The package is made with the core (sharing itself asks for Touch ID, so not from a test).
#[cfg(unix)]
#[test]
fn import_opens_only_packages_for_this_computer() {
    use keyfold_core::registry::RegistryStore;
    use keyfold_core::store::MemoryStore;
    use keyfold_core::Keyfold;

    let mut s = Sandbox::new("import");
    let mut other = Sandbox::new("import-other");
    let value = secret("exa");
    let id = s.cli(&["id"], None);
    assert_eq!(id.status, 0, "{}", id.err);
    let public_key = id.out.trim().to_string();
    assert!(public_key.starts_with("age1"), "{public_key}");
    assert_eq!(s.cli(&["id"], None).out.trim(), public_key, "the identity must persist");

    let sender_home = s.home.join("sender");
    let sender = Keyfold::new(Box::new(MemoryStore::default()), RegistryStore::new(&sender_home));
    sender.create_slot("EXA_API_KEY", Some("demo")).unwrap();
    sender.set_value("EXA_API_KEY", &value).unwrap();
    let package = sender.share(&["EXA_API_KEY".into()], &[public_key]).unwrap();
    let file = s.home.join("exa.age");
    std::fs::write(&file, &package).unwrap();
    assert!(!package.contains(&value));

    let denied = other.cli(&["import", file.to_str().unwrap()], None);
    assert_eq!(denied.status, 1);
    assert!(!denied.err.trim().is_empty(), "the refusal must say why");
    assert!(!other.registry().contains("EXA_API_KEY"), "a package for someone else must not create keys");

    let imported = s.cli(&["import", file.to_str().unwrap()], None);
    assert_eq!(imported.status, 0, "{}", imported.err);
    assert!(imported.out.contains("keyfold://EXA_API_KEY") && imported.out.contains("demo"), "{}", imported.out);
    let args = check_env_equals_stdin("EXA_API_KEY");
    let mut run = vec!["run", "--project", "demo", "--"];
    run.extend(args.iter().map(String::as_str));
    assert_eq!(s.cli(&run, Some(&format!("{value}\n"))).status, 0, "imported value did not reach run");

    s.assert_never_printed(&value);
    other.assert_never_printed(&value);
}

/// Several processes changing the registry at once (app, MCP server, CLI): none of the
/// changes is lost, thanks to the registry lock.
#[test]
fn concurrent_processes_never_lose_a_key() {
    let s = Sandbox::new("race");
    let children: Vec<_> = (0..12)
        .map(|i| {
            Command::new(env!("CARGO_BIN_EXE_keyfold"))
                .args(["create", &format!("RACE_KEY_{i}"), "--project", "race", "--no-prompt"])
                .env("KEYFOLD_HOME", &s.home)
                .env("KEYFOLD_SERVICE", &s.service)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .expect("spawn keyfold")
        })
        .collect();
    for mut child in children {
        assert!(child.wait().unwrap().success());
    }
    let registry: Value = serde_json::from_str(&s.registry()).unwrap();
    assert_eq!(registry["secrets"].as_object().unwrap().len(), 12, "{registry}");
    assert_eq!(registry["projects"]["race"].as_array().unwrap().len(), 12, "{registry}");
}
