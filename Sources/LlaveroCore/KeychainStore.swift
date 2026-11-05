import Foundation
import Security

/// Where secret values live. Values never go anywhere else (not the registry, not output).
public protocol SecretStore {
    func set(_ value: String, for name: String) throws
    func value(for name: String) throws -> String?
    func hasValue(for name: String) -> Bool
    func delete(_ name: String) throws
}

public struct KeychainError: Error, CustomStringConvertible {
    public let status: OSStatus

    public var description: String {
        let message = SecCopyErrorMessageString(status, nil) as String? ?? "unknown"
        return "Keychain error \(status): \(message)"
    }
}

/// Stores each value as a generic password in the user's login Keychain,
/// which macOS encrypts at rest. Service = app id, account = secret name.
public struct KeychainStore: SecretStore {
    public static let defaultService = "com.diego.llavero"

    public let service: String

    public init(service: String = KeychainStore.defaultService) {
        self.service = service
    }

    private func baseQuery(_ name: String) -> [CFString: Any] {
        [
            kSecClass: kSecClassGenericPassword,
            kSecAttrService: service,
            kSecAttrAccount: name,
        ]
    }

    public func set(_ value: String, for name: String) throws {
        let data = Data(value.utf8)
        let query = baseQuery(name)
        let update = SecItemUpdate(query as CFDictionary, [kSecValueData: data] as CFDictionary)
        if update == errSecSuccess { return }
        guard update == errSecItemNotFound else { throw KeychainError(status: update) }

        var item = query
        item[kSecValueData] = data
        item[kSecAttrLabel] = "Llavero: \(name)"
        let add = SecItemAdd(item as CFDictionary, nil)
        guard add == errSecSuccess else { throw KeychainError(status: add) }
    }

    public func value(for name: String) throws -> String? {
        var query = baseQuery(name)
        query[kSecReturnData] = true
        query[kSecMatchLimit] = kSecMatchLimitOne
        var result: CFTypeRef?
        let status = SecItemCopyMatching(query as CFDictionary, &result)
        if status == errSecItemNotFound { return nil }
        guard status == errSecSuccess, let data = result as? Data else {
            throw KeychainError(status: status)
        }
        return String(decoding: data, as: UTF8.self)
    }

    /// Checks existence by attributes only, so it never decrypts the value.
    public func hasValue(for name: String) -> Bool {
        var query = baseQuery(name)
        query[kSecReturnAttributes] = true
        query[kSecMatchLimit] = kSecMatchLimitOne
        return SecItemCopyMatching(query as CFDictionary, nil) == errSecSuccess
    }

    public func delete(_ name: String) throws {
        let status = SecItemDelete(baseQuery(name) as CFDictionary)
        guard status == errSecSuccess || status == errSecItemNotFound else {
            throw KeychainError(status: status)
        }
    }
}
