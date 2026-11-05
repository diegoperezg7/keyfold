import Foundation
import LlaveroCore

// Minimal MCP server over stdio (newline-delimited JSON-RPC 2.0).
// Same contract as the CLI: no tool ever returns a secret value.

private let supportedProtocolVersions = ["2025-11-25", "2025-06-18", "2025-03-26", "2024-11-05"]

private let usageNote = """
Never ask the user to paste a key into the chat or into a file. \
To run the project with its keys, use the shell: `llavero run --project <project> -- <command>`. \
Do not write values into .env files; the keys are injected at run time.
"""

private let tools: [[String: Any]] = [
    [
        "name": "list_secrets",
        "description": "List stored API key names, whether each has a value, and the projects they are assigned to. Never returns values.",
        "inputSchema": [
            "type": "object",
            "properties": [
                "project": ["type": "string", "description": "Only keys assigned to this project."],
            ],
        ],
        "annotations": ["readOnlyHint": true],
    ],
    [
        "name": "create_secret",
        "description": "Create a slot for an API key (e.g. EXA_API_KEY), optionally assign it to a project, and open a native macOS window where the user pastes the value. Blocks until the user saves or cancels. Returns only the reference llavero://NAME. " + usageNote,
        "inputSchema": [
            "type": "object",
            "properties": [
                "name": ["type": "string", "description": "UPPER_SNAKE_CASE env var name, e.g. EXA_API_KEY."],
                "project": ["type": "string", "description": "Project name (letters, digits, . _ -)."],
                "prompt": ["type": "boolean", "description": "Open the paste window if the slot has no value. Default true."],
            ],
            "required": ["name"],
        ],
    ],
    [
        "name": "request_secret_value",
        "description": "Open the native window so the user pastes or replaces the value of an existing key. Blocks until saved or cancelled. Never returns the value.",
        "inputSchema": [
            "type": "object",
            "properties": ["name": ["type": "string"]],
            "required": ["name"],
        ],
    ],
    [
        "name": "assign_secret",
        "description": "Assign an existing key to a project so `llavero run --project <project>` injects it. " + usageNote,
        "inputSchema": [
            "type": "object",
            "properties": ["name": ["type": "string"], "project": ["type": "string"]],
            "required": ["name", "project"],
        ],
    ],
]

private struct ToolError: Error {
    let message: String
}

private func slotJSON(_ info: SlotInfo) -> [String: Any] {
    ["name": info.name, "reference": info.reference, "hasValue": info.hasValue, "projects": info.projects]
}

private func slot(_ name: String) throws -> SlotInfo {
    guard let info = try llavero.list().first(where: { $0.name == name }) else {
        throw LlaveroError.unknownSecret(name)
    }
    return info
}

private func string(_ arguments: [String: Any], _ key: String) throws -> String {
    guard let value = arguments[key] as? String, !value.isEmpty else {
        throw ToolError(message: "Missing argument: \(key)")
    }
    return value
}

/// Returns structured content for a tool call. Only names, references, status and projects.
private func callTool(_ name: String, _ arguments: [String: Any]) throws -> [String: Any] {
    switch name {
    case "list_secrets":
        let project = arguments["project"] as? String
        return ["secrets": try llavero.list(project: project).map(slotJSON)]

    case "create_secret":
        let secretName = try string(arguments, "name")
        let project = arguments["project"] as? String
        var info = try llavero.createSlot(secretName, project: project)
        if !info.hasValue && (arguments["prompt"] as? Bool ?? true) {
            _ = promptAndSave(secretName, project: project)
            info = try slot(secretName)
        }
        return slotJSON(info)

    case "request_secret_value":
        let secretName = try string(arguments, "name")
        _ = try slot(secretName)
        let saved = promptAndSave(secretName, project: nil)
        var result = slotJSON(try slot(secretName))
        result["saved"] = saved
        return result

    case "assign_secret":
        let secretName = try string(arguments, "name")
        try llavero.assign(secretName, to: try string(arguments, "project"))
        return slotJSON(try slot(secretName))

    default:
        throw ToolError(message: "Unknown tool: \(name)")
    }
}

private func toolResult(_ structured: [String: Any], isError: Bool = false) -> [String: Any] {
    let data = (try? JSONSerialization.data(withJSONObject: structured, options: [.sortedKeys])) ?? Data()
    return [
        "content": [["type": "text", "text": String(decoding: data, as: UTF8.self)]],
        "structuredContent": structured,
        "isError": isError,
    ]
}

private func handle(_ method: String, _ params: [String: Any]) throws -> [String: Any] {
    switch method {
    case "initialize":
        let requested = params["protocolVersion"] as? String ?? ""
        let version = supportedProtocolVersions.contains(requested) ? requested : supportedProtocolVersions[1]
        return [
            "protocolVersion": version,
            "capabilities": ["tools": [:] as [String: Any]],
            "serverInfo": ["name": "llavero", "version": "0.1.0"],
            "instructions": "Local API key vault. Keys are referenced by name; values stay in the macOS Keychain and are never returned. " + usageNote,
        ]
    case "ping":
        return [:]
    case "tools/list":
        return ["tools": tools]
    case "tools/call":
        let name = params["name"] as? String ?? ""
        let arguments = params["arguments"] as? [String: Any] ?? [:]
        do {
            return toolResult(try callTool(name, arguments))
        } catch let error as ToolError {
            return toolResult(["error": error.message], isError: true)
        } catch {
            return toolResult(["error": describe(error)], isError: true)
        }
    default:
        throw RPCError(code: -32601, message: "Method not found: \(method)")
    }
}

private struct RPCError: Error {
    let code: Int
    let message: String
}

private func send(_ message: [String: Any]) {
    guard var data = try? JSONSerialization.data(withJSONObject: message, options: [.sortedKeys]) else { return }
    data.append(UInt8(ascii: "\n"))
    FileHandle.standardOutput.write(data)
}

/// Reads requests from stdin until EOF. Runs on the main thread so the paste window can open.
func serveMCP() -> Int32 {
    while let line = readLine(strippingNewline: true) {
        guard !line.isEmpty else { continue }
        guard let object = try? JSONSerialization.jsonObject(with: Data(line.utf8)),
              let request = object as? [String: Any],
              let method = request["method"] as? String
        else {
            send(["jsonrpc": "2.0", "id": NSNull(), "error": ["code": -32700, "message": "Parse error"]])
            continue
        }
        // Notifications (no id) get no response.
        guard let id = request["id"] else { continue }
        let params = request["params"] as? [String: Any] ?? [:]
        do {
            send(["jsonrpc": "2.0", "id": id, "result": try handle(method, params)])
        } catch let error as RPCError {
            send(["jsonrpc": "2.0", "id": id, "error": ["code": error.code, "message": error.message]])
        } catch {
            send(["jsonrpc": "2.0", "id": id, "error": ["code": -32603, "message": describe(error)]])
        }
    }
    return 0
}
