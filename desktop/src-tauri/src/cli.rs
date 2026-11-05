use crate::{connect, prompt};
use keyfold_core::l10n::t;
use keyfold_core::redactor::Redactor;
use keyfold_core::{Config, Keyfold, KeyfoldError, SlotInfo};
use std::io::{Read, Write};
use std::process::{Command, Stdio};

struct Arguments {
    positionals: Vec<String>,
    project: Option<String>,
    flags: Vec<String>,
    command: Vec<String>,
}

impl Arguments {
    fn parse(raw: &[String]) -> Result<Self, KeyfoldError> {
        let mut args = Self { positionals: vec![], project: None, flags: vec![], command: vec![] };
        let mut it = raw.iter();
        while let Some(arg) = it.next() {
            match arg.as_str() {
                "--" => {
                    args.command = it.cloned().collect();
                    break;
                }
                "--project" => args.project = Some(it.next().cloned().ok_or(KeyfoldError::InvalidProject(String::new()))?),
                flag if flag.starts_with("--") => args.flags.push(flag.into()),
                other => args.positionals.push(other.into()),
            }
        }
        Ok(args)
    }

    fn has(&self, flag: &str) -> bool {
        self.flags.iter().any(|f| f == flag)
    }
}

pub fn run(config: &Config, argv: &[String]) -> i32 {
    let keyfold = Keyfold::open(config);
    let command = argv[0].as_str();
    let args = match Arguments::parse(&argv[1..]) {
        Ok(a) => a,
        Err(e) => return fail(&e),
    };
    let result = match command {
        "create" => create(config, &keyfold, &args),
        "set" => set(config, &keyfold, &args),
        "receive" => receive(config, &keyfold, &args),
        "assign" => assign(&keyfold, &args),
        "list" => list(&keyfold, &args),
        "run" => run_command(&keyfold, &args),
        "delete" => delete(&keyfold, &args),
        "connect" => return connect_agent(&args),
        _ => Ok(usage()),
    };
    result.unwrap_or_else(|e| fail(&e))
}

fn fail(e: &KeyfoldError) -> i32 {
    eprintln!("{e}");
    1
}

fn usage() -> i32 {
    eprintln!(
        "{}",
        t(
            "Uso:
  keyfold                                             abre la app
  keyfold create NOMBRE [--project P] [--no-prompt]   crea el hueco y abre la ventana para pegar el valor
  keyfold set NOMBRE [--stdin]                        guarda o cambia el valor (ventana, o stdin)
  keyfold receive NOMBRE                              modo recepción: crea el hueco si falta y abre el campo seguro
  keyfold assign NOMBRE --project P                   asigna una clave existente a un proyecto
  keyfold list [--project P]                          lista nombres y estado, nunca valores
  keyfold run --project P -- comando [args...]        ejecuta con las claves del proyecto (salida censurada)
  keyfold delete NOMBRE                               borra la clave y su valor
  keyfold mcp                                         servidor MCP por stdio para agentes
  keyfold connect claude|codex|cursor                 registra este binario como servidor MCP del agente",
            "Usage:
  keyfold                                             open the app
  keyfold create NAME [--project P] [--no-prompt]     create the slot and open the window to paste the value
  keyfold set NAME [--stdin]                          store or replace the value (window, or stdin)
  keyfold receive NAME                                receive mode: create the slot if missing and open the secure field
  keyfold assign NAME --project P                     assign an existing key to a project
  keyfold list [--project P]                          list names and status, never values
  keyfold run --project P -- command [args...]        run with the project's keys (output redacted)
  keyfold delete NAME                                 delete the key and its value
  keyfold mcp                                         MCP server over stdio for agents
  keyfold connect claude|codex|cursor                 register this binary as the agent's MCP server",
        )
    );
    64
}

/// `keyfold connect claude|codex|cursor`: register this binary as the agent's MCP server.
fn connect_agent(args: &Arguments) -> i32 {
    let Some(agent) = first(args).and_then(connect::Agent::parse) else { return usage() };
    match connect::connect(agent) {
        Ok(outcome) => {
            println!("{}", outcome.message());
            0
        }
        Err(message) => {
            eprintln!("{message}");
            1
        }
    }
}

fn status(info: &SlotInfo) -> String {
    if info.has_value {
        t("guardado", "stored")
    } else {
        t("pendiente", "pending")
    }
}

fn first(args: &Arguments) -> Option<&str> {
    args.positionals.first().map(String::as_str)
}

fn create(config: &Config, l: &Keyfold, args: &Arguments) -> Result<i32, KeyfoldError> {
    let Some(name) = first(args) else { return Ok(usage()) };
    let mut info = l.create_slot(name, args.project.as_deref())?;
    println!("{}", t(&format!("Hueco creado: {}", info.reference), &format!("Slot created: {}", info.reference)));
    if let Some(p) = &args.project {
        println!("{}", t(&format!("Proyecto: {p}"), &format!("Project: {p}")));
    }
    if !info.has_value && !args.has("--no-prompt") {
        prompt::request(config, name, args.project.as_deref());
        info = l.slot(name)?;
    }
    println!("{}", t(&format!("Valor: {}", status(&info)), &format!("Value: {}", status(&info))));
    if !info.has_value {
        println!("{}", t(&format!("Para añadirlo: keyfold set {name}"), &format!("To add it: keyfold set {name}")));
    }
    Ok(0)
}

