import Foundation

final class MacTrustedClientStore {
    private let keychain: KeychainSecretStore
    private let account: String
    private let encoder = JSONEncoder()
    private let decoder = JSONDecoder()

    init(
        keychain: KeychainSecretStore = KeychainSecretStore(),
        account: String = "trusted-mac-clients-v1"
    ) {
        self.keychain = keychain
        self.account = account
    }

    func load() throws -> [MacPairingClient] {
        guard let data = try keychain.load(account: account) else {
            return []
        }
        return try decoder.decode([MacPairingClient].self, from: data)
    }

    func save(_ clients: [MacPairingClient]) throws {
        let data = try encoder.encode(clients)
        try keychain.save(data, account: account)
    }

    func clear() throws {
        try keychain.delete(account: account)
    }
}
