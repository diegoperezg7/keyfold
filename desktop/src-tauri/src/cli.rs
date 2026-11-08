use crate::{connect, prompt, unlock};
use keyfold_core::l10n::t;
use keyfold_core::redactor::Redactor;
use keyfold_core::{Config, Keyfold, KeyfoldError, SlotInfo, GENERATE_DEFAULT};
use std::path::PathBuf;
use std::io::{Read, Write};
use std::process::{Command, Stdio};

struct Arguments {
    positionals: Vec<String>,
    project: Option<String>,
    /// `--to age1…`, repeatable (share).
    to: Vec<String>,
    /// `--out FILE` (share).
    out: Option<String>,
    /// `--length N` (generate).
    length: Option<String>,
    flags: Vec<String>,
    command: Vec<String>,
}

impl Arguments {
    fn parse(raw: &[String]) -> Result<Self, KeyfoldError> {
        let mut args = Self { positionals: vec![], project: None, to: vec![], out: None, length: None, flags: vec![], command: vec![] };
        let mut it = raw.iter();
        while let Some(arg) = it.next() {
            match arg.as_str() {
                "--" => {
                    args.command = it.cloned().collect();
                    break;
                }
                "--project" => args.project = Some(it.next().cloned().ok_or(KeyfoldError::InvalidProject(String::new()))?),
                "--to" => args.to.push(it.next().cloned().ok_or(KeyfoldError::InvalidRecipient(String::new()))?),
                "--length" => args.length = Some(it.next().cloned().ok_or(KeyfoldError::InvalidLength(0))?),
                "--out" => args.out = Some(it.next().cloned().ok_or(KeyfoldError::Io("--out needs a file".into()))?),
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
        "generate" => generate(&keyfold, &args),
        "receive" => receive(config, &keyfold, &args),
        "assign" => assign(&keyfold, &args),
        "unassign" => unassign(&keyfold, &args),
        "link" => link(&keyfold, &args),
        "unlink" => unlink(&keyfold, &args),
        "folders" => folders(&keyfold),
        "list" => list(&keyfold, &args),
        "run" => run_command(&keyfold, &args),
        "delete" => delete(&keyfold, &args),
        "id" => id(&keyfold),
        "share" => share(&keyfold, &args),
        "import" => import(&keyfold, &args),
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
  keyfold generate NOMBRE [--project P] [--length N]  genera un secreto aleatorio y lo guarda (no lo muestra; N: 16-256, por defecto 48)
  keyfold receive NOMBRE                              modo recepción: crea el hueco si falta y abre el campo seguro
  keyfold assign NOMBRE --project P                   asigna una clave existente a un proyecto
  keyfold unassign NOMBRE --project P                 quita la clave del proyecto (la clave y su valor se quedan)
  keyfold list [--project P]                          lista nombres y estado, nunca valores
  keyfold run [--project P] -- comando [args...]      ejecuta con las claves del proyecto (salida censurada);
                                                      sin --project usa el .keyfold o la carpeta vinculada más cercana
  keyfold link [CARPETA] --project P                  vincula la carpeta (por defecto la actual) a un proyecto, solo en este equipo
  keyfold unlink [CARPETA]                            quita el vínculo de la carpeta
  keyfold folders                                     lista las carpetas vinculadas
  keyfold delete NOMBRE                               borra la clave y su valor
  keyfold id                                          muestra la clave pública de este equipo (age1…), para que te compartan claves
  keyfold share NOMBRE... | --all --to age1... [--to ...] [--out ARCHIVO]
                                                      cifra las claves solo para esos destinatarios (pide Touch ID)
  keyfold import [ARCHIVO]                            importa un paquete cifrado para este equipo (o desde stdin)
  keyfold mcp                                         servidor MCP por stdio para agentes
  keyfold connect claude|codex|cursor                 registra este binario como servidor MCP del agente",
            "Usage:
  keyfold                                             open the app
  keyfold create NAME [--project P] [--no-prompt]     create the slot and open the window to paste the value
  keyfold set NAME [--stdin]                          store or replace the value (window, or stdin)
  keyfold generate NAME [--project P] [--length N]    generate a random secret and store it (never shown; N: 16-256, default 48)
  keyfold receive NAME                                receive mode: create the slot if missing and open the secure field
  keyfold assign NAME --project P                     assign an existing key to a project
  keyfold unassign NAME --project P                   remove the key from the project (the key and its value stay)
  keyfold list [--project P]                          list names and status, never values
  keyfold run [--project P] -- command [args...]      run with the project's keys (output redacted);
                                                      without --project, uses the nearest .keyfold or linked folder
  keyfold link [FOLDER] --project P                   link the folder (default: current) to a project, on this machine only
  keyfold unlink [FOLDER]                             remove the folder's link
  keyfold folders                                     list linked folders
  keyfold delete NAME                                 delete the key and its value
  keyfold id                                          show this computer's public key (age1…), so others can share keys with you
  keyfold share NAME... | --all --to age1... [--to ...] [--out FILE]
                                                      encrypt the keys for those recipients only (asks for Touch ID)
  keyfold import [FILE]                               import a package encrypted for this computer (or from stdin)
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

/// `keyfold id`: this computer's public key. Safe to send to anyone.
fn id(l: &Keyfold) -> Result<i32, KeyfoldError> {
    println!("{}", l.public_key()?);
    Ok(0)
}

/// `keyfold share`: an age package for the given public keys. It reads values, so it asks for
/// Touch ID every time: an agent with a shell must not be able to send keys to itself.
fn share(l: &Keyfold, args: &Arguments) -> Result<i32, KeyfoldError> {
    let all = args.has("--all");
    if args.to.is_empty() || (all == !args.positionals.is_empty()) {
        return Ok(usage());
    }
    if !unlock::AVAILABLE {
        eprintln!("{}", unlock::unavailable_message());
        return Ok(1);
    }
    let what = if all { t("todas tus claves", "all your keys") } else { args.positionals.join(", ") };
    let reason = t(&format!("compartir {what} cifradas"), &format!("share {what}, encrypted"));
    if !unlock::authenticate(&reason) {
        eprintln!("{}", t("Cancelado. No se ha compartido nada.", "Cancelled. Nothing was shared."));
        return Ok(1);
    }
    let package = if all { l.share_all(&args.to)? } else { l.share(&args.positionals, &args.to)? };
    match &args.out {
        Some(file) => {
            std::fs::write(file, &package)?;
            eprintln!("{}", t(&format!("Paquete cifrado en {file}"), &format!("Encrypted package in {file}")));
        }
        None => print!("{package}"),
    }
    Ok(0)
}

/// `keyfold import [FILE]`: opens a package made for this computer (stdin without FILE).
fn import(l: &Keyfold, args: &Arguments) -> Result<i32, KeyfoldError> {
    let package = match first(args) {
        Some(file) => std::fs::read_to_string(file)?,
        None => {
            let mut input = String::new();
            std::io::stdin().read_to_string(&mut input)?;
            input
        }
    };
    for info in l.import(&package)? {
        let projects = if info.projects.is_empty() { "-".to_string() } else { info.projects.join(", ") };
        println!("{}", t(&format!("Importada: {}  ({projects})", info.reference), &format!("Imported: {}  ({projects})", info.reference)));
    }
    Ok(0)
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

/// `keyfold generate NAME`: a random secret made and stored here. Prints the reference only.
fn generate(l: &Keyfold, args: &Arguments) -> Result<i32, KeyfoldError> {
    let Some(name) = first(args) else { return Ok(usage()) };
    let length = match &args.length {
        Some(n) => n.parse().map_err(|_| KeyfoldError::InvalidLength(0))?,
        None => GENERATE_DEFAULT,
    };
    let info = l.generate(name, args.project.as_deref(), length)?;
    println!("{}", t(&format!("Secreto generado y guardado: {}", info.reference), &format!("Secret generated and stored: {}", info.reference)));
    if let Some(p) = &args.project {
        println!("{}", t(&format!("Proyecto: {p}"), &format!("Project: {p}")));
    }
    Ok(0)
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

fn unassign(l: &Keyfold, args: &Arguments) -> Result<i32, KeyfoldError> {
    let (Some(name), Some(project)) = (first(args), args.project.as_deref()) else { return Ok(usage()) };
    l.unassign(name, project)?;
    println!("{}", t(&format!("keyfold://{name} quitada de {project}"), &format!("keyfold://{name} removed from {project}")));
    Ok(0)
}

/// The folder argument, or the current folder.
fn folder_arg(args: &Arguments) -> Result<PathBuf, KeyfoldError> {
    Ok(match first(args) {
        Some(path) => PathBuf::from(path),
        None => std::env::current_dir()?,
    })
}

fn link(l: &Keyfold, args: &Arguments) -> Result<i32, KeyfoldError> {
    let Some(project) = args.project.as_deref() else { return Ok(usage()) };
    let path = l.link_folder(&folder_arg(args)?, project)?;
    println!("{}", t(&format!("{path} vinculada a {project}"), &format!("{path} linked to {project}")));
    Ok(0)
}

fn unlink(l: &Keyfold, args: &Arguments) -> Result<i32, KeyfoldError> {
    let folder = folder_arg(args)?;
    if l.unlink_folder(&folder)? {
        println!("{}", t("Vínculo quitado.", "Link removed."));
        Ok(0)
    } else {
        eprintln!("{}", t(&format!("{} no está vinculada.", folder.display()), &format!("{} is not linked.", folder.display())));
        Ok(1)
    }
}

fn folders(l: &Keyfold) -> Result<i32, KeyfoldError> {
    let folders = l.folders()?;
    if folders.is_empty() {
        println!("{}", t("Sin carpetas vinculadas.", "No linked folders."));
    }
    for (path, project) in &folders {
        println!("{path}  {project}");
    }
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
    // --project wins; otherwise the nearest .keyfold or linked folder from here up.
    let project = match &args.project {
        Some(p) => p.clone(),
        None => match l.project_for(&std::env::current_dir()?)? {
            Some(p) => p,
            None => return Ok(usage()),
        },
    };
    let project = project.as_str();
    let Some((program, rest)) = args.command.split_first() else { return Ok(usage()) };
    let known = l.projects()?;
    if !known.contains(&project.to_string()) {
        let list = if known.is_empty() { "-".to_string() } else { known.join(", ") };
        eprintln!(
            "{}",
            t(
                &format!("Aviso: el proyecto {project} no existe en Keyfold; se ejecuta sin claves. Proyectos: {list}"),
                &format!("Warning: project {project} does not exist in Keyfold; running without keys. Projects: {list}"),
            )
        );
    }
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
