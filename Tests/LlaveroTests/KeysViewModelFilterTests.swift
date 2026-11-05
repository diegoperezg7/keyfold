import Foundation
import LlaveroCore
import XCTest

/// In-memory store so filter/search/sort logic is tested without the Keychain.
private final class MemoryStore: SecretStore {
    var values: [String: String] = [:]
    func set(_ value: String, for name: String) throws { values[name] = value }
    func value(for name: String) throws -> String? { values[name] }
    func hasValue(for name: String) -> Bool { values[name] != nil }
    func delete(_ name: String) throws { values[name] = nil }
}

final class KeysViewModelFilterTests: XCTestCase {
    private var home: URL!
    private var model: KeysViewModel!

    override func setUpWithError() throws {
        home = FileManager.default.temporaryDirectory.appendingPathComponent("llavero-filter-\(UUID().uuidString)")
        let llavero = Llavero(store: MemoryStore(), registryStore: RegistryStore(directory: home))
        try llavero.createSlot("EXA_API_KEY", project: "search-bot")
        try llavero.createSlot("OPENAI_API_KEY", project: "web")
        try llavero.createSlot("RESEND_API_KEY", project: nil)
        try llavero.setValue("value-exa", for: "EXA_API_KEY")
        try llavero.setValue("value-openai", for: "OPENAI_API_KEY")
        model = KeysViewModel(llavero: llavero)
        model.reload()
    }

    override func tearDownWithError() throws {
        try? FileManager.default.removeItem(at: home)
    }

    func testCountsAndStatusFilter() {
        XCTAssertEqual(model.count(for: .all), 3)
        XCTAssertEqual(model.count(for: .stored), 2)
        XCTAssertEqual(model.count(for: .empty), 1)

        model.statusFilter = .empty
        XCTAssertEqual(model.visibleKeys.map(\.name), ["RESEND_API_KEY"])
        model.statusFilter = .stored
        XCTAssertEqual(model.visibleKeys.map(\.name), ["EXA_API_KEY", "OPENAI_API_KEY"])
    }

    func testSearchMatchesNameOrProjectCaseInsensitive() {
        model.searchText = "openai"
        XCTAssertEqual(model.visibleKeys.map(\.name), ["OPENAI_API_KEY"])
        model.searchText = "  SEARCH-bot "
        XCTAssertEqual(model.visibleKeys.map(\.name), ["EXA_API_KEY"])
        model.searchText = "nothing-matches"
        XCTAssertTrue(model.visibleKeys.isEmpty)
    }

    func testSortByNameAndByStatus() {
        XCTAssertEqual(model.visibleKeys.map(\.name), ["EXA_API_KEY", "OPENAI_API_KEY", "RESEND_API_KEY"])
        model.sortOrder = [KeyPathComparator(\SlotInfo.name, order: .reverse)]
        XCTAssertEqual(model.visibleKeys.map(\.name), ["RESEND_API_KEY", "OPENAI_API_KEY", "EXA_API_KEY"])
        model.sortOrder = [KeyPathComparator(\SlotInfo.statusRank), KeyPathComparator(\SlotInfo.name)]
        XCTAssertEqual(model.visibleKeys.first?.name, "RESEND_API_KEY", "empty keys sort first")
    }

    func testKnownProjectsAndAssignUpdatesThem() {
        XCTAssertEqual(model.knownProjects, ["search-bot", "web"])
        XCTAssertTrue(model.assign("RESEND_API_KEY", to: "web"))
        XCTAssertEqual(model.keys.first { $0.name == "RESEND_API_KEY" }?.projects, ["web"])
        XCTAssertEqual(model.knownProjects, ["search-bot", "web"])
    }
}
