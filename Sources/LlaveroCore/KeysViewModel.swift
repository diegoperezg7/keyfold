import Foundation
import Observation

extension SlotInfo: Identifiable {
    public var id: String { name }
    /// Sort key for the status column: empty (0) before stored (1).
    public var statusRank: Int { hasValue ? 1 : 0 }
}

public enum KeyStatusFilter: String, CaseIterable, Identifiable {
    case all, stored, empty
    public var id: String { rawValue }
}

/// State for the `llavero app` window. Holds only SlotInfo (name, status, projects),
/// so there is no value anywhere for the view to show or copy.
@Observable
public final class KeysViewModel {
    public private(set) var keys: [SlotInfo] = []
    public var errorMessage: String?
    public var searchText = ""
    public var statusFilter: KeyStatusFilter = .all
    public var sortOrder = [KeyPathComparator(\SlotInfo.name)]

    private let llavero: Llavero

    public init(llavero: Llavero) {
        self.llavero = llavero
    }

    /// Keys after status filter and search (name or project, case-insensitive), sorted.
    public var visibleKeys: [SlotInfo] {
        let query = searchText.trimmingCharacters(in: .whitespacesAndNewlines)
        return keys
            .filter { key in
                switch statusFilter {
                case .all: true
                case .stored: key.hasValue
                case .empty: !key.hasValue
                }
            }
            .filter { key in
                query.isEmpty
                    || key.name.localizedCaseInsensitiveContains(query)
                    || key.projects.contains { $0.localizedCaseInsensitiveContains(query) }
            }
            .sorted(using: sortOrder)
    }

    public var storedCount: Int { keys.filter(\.hasValue).count }
    public var emptyCount: Int { keys.count - storedCount }

    public func count(for filter: KeyStatusFilter) -> Int {
        switch filter {
        case .all: keys.count
        case .stored: storedCount
        case .empty: emptyCount
        }
    }

    /// Every project some key is assigned to, for quick picks when assigning.
    public var knownProjects: [String] {
        Array(Set(keys.flatMap(\.projects))).sorted()
    }

    public func reload() {
        do {
            keys = try llavero.list()
            errorMessage = nil
        } catch {
            errorMessage = L10n.t("No se pudieron cargar las claves.", "Could not load the keys.")
        }
    }

    /// Returns true when the assignment was stored.
    @discardableResult
    public func assign(_ name: String, to project: String) -> Bool {
        let project = project.trimmingCharacters(in: .whitespacesAndNewlines)
        do {
            try llavero.assign(name, to: project)
            reload()
            return true
        } catch LlaveroError.invalidProject {
            errorMessage = L10n.t(
                "Proyecto no válido. Usa letras, números, punto, guion o guion bajo.",
                "Invalid project. Use letters, digits, dot, dash or underscore."
            )
        } catch {
            errorMessage = L10n.t("No se pudo asignar el proyecto.", "Could not assign the project.")
        }
        return false
    }
}
