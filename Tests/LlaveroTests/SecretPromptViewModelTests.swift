import Foundation
import LlaveroCore
import XCTest

private final class InMemoryStore: SecretStore {
    var values: [String: String] = [:]
    func set(_ value: String, for name: String) throws { values[name] = value }
    func value(for name: String) throws -> String? { values[name] }
    func hasValue(for name: String) -> Bool { values[name] != nil }
    func delete(_ name: String) throws { values[name] = nil }
}

/// Receive mode: the value is pasted once, stored, and gone from every piece of view state.
final class SecretPromptViewModelTests: XCTestCase {
    private var home: URL!
    private var store: InMemoryStore!
    private var llavero: Llavero!
    private let secret = "exa-receive-" + UUID().uuidString

    override func setUpWithError() throws {
        home = FileManager.default.temporaryDirectory.appendingPathComponent("llavero-receive-\(UUID().uuidString)")
        store = InMemoryStore()
        llavero = Llavero(store: store, registryStore: RegistryStore(directory: home))
    }

    override func tearDownWithError() throws {
        try? FileManager.default.removeItem(at: home)
    }

    func testReceivedValueIsStoredAndNotInViewState() throws {
        try llavero.createSlot("EXA_API_KEY", project: nil)
        let prompt = SecretPromptViewModel(name: "EXA_API_KEY", project: nil) { [llavero] value in
            try llavero!.setValue(value, for: "EXA_API_KEY")
        }
        XCTAssertEqual(prompt.value, "", "the field opens empty")
        XCTAssertFalse(prompt.canSave)

        prompt.value = "  \(secret)\n"
        XCTAssertTrue(prompt.save())

        XCTAssertEqual(store.values["EXA_API_KEY"], secret, "value stored once, trimmed")
        XCTAssertEqual(prompt.value, "")
        XCTAssertNil(prompt.errorMessage)
        XCTAssertFalse(dumped(prompt).contains(secret), "value left in the paste window state")

        let keys = KeysViewModel(llavero: llavero)
        keys.reload()
        XCTAssertEqual(keys.keys.map(\.name), ["EXA_API_KEY"])
        XCTAssertEqual(keys.keys.first?.hasValue, true, "shows as stored")
        // Everything the app window renders. (Not dump(keys): that would walk into the
        // injected store, which in this test is an in-memory fake holding the value.)
        let rendered = dumped(keys.keys) + dumped(keys.visibleKeys) + dumped(keys.knownProjects)
            + dumped(keys.errorMessage as Any) + keys.searchText
        XCTAssertFalse(rendered.contains(secret), "value present in the app window state")
        XCTAssertFalse(dumped(KeychainStore(service: "x")).contains(secret), "the real store holds no value in memory")
    }

    func testCancelDropsTypedValue() throws {
        try llavero.createSlot("EXA_API_KEY", project: nil)
        let prompt = SecretPromptViewModel(name: "EXA_API_KEY", project: nil) { [llavero] value in
            try llavero!.setValue(value, for: "EXA_API_KEY")
        }
        prompt.value = secret
        prompt.cancel()

        XCTAssertEqual(prompt.value, "")
        XCTAssertNil(store.values["EXA_API_KEY"])
        XCTAssertFalse(dumped(prompt).contains(secret))
    }

    func testFailedSaveDoesNotEchoValueInError() {
        struct Failure: Error {}
        let prompt = SecretPromptViewModel(name: "EXA_API_KEY", project: nil) { _ in throw Failure() }
        prompt.value = secret
        XCTAssertFalse(prompt.save())
        XCTAssertNotNil(prompt.errorMessage)
        XCTAssertFalse(prompt.errorMessage?.contains(secret) ?? true)
    }

    /// Full reflection of the object, including @Observable backing storage.
    private func dumped(_ object: Any) -> String {
        var output = ""
        dump(object, to: &output, maxDepth: 20)
        return output
    }
}
