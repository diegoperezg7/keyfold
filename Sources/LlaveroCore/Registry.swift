import Foundation

/// Names and project assignments only. It never holds a value.
public struct Registry: Codable, Equatable {
    public var secrets: [String: SecretSlot] = [:]
    /// project name -> secret names assigned to it
    public var projects: [String: [String]] = [:]

    public init() {}
}

public struct SecretSlot: Codable, Equatable {
    public var createdAt: Date
}

public struct RegistryStore {
    public let url: URL

    public init(directory: URL) {
        url = directory.appendingPathComponent("registry.json")
    }

    /// `LLAVERO_HOME` overrides the location (used by tests).
    public static func defaultDirectory(environment: [String: String] = ProcessInfo.processInfo.environment) -> URL {
        if let home = environment["LLAVERO_HOME"], !home.isEmpty {
            return URL(fileURLWithPath: home, isDirectory: true)
        }
        return FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
            .appendingPathComponent("Llavero", isDirectory: true)
    }

    public func load() throws -> Registry {
        guard FileManager.default.fileExists(atPath: url.path) else { return Registry() }
        let decoder = JSONDecoder()
        decoder.dateDecodingStrategy = .iso8601
        return try decoder.decode(Registry.self, from: Data(contentsOf: url))
    }

    public func save(_ registry: Registry) throws {
        let directory = url.deletingLastPathComponent()
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        let encoder = JSONEncoder()
        encoder.dateEncodingStrategy = .iso8601
        encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
        try encoder.encode(registry).write(to: url, options: .atomic)
        try FileManager.default.setAttributes([.posixPermissions: 0o600], ofItemAtPath: url.path)
    }
}
