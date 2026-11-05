//! One binary, three faces:
//!   keyfold            / keyfold app  -> desktop window with the keys
//!   keyfold mcp                       -> MCP server over stdio for coding agents
//!   keyfold <command>                 -> CLI (create, set, receive, assign, list, run, delete, connect)
//! Output contract everywhere: names, references, status and projects. Never a value.

mod cli;
mod connect;
mod gui;
mod mcp;
mod prompt;
mod tray;
mod unlock;

use keyfold_core::Config;

fn main() {
    // Finder may pass a process serial number argument on older macOS.
    let args: Vec<String> = std::env::args().skip(1).filter(|a| !a.starts_with("-psn_")).collect();
    let config = Config::from_env();

    let code = match args.first().map(String::as_str) {
        None | Some("app") => gui::run(config, gui::Mode::Keys),
        Some(prompt::CHILD_COMMAND) => match prompt::ChildArgs::parse(&args[1..]) {
            Some(child) => gui::run(child.config, gui::Mode::Prompt(child.request)),
            None => 64,
        },
        Some("mcp") => mcp::serve(&config),
        Some(_) => cli::run(&config, &args),
    };
    std::process::exit(code);
}
