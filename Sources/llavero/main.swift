import Foundation
import LlaveroCore

// Output contract: nothing printed here ever includes a secret value.
// Only names, references (llavero://NAME), status and project names.

let t = L10n.t
let environment = ProcessInfo.processInfo.environment
let llavero = Llavero(
    store: KeychainStore(service: environment["LLAVERO_SERVICE"] ?? KeychainStore.defaultService),
    registryStore: RegistryStore(directory: RegistryStore.defaultDirectory())
)

func printErr(_ message: String) {
    FileHandle.standardError.write(Data((message + "\n").utf8))
}

func usage() -> Int32 {
    printErr(t(
        """
        Uso:
          llavero create NOMBRE [--project P] [--no-prompt]   crea el hueco y abre la ventana para pegar el valor
          llavero set NOMBRE [--stdin]                        guarda o cambia el valor (ventana, o stdin)
          llavero receive NOMBRE                              modo recepción: crea el hueco si falta y abre el campo seguro
          llavero assign NOMBRE --project P                   asigna una clave existente a un proyecto
          llavero list [--project P]                          lista nombres y estado, nunca valores
          llavero run --project P -- comando [args...]        ejecuta con las claves del proyecto (salida censurada)
          llavero delete NOMBRE                               borra la clave y su valor
          llavero mcp                                         servidor MCP por stdio para agentes
          llavero app                                         ventana con las claves (nunca muestra valores)
        """,
        """
        Usage:
          llavero create NAME [--project P] [--no-prompt]     create the slot and open the window to paste the value
          llavero set NAME [--stdin]                          store or replace the value (window, or stdin)
          llavero receive NAME                                receive mode: create the slot if missing and open the secure field
          llavero assign NAME --project P                     assign an existing key to a project
          llavero list [--project P]                          list names and status, never values
          llavero run --project P -- command [args...]        run with the project's keys (output redacted)
          llavero delete NAME                                 delete the key and its value
          llavero mcp                                         MCP server over stdio for agents
          llavero app                                         window listing keys (never shows values)
        """
    ))
    return 64
}

struct Arguments {
    var positionals: [String] = []
    var project: String?
    var flags: Set<String> = []
    var command: [String] = []

    init(_ raw: ArraySlice<String>) throws {
        var iterator = raw.makeIterator()
        while let arg = iterator.next() {
            switch arg {
            case "--":
                command = Array(iterator)
                return
            case "--project":
                guard let value = iterator.next() else { throw LlaveroError.invalidProject("") }
                project = value
            case let flag where flag.hasPrefix("--"):
                flags.insert(flag)
            default:
                positionals.append(arg)
            }
        }
    }
}

func status(_ info: SlotInfo) -> String {
    info.hasValue ? t("guardado", "stored") : t("pendiente", "pending")
}

func promptAndSave(_ name: String, project: String?) -> Bool {
    MainActor.assumeIsolated {
        presentSecretPrompt(name: name, project: project) { value in
            try llavero.setValue(value, for: name)
        }
    }
}

func create(_ args: Arguments) throws -> Int32 {
    guard let name = args.positionals.first else { return usage() }
    var info = try llavero.createSlot(name, project: args.project)
    print(t("Hueco creado: \(info.reference)", "Slot created: \(info.reference)"))
    if let project = args.project { print(t("Proyecto: \(project)", "Project: \(project)")) }

    if !info.hasValue && !args.flags.contains("--no-prompt") {
        _ = promptAndSave(name, project: args.project)
        info = try llavero.list().first { $0.name == name } ?? info
    }
    print(t("Valor: \(status(info))", "Value: \(status(info))"))
    if !info.hasValue {
        print(t("Para añadirlo: llavero set \(name)", "To add it: llavero set \(name)"))
    }
    return 0
}

func set(_ args: Arguments) throws -> Int32 {
    guard let name = args.positionals.first else { return usage() }
    if args.flags.contains("--stdin") {
        let input = FileHandle.standardInput.readDataToEndOfFile()
        let value = String(decoding: input, as: UTF8.self).trimmingCharacters(in: .whitespacesAndNewlines)
        try llavero.setValue(value, for: name)
    } else {
        guard try llavero.list().contains(where: { $0.name == name }) else {
            throw LlaveroError.unknownSecret(name)
        }
        guard promptAndSave(name, project: nil) else {
            print(t("Cancelado. Valor sin cambios.", "Cancelled. Value unchanged."))
            return 1
        }
    }
    print(t("Valor guardado para llavero://\(name)", "Value stored for llavero://\(name)"))
    return 0
}

