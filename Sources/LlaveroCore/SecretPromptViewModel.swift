import Foundation
import Observation

/// State for the paste window (create, set, receive and the app's "add value").
/// The value only lives in `value` while the user is typing; it is cleared on save
/// and on cancel, and is never part of any message.
@Observable
public final class SecretPromptViewModel {
    public let name: String
    public let project: String?
    public var value = ""
    public var errorMessage: String?

    private let onSave: (String) throws -> Void

    public init(name: String, project: String?, onSave: @escaping (String) throws -> Void) {
        self.name = name
        self.project = project
        self.onSave = onSave
    }

    public var canSave: Bool { !trimmed.isEmpty }

    private var trimmed: String { value.trimmingCharacters(in: .whitespacesAndNewlines) }

    /// Stores the value and clears the field. Returns true on success.
    public func save() -> Bool {
        do {
            try onSave(trimmed)
            value = ""
            return true
        } catch {
            errorMessage = L10n.t("No se pudo guardar en el Llavero.", "Could not save to the Keychain.")
            return false
        }
    }

    /// Drops whatever was typed.
    public func cancel() {
        value = ""
    }
}