fn set(config: &Config, l: &Keyfold, args: &Arguments) -> Result<i32, KeyfoldError> {
    let Some(name) = first(args) else { return Ok(usage()) };
    if args.has("--stdin") {
        let mut input = String::new();
        std::io::stdin().read_to_string(&mut input)?;
        l.set_value(name, input.trim())?;
    } else {
        l.slot(name)?;
        if !prompt::request(config, name, None) {
            println!("{}", t("Cancelado. Valor sin cambios.", "Cancelled. Value unchanged."));
            return Ok(1);
        }
    }
    println!("{}", t(&format!("Valor guardado para keyfold://{name}"), &format!("Value stored for keyfold://{name}")));
    Ok(0)
}

fn receive(config: &Config, l: &Keyfold, args: &Arguments) -> Result<i32, KeyfoldError> {
    let Some(name) = first(args) else { return Ok(usage()) };
    let existed = l.slot(name).is_ok();
    let info = l.create_slot(name, None)?;
    if !existed {
        println!("{}", t(&format!("Hueco creado: {}", info.reference), &format!("Slot created: {}", info.reference)));
    }
    if !prompt::request(config, name, None) {
        println!("{}", t("Cancelado. Valor sin cambios.", "Cancelled. Value unchanged."));
        return Ok(1);
    }
    let stored = l.slot(name)?;
    println!("{}", t(&format!("Recibida: {}", stored.reference), &format!("Received: {}", stored.reference)));
    println!("{}", t(&format!("Valor: {}", status(&stored)), &format!("Value: {}", status(&stored))));
    Ok(0)
}

fn assign(l: &Keyfold, args: &Arguments) -> Result<i32, KeyfoldError> {
    let (Some(name), Some(project)) = (first(args), args.project.as_deref()) else { return Ok(usage()) };
    l.assign(name, project)?;
    println!("{}", t(&format!("keyfold://{name} asignada a {project}"), &format!("keyfold://{name} assigned to {project}")));
    Ok(0)
}

fn list(l: &Keyfold, args: &Arguments) -> Result<i32, KeyfoldError> {
    let slots = l.list(args.project.as_deref())?;
    if slots.is_empty() {
        println!("{}", t("Sin claves.", "No keys."));
        return Ok(0);
    }
    let width = slots.iter().map(|s| s.name.len()).max().unwrap_or(0);
    for slot in &slots {
        let projects = if slot.projects.is_empty() { "-".to_string() } else { slot.projects.join(", ") };
        println!("{:<width$}  {:<9}  {projects}", slot.name, status(slot));
    }
    Ok(0)
}

fn delete(l: &Keyfold, args: &Arguments) -> Result<i32, KeyfoldError> {
    let Some(name) = first(args) else { return Ok(usage()) };
    l.delete(name)?;
    println!("{}", t(&format!("Borrada: keyfold://{name}"), &format!("Deleted: keyfold://{name}")));
    Ok(0)
}

/// Runs the command with only the project's keys in its environment and redacts
/// their values from its stdout/stderr before passing them on.
fn run_command(l: &Keyfold, args: &Arguments) -> Result<i32, KeyfoldError> {
    let Some(project) = args.project.as_deref() else { return Ok(usage()) };
    let Some((program, rest)) = args.command.split_first() else { return Ok(usage()) };
    let secrets = l.values_for(project)?;
    for slot in l.list(Some(project))?.iter().filter(|s| !s.has_value) {
        eprintln!("{}", t(&format!("Aviso: {} no tiene valor.", slot.name), &format!("Warning: {} has no value.", slot.name)));
    }

    let mut child = Command::new(program);
    child.args(rest).env_remove("KEYFOLD_HOME").env_remove("KEYFOLD_SERVICE");
    for (name, value) in &secrets {
        child.env(name, value);
    }
    let mut child = child.stdin(Stdio::inherit()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn()?;

    let pump = |mut source: Box<dyn Read + Send>, to_stdout: bool, secrets: Vec<(String, String)>| {
        std::thread::spawn(move || {
            let mut redactor = Redactor::new(&secrets);
            let mut chunk = [0u8; 8192];
            let write = |bytes: &[u8]| {
                if to_stdout {
                    let mut out = std::io::stdout().lock();
                    let _ = out.write_all(bytes).and_then(|_| out.flush());
                } else {
                    let _ = std::io::stderr().lock().write_all(bytes);
                }
            };
            while let Ok(n) = source.read(&mut chunk) {
                if n == 0 {
                    break;
                }
                write(&redactor.feed(&chunk[..n]));
            }
            write(&redactor.finish());
        })
    };
    let out = pump(Box::new(child.stdout.take().expect("piped")), true, secrets.clone());
    let err = pump(Box::new(child.stderr.take().expect("piped")), false, secrets);
    let status = child.wait()?;
    let _ = out.join();
    let _ = err.join();
    Ok(status.code().unwrap_or(1))
}