/// Receive mode: ensures the slot exists (no project change) and always opens the
/// secure field, so a key can be pasted in one step.
func receive(_ args: Arguments) throws -> Int32 {
    guard let name = args.positionals.first else { return usage() }
    let existed = try llavero.list().contains { $0.name == name }
    let info = try llavero.createSlot(name, project: nil)
    if !existed { print(t("Hueco creado: \(info.reference)", "Slot created: \(info.reference)")) }
    guard promptAndSave(name, project: nil) else {
        print(t("Cancelado. Valor sin cambios.", "Cancelled. Value unchanged."))
        return 1
    }
    let stored = try llavero.list().first { $0.name == name } ?? info
    print(t("Recibida: \(stored.reference)", "Received: \(stored.reference)"))
    print(t("Valor: \(status(stored))", "Value: \(status(stored))"))
    return 0
}

func assign(_ args: Arguments) throws -> Int32 {
    guard let name = args.positionals.first, let project = args.project else { return usage() }
    try llavero.assign(name, to: project)
    print(t("llavero://\(name) asignada a \(project)", "llavero://\(name) assigned to \(project)"))
    return 0
}

func list(_ args: Arguments) throws -> Int32 {
    let slots = try llavero.list(project: args.project)
    if slots.isEmpty {
        print(t("Sin claves.", "No keys."))
        return 0
    }
    let width = slots.map(\.name.count).max() ?? 0
    for slot in slots {
        let padded = slot.name.padding(toLength: width, withPad: " ", startingAt: 0)
        let projects = slot.projects.isEmpty ? "-" : slot.projects.joined(separator: ", ")
        print("\(padded)  \(status(slot).padding(toLength: 9, withPad: " ", startingAt: 0))  \(projects)")
    }
    return 0
}

func delete(_ args: Arguments) throws -> Int32 {
    guard let name = args.positionals.first else { return usage() }
    try llavero.delete(name)
    print(t("Borrada: llavero://\(name)", "Deleted: llavero://\(name)"))
    return 0
}

/// Runs the command with only the project's keys in its environment, and redacts
/// any of those values from its stdout/stderr before passing them on.
func run(_ args: Arguments) throws -> Int32 {
    guard let project = args.project, !args.command.isEmpty else { return usage() }
    let secrets = try llavero.values(for: project)
    for slot in try llavero.list(project: project) where !slot.hasValue {
        printErr(t("Aviso: \(slot.name) no tiene valor.", "Warning: \(slot.name) has no value."))
    }

    var childEnvironment = environment
    for key in ["LLAVERO_HOME", "LLAVERO_SERVICE"] { childEnvironment[key] = nil }
    for secret in secrets { childEnvironment[secret.name] = secret.value }

    let process = Process()
    process.executableURL = URL(fileURLWithPath: "/usr/bin/env")
    process.arguments = args.command
    process.environment = childEnvironment
    let stdoutPipe = Pipe()
    let stderrPipe = Pipe()
    process.standardOutput = stdoutPipe
    process.standardError = stderrPipe
    try process.run()

    let group = DispatchGroup()
    for (pipe, sink) in [(stdoutPipe, FileHandle.standardOutput), (stderrPipe, FileHandle.standardError)] {
        group.enter()
        Thread.detachNewThread {
            var redactor = Redactor(secrets: secrets)
            let handle = pipe.fileHandleForReading
            while true {
                let chunk = handle.availableData
                if chunk.isEmpty { break }
                sink.write(redactor.feed(chunk))
            }
            sink.write(redactor.finish())
            group.leave()
        }
    }
    process.waitUntilExit()
    group.wait()
    return process.terminationStatus
}

func describe(_ error: Error) -> String {
    switch error {
    case LlaveroError.invalidName(let name):
        return t("Nombre no válido: \(name). Usa MAYUSCULAS_CON_GUIONES.", "Invalid name: \(name). Use UPPER_SNAKE_CASE.")
    case LlaveroError.invalidProject(let project):
        return t("Proyecto no válido: '\(project)'.", "Invalid project: '\(project)'.")
    case LlaveroError.unknownSecret(let name):
        return t("No existe la clave \(name). Créala con: llavero create \(name)", "No key named \(name). Create it with: llavero create \(name)")
    case LlaveroError.emptyValue:
        return t("El valor está vacío.", "The value is empty.")
    case let error as KeychainError:
        return error.description
    default:
        // Registry and process errors only; neither can carry a secret value.
        return error.localizedDescription
    }
}

let argv = CommandLine.arguments
let commands: [String: (Arguments) throws -> Int32] = [
    "create": create, "set": set, "receive": receive, "assign": assign, "list": list, "delete": delete, "run": run,
    "mcp": { _ in serveMCP() },
    "app": { _ in MainActor.assumeIsolated { runKeysApp() } },
]

guard argv.count >= 2, let handler = commands[argv[1]] else { exit(usage()) }
do {
    exit(try handler(Arguments(argv.dropFirst(2))))
} catch {
    printErr(describe(error))
    exit(1)
}
