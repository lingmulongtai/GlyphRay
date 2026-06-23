import Foundation

#if canImport(Security)
import Security
#endif

enum MacKeychainError: Error, CustomStringConvertible {
    case frameworkUnavailable
    case unexpectedData
    case operationFailed(OSStatus)

    var description: String {
        switch self {
        case .frameworkUnavailable:
            return "Security framework is unavailable"
        case .unexpectedData:
            return "Keychain returned an unexpected item type"
        case .operationFailed(let status):
            return "Keychain operation failed: \(status)"
        }
    }
}

protocol KeychainSecretStoring {
    func save(_ data: Data, account: String) throws
    func load(account: String) throws -> Data?
    func delete(account: String) throws
}

final class KeychainSecretStore: KeychainSecretStoring {
    private let service: String

    init(service: String = "com.glyphray.host") {
        self.service = service
    }

    func save(_ data: Data, account: String) throws {
        #if canImport(Security)
        let query = baseQuery(account: account)
        let attributes: [String: Any] = [
            kSecValueData as String: data,
            kSecAttrAccessible as String: kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly
        ]
        let updateStatus = SecItemUpdate(query as CFDictionary, attributes as CFDictionary)
        if updateStatus == errSecSuccess {
            return
        }
        guard updateStatus == errSecItemNotFound else {
            throw MacKeychainError.operationFailed(updateStatus)
        }

        var query = baseQuery(account: account)
        query[kSecValueData as String] = data
        query[kSecAttrAccessible as String] = kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly

        let status = SecItemAdd(query as CFDictionary, nil)
        guard status == errSecSuccess else {
            throw MacKeychainError.operationFailed(status)
        }
        #else
        throw MacKeychainError.frameworkUnavailable
        #endif
    }

    func load(account: String) throws -> Data? {
        #if canImport(Security)
        var query = baseQuery(account: account)
        query[kSecReturnData as String] = true
        query[kSecMatchLimit as String] = kSecMatchLimitOne

        var result: CFTypeRef?
        let status = SecItemCopyMatching(query as CFDictionary, &result)
        if status == errSecItemNotFound {
            return nil
        }
        guard status == errSecSuccess else {
            throw MacKeychainError.operationFailed(status)
        }
        guard let data = result as? Data else {
            throw MacKeychainError.unexpectedData
        }
        return data
        #else
        throw MacKeychainError.frameworkUnavailable
        #endif
    }

    func delete(account: String) throws {
        #if canImport(Security)
        let status = SecItemDelete(baseQuery(account: account) as CFDictionary)
        guard status == errSecSuccess || status == errSecItemNotFound else {
            throw MacKeychainError.operationFailed(status)
        }
        #else
        throw MacKeychainError.frameworkUnavailable
        #endif
    }

    private func baseQuery(account: String) -> [String: Any] {
        #if canImport(Security)
        return [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: account
        ]
        #else
        return [:]
        #endif
    }
}
