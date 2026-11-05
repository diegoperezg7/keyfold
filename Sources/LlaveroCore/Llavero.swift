import Foundation

public enum LlaveroError: Error, Equatable {
    case invalidName(String)
    case invalidProject(String)
    case unknownSecret(String)
    case emptyValue
}

/// What the agent is allowed to see about a secret: never the value.
public struct SlotInfo: Equatable {
    public let name: String
    public let hasValue: Bool
    public let projects: [String]

    public var reference: String { "llavero://\(name)" }
}

public final class Llavero {
    private let store: SecretStore
    private let registryStore: RegistryStore

    public init(store: SecretStore, registryStore: RegistryStore) {
        self.store = store
        self.registryStore = registryStore
    }

    /// Creates an empty slot and, if given, assigns it to a project.
    @discardableResult
    public func createSlot(_ name: String, project: String?) throws -> SlotInfo {
        try validate(name: name)
        var registry = try registryStore.load()
        if registry.secrets[name] == nil {
            registry.secrets[name] = SecretSlot(createdAt: Date())
        }
        if let project {
            try validate(project: project)
            add(name, to: project, in: &registry)
        }
        try registryStore.save(registry)
        return info(name, registry)
    }

    public func setValue(_ value: String, for name: String) throws {
        let registry = try registryStore.load()
        guard registry.secrets[name] != nil else { throw LlaveroError.unknownSecret(name) }
        guard !value.isEmpty else { throw LlaveroError.emptyValue }
        try store.set(value, for: name)
    }

    public func assign(_ name: String, to project: String) throws {
        try validate(project: project)
        var registry = try registryStore.load()
        guard registry.secrets[name] != nil else { throw LlaveroError.unknownSecret(name) }
        add(name, to: project, in: &registry)
        try registryStore.save(registry)
    }

    public func list(project: String? = nil) throws -> [SlotInfo] {
        let registry = try registryStore.load()
        let names = project.map { registry.projects[$0] ?? [] } ?? Array(registry.secrets.keys)
        return names.sorted().map { info($0, registry) }
    }

    public func delete(_ name: String) throws {
        var registry = try registryStore.load()
        guard registry.secrets[name] != nil else { throw LlaveroError.unknownSecret(name) }
        try store.delete(name)
        registry.secrets[name] = nil
        for (project, names) in registry.projects {
            let remaining = names.filter { $0 != name }
            registry.projects[project] = remaining.isEmpty ? nil : remaining
        }
        try registryStore.save(registry)
    }

    /// Only the secrets assigned to `project` that have a value. Used by `run`,
    /// whose child output is redacted before it reaches the caller.
    public func values(for project: String) throws -> [(name: String, value: String)] {
        let registry = try registryStore.load()
        return try (registry.projects[project] ?? []).sorted().compactMap { name in
            try store.value(for: name).map { (name, $0) }
        }
    }

    // MARK: - Private

    private func add(_ name: String, to project: String, in registry: inout Registry) {
        var names = registry.projects[project] ?? []
        if !names.contains(name) { names.append(name) }
        registry.projects[project] = names.sorted()
    }

    private func info(_ name: String, _ registry: Registry) -> SlotInfo {
        let projects = registry.projects.filter { $0.value.contains(name) }.map(\.key).sorted()
        return SlotInfo(name: name, hasValue: store.hasValue(for: name), projects: projects)
    }

    private func validate(name: String) throws {
        guard name.range(of: #"^[A-Z_][A-Z0-9_]*$"#, options: .regularExpression) != nil else {
            throw LlaveroError.invalidName(name)
        }
    }

    private func validate(project: String) throws {
        guard project.range(of: #"^[A-Za-z0-9._-]+$"#, options: .regularExpression) != nil else {
            throw LlaveroError.invalidProject(project)
        }
    }
}
