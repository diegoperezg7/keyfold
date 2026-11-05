import Foundation
import LlaveroCore
import XCTest

/// The `llavero app` window drives KeysViewModel; an assignment made through it
/// must show up in the real `llavero list`.
final class KeysViewModelTests: XCTestCase {
    private var home: URL!
    private var service: String!
    private var model: KeysViewModel!

    override func setUpWithError() throws {
        home = FileManager.default.temporaryDirectory.appendingPathComponent("llavero-app-\(UUID().uuidString)")
        service = "com.diego.llavero.tests.\(UUID().uuidString)"
        let llavero = Llavero(store: KeychainStore(service: service), registryStore: RegistryStore(directory: home))
        try llavero.createSlot("EXA_API_KEY", project: nil)
        model = KeysViewModel(llavero: llavero)
    }

    override func tearDownWithError() throws {
        try? FileManager.default.removeItem(at: home)
    }

    func testAssignFromWindowIsReflectedByCLIList() throws {
        model.reload()
        XCTAssertEqual(model.keys.map(\.name), ["EXA_API_KEY"])
        XCTAssertEqual(model.keys.first?.projects, [])

        XCTAssertTrue(model.assign("EXA_API_KEY", to: "  demo-app "))
        XCTAssertNil(model.errorMessage)
        XCTAssertEqual(model.keys.first?.projects, ["demo-app"])

        let listed = try cliList()
        let line = try XCTUnwrap(listed.split(separator: "\n").first { $0.hasPrefix("EXA_API_KEY") })
        XCTAssertTrue(line.hasSuffix("demo-app"), "llavero list does not show the project: \(line)")
    }

    func testInvalidProjectIsRejectedWithMessage() {
        model.reload()
        XCTAssertFalse(model.assign("EXA_API_KEY", to: "bad project/name"))
        XCTAssertNotNil(model.errorMessage)
        XCTAssertEqual(model.keys.first?.projects, [])
    }

    private func cliList() throws -> String {
        let process = Process()
        process.executableURL = productsDirectory.appendingPathComponent("llavero")
        process.arguments = ["list"]
        var env = ProcessInfo.processInfo.environment
        env["LLAVERO_HOME"] = home.path
        env["LLAVERO_SERVICE"] = service
        process.environment = env
        let out = Pipe()
        process.standardOutput = out
        try process.run()
        let data = out.fileHandleForReading.readDataToEndOfFile()
        process.waitUntilExit()
        XCTAssertEqual(process.terminationStatus, 0)
        return String(decoding: data, as: UTF8.self)
    }

    private var productsDirectory: URL {
        for bundle in Bundle.allBundles where bundle.bundlePath.hasSuffix(".xctest") {
            return bundle.bundleURL.deletingLastPathComponent()
        }
        fatalError("products directory not found")
    }
}
