import Foundation
import XCTest

/// Talks to `llavero mcp` over stdio like an agent would.
final class MCPServerTests: XCTestCase {
    private var home: URL!
    private var service: String!
    private let secret = "exa-mcp-test-" + UUID().uuidString

    override func setUpWithError() throws {
        home = FileManager.default.temporaryDirectory.appendingPathComponent("llavero-mcp-\(UUID().uuidString)")
        service = "com.diego.llavero.tests.\(UUID().uuidString)"
    }

    override func tearDownWithError() throws {
        _ = try? run(["delete", "EXA_API_KEY"])
        try? FileManager.default.removeItem(at: home)
    }

    func testToolsNeverReturnStoredValue() throws {
        // Slot created through MCP (no window in tests), value set out of band as the user would.
        let first = try mcp([
            request(1, "initialize", ["protocolVersion": "2025-06-18", "capabilities": [:], "clientInfo": ["name": "test", "version": "0"]]),
            ["jsonrpc": "2.0", "method": "notifications/initialized"],
            request(2, "tools/list"),
            call(3, "create_secret", ["name": "EXA_API_KEY", "project": "demo", "prompt": false]),
        ])
        XCTAssertEqual(first.responses.count, 3, "notification must get no response")
        XCTAssertEqual((first.responses[0]["result"] as? [String: Any])?["protocolVersion"] as? String, "2025-06-18")

        let toolNames = ((first.responses[1]["result"] as? [String: Any])?["tools"] as? [[String: Any]])?.compactMap { $0["name"] as? String }
        XCTAssertEqual(Set(toolNames ?? []), ["list_secrets", "create_secret", "request_secret_value", "assign_secret"])

        let created = try structured(first.responses[2])
        XCTAssertEqual(created["reference"] as? String, "llavero://EXA_API_KEY")
        XCTAssertEqual(created["hasValue"] as? Bool, false)

        XCTAssertEqual(try run(["set", "EXA_API_KEY", "--stdin"], stdin: secret).status, 0)

        let second = try mcp([
            request(1, "initialize", ["protocolVersion": "2025-06-18"]),
            call(2, "list_secrets", [:]),
            call(3, "list_secrets", ["project": "demo"]),
            call(4, "assign_secret", ["name": "EXA_API_KEY", "project": "other"]),
            call(5, "create_secret", ["name": "EXA_API_KEY", "project": "demo"]),
            call(6, "create_secret", ["name": "bad name"]),
            call(7, "request_secret_value", ["name": "MISSING"]),
            request(8, "nope/method"),
        ])
        XCTAssertEqual(second.responses.count, 8)
        XCTAssertFalse(second.raw.contains(secret), "value leaked over MCP: \(second.raw)")
        XCTAssertFalse(first.raw.contains(secret))

        let listed = try structured(second.responses[1])["secrets"] as? [[String: Any]]
        XCTAssertEqual(listed?.first?["name"] as? String, "EXA_API_KEY")
        XCTAssertEqual(listed?.first?["hasValue"] as? Bool, true)
        XCTAssertEqual(Set(listed?.first?.keys.map { $0 } ?? []), ["name", "reference", "hasValue", "projects"])

        let assigned = try structured(second.responses[3])
        XCTAssertEqual(assigned["projects"] as? [String], ["demo", "other"])

        // Existing value: create_secret must not reopen the window, just report it.
        XCTAssertEqual(try structured(second.responses[4])["hasValue"] as? Bool, true)

        XCTAssertEqual((second.responses[5]["result"] as? [String: Any])?["isError"] as? Bool, true)
        XCTAssertEqual((second.responses[6]["result"] as? [String: Any])?["isError"] as? Bool, true)
        XCTAssertEqual((second.responses[7]["error"] as? [String: Any])?["code"] as? Int, -32601)
    }

    // MARK: - Helpers

    private func request(_ id: Int, _ method: String, _ params: [String: Any] = [:]) -> [String: Any] {
        ["jsonrpc": "2.0", "id": id, "method": method, "params": params]
    }

    private func call(_ id: Int, _ tool: String, _ arguments: [String: Any]) -> [String: Any] {
        request(id, "tools/call", ["name": tool, "arguments": arguments])
    }

    private func structured(_ response: [String: Any]) throws -> [String: Any] {
        let result = try XCTUnwrap(response["result"] as? [String: Any])
        XCTAssertEqual(result["isError"] as? Bool, false)
        return try XCTUnwrap(result["structuredContent"] as? [String: Any])
    }

    private func mcp(_ messages: [[String: Any]]) throws -> (responses: [[String: Any]], raw: String) {
        let input = try messages.map { String(decoding: try JSONSerialization.data(withJSONObject: $0), as: UTF8.self) }
            .joined(separator: "\n") + "\n"
        let result = try run(["mcp"], stdin: input)
        XCTAssertEqual(result.status, 0, result.err)
        let responses = try result.out.split(separator: "\n").map {
            try XCTUnwrap(JSONSerialization.jsonObject(with: Data($0.utf8)) as? [String: Any])
        }
        return (responses, result.out + result.err)
    }

    private func run(_ arguments: [String], stdin: String? = nil) throws -> (status: Int32, out: String, err: String) {
        let process = Process()
        process.executableURL = productsDirectory.appendingPathComponent("llavero")
        process.arguments = arguments
        var env = ProcessInfo.processInfo.environment
        env["LLAVERO_HOME"] = home.path
        env["LLAVERO_SERVICE"] = service
        process.environment = env
        let out = Pipe(), err = Pipe(), input = Pipe()
        process.standardOutput = out
        process.standardError = err
        process.standardInput = input
        try process.run()
        if let stdin { input.fileHandleForWriting.write(Data(stdin.utf8)) }
        try input.fileHandleForWriting.close()
        let outData = out.fileHandleForReading.readDataToEndOfFile()
        let errData = err.fileHandleForReading.readDataToEndOfFile()
        process.waitUntilExit()
        return (process.terminationStatus, String(decoding: outData, as: UTF8.self), String(decoding: errData, as: UTF8.self))
    }

    private var productsDirectory: URL {
        for bundle in Bundle.allBundles where bundle.bundlePath.hasSuffix(".xctest") {
            return bundle.bundleURL.deletingLastPathComponent()
        }
        fatalError("products directory not found")
    }
}
