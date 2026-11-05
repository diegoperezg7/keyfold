import Foundation
import LlaveroCore
import XCTest

/// Drives the real `llavero` binary against the real login Keychain, isolated by
/// a throwaway Keychain service and registry directory per test.
final class LlaveroCLITests: XCTestCase {
    private var home: URL!
    private var service: String!
    private let secret = "exa-test-" + UUID().uuidString
    private let otherSecret = "other-test-" + UUID().uuidString
    private var outputs: [String] = []

    override func setUpWithError() throws {
        home = FileManager.default.temporaryDirectory.appendingPathComponent("llavero-tests-\(UUID().uuidString)")
        service = "com.diego.llavero.tests.\(UUID().uuidString)"
        outputs = []
    }

    override func tearDownWithError() throws {
        for name in ["EXA_API_KEY", "OTHER_KEY"] { _ = try? cli(["delete", name]) }
        try? FileManager.default.removeItem(at: home)
    }

    // Criterion 1: creating a slot registers the name without a value; list shows names only.
    func testCreateRegistersSlotWithoutValue() throws {
        let created = try cli(["create", "EXA_API_KEY", "--project", "demo", "--no-prompt"])
        XCTAssertEqual(created.status, 0, created.err)
        XCTAssertTrue(created.out.contains("llavero://EXA_API_KEY"))

        XCTAssertFalse(keychainHasItem("EXA_API_KEY"), "a slot must not create a Keychain item")
        let registry = try String(contentsOf: home.appendingPathComponent("registry.json"))
        XCTAssertTrue(registry.contains("EXA_API_KEY"))

        let listed = try cli(["list"])
        XCTAssertEqual(listed.status, 0)
        XCTAssertTrue(listed.out.contains("EXA_API_KEY"))
        XCTAssertTrue(listed.out.contains("demo"))
    }

    // Criterion 2: no CLI output ever contains the stored value.
    func testNoOutputContainsStoredValue() throws {
        try cli(["create", "EXA_API_KEY", "--project", "demo", "--no-prompt"])
        let set = try cli(["set", "EXA_API_KEY", "--stdin"], stdin: secret + "\n")
        XCTAssertEqual(set.status, 0, set.err)
        try cli(["assign", "EXA_API_KEY", "--project", "other"])
        try cli(["list"])
        try cli(["list", "--project", "demo"])
        try cli(["create", "EXA_API_KEY", "--project", "demo", "--no-prompt"])
        try cli(["set", "MISSING", "--stdin"], stdin: secret)
        try cli(["bogus"])

        // A child that tries to print the value on both streams, split mid-line and whole.
        let leak = try cli(["run", "--project", "demo", "--", "/bin/sh", "-c",
                            #"printenv EXA_API_KEY; printf 'x=%s' "$EXA_API_KEY" >&2; echo "a $EXA_API_KEY b""#])
        XCTAssertEqual(leak.status, 0, leak.err)
        XCTAssertTrue(leak.out.contains("[llavero:EXA_API_KEY]"), "child must receive the value and it must be masked")
        XCTAssertTrue(leak.err.contains("x=[llavero:EXA_API_KEY]"))

        // The child really got the right value (compared inside the child, never printed).
        let check = try cli(["run", "--project", "demo", "--", "/bin/sh", "-c", #"read v; test "$v" = "$EXA_API_KEY""#],
                            stdin: secret + "\n")
        XCTAssertEqual(check.status, 0)

        for output in outputs {
            XCTAssertFalse(output.contains(secret), "value leaked in: \(output)")
        }
        let registry = try String(contentsOf: home.appendingPathComponent("registry.json"))
        XCTAssertFalse(registry.contains(secret), "value leaked into the registry file")
    }

    // Criterion 3: the value survives a restart (new process) and lives in the Keychain.
    func testValueSurvivesRestartAndIsStoredInKeychain() throws {
        try cli(["create", "EXA_API_KEY", "--project", "demo", "--no-prompt"])
        try cli(["set", "EXA_API_KEY", "--stdin"], stdin: secret)

        XCTAssertTrue(keychainHasItem("EXA_API_KEY"), "value must be a Keychain item")

        // Every cli() call is a fresh process: this one only has the Keychain to go on.
        let check = try cli(["run", "--project", "demo", "--", "/bin/sh", "-c", #"read v; test "$v" = "$EXA_API_KEY""#],
                            stdin: secret + "\n")
        XCTAssertEqual(check.status, 0, "value did not survive a new process")

        let listed = try cli(["list"])
        XCTAssertTrue(listed.out.contains("EXA_API_KEY"))
        XCTAssertFalse(listed.out.contains(secret))
    }

    // Criterion 4: a project receives only the keys assigned to it.
    func testProjectReceivesOnlyAssignedKeys() throws {
        try cli(["create", "EXA_API_KEY", "--project", "alpha", "--no-prompt"])
        try cli(["create", "OTHER_KEY", "--project", "beta", "--no-prompt"])
        try cli(["set", "EXA_API_KEY", "--stdin"], stdin: secret)
        try cli(["set", "OTHER_KEY", "--stdin"], stdin: otherSecret)

        let alpha = try cli(["run", "--project", "alpha", "--", "/bin/sh", "-c",
                             #"test -n "$EXA_API_KEY" && test -z "${OTHER_KEY+set}""#])
        XCTAssertEqual(alpha.status, 0, "alpha must get EXA_API_KEY and not OTHER_KEY")

        let beta = try cli(["run", "--project", "beta", "--", "/bin/sh", "-c",
                            #"test -n "$OTHER_KEY" && test -z "${EXA_API_KEY+set}""#])
        XCTAssertEqual(beta.status, 0, "beta must get OTHER_KEY and not EXA_API_KEY")

        let unknown = try cli(["run", "--project", "gamma", "--", "/bin/sh", "-c",
                               #"test -z "${EXA_API_KEY+set}" && test -z "${OTHER_KEY+set}""#])
        XCTAssertEqual(unknown.status, 0, "an unassigned project must get no keys")
    }

    // MARK: - Helpers

    @discardableResult
    private func cli(_ arguments: [String], stdin: String? = nil) throws -> (status: Int32, out: String, err: String) {
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

        let result = (process.terminationStatus, String(decoding: outData, as: UTF8.self), String(decoding: errData, as: UTF8.self))
        outputs.append(contentsOf: [result.1, result.2])
        return result
    }

    /// Asks the Keychain for the item's attributes only (no -w), so nothing is decrypted.
    private func keychainHasItem(_ name: String) -> Bool {
        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/usr/bin/security")
        process.arguments = ["find-generic-password", "-s", service, "-a", name]
        process.standardOutput = FileHandle.nullDevice
        process.standardError = FileHandle.nullDevice
        try? process.run()
        process.waitUntilExit()
        return process.terminationStatus == 0
    }

    private var productsDirectory: URL {
        for bundle in Bundle.allBundles where bundle.bundlePath.hasSuffix(".xctest") {
            return bundle.bundleURL.deletingLastPathComponent()
        }
        fatalError("products directory not found")
    }
}
