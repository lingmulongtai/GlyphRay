import Foundation

final class MacTrustedClientStore {
    struct LoadResult {
        let clients: [MacPairingClient]
        let quarantinedAccount: String?
    }

    private let keychain: KeychainSecretStoring
    private let account: String
    private let encoder = JSONEncoder()
    private let decoder = JSONDecoder()

    init(
        keychain: KeychainSecretStoring = KeychainSecretStore(),
        account: String = "trusted-mac-clients-v1"
    ) {
        self.keychain = keychain
        self.account = account
    }

    func load() throws -> [MacPairingClient] {
        try loadOrRecover().clients
    }

    func loadOrRecover() throws -> LoadResult {
        guard let data = try keychain.load(account: account) else {
            return LoadResult(clients: [], quarantinedAccount: nil)
        }
        do {
            return LoadResult(
                clients: try decoder.decode([MacPairingClient].self, from: data),
                quarantinedAccount: nil
            )
        } catch {
            let quarantine = macRecoveryAccount(for: account)
            try keychain.save(data, account: quarantine)
            try keychain.delete(account: account)
            return LoadResult(clients: [], quarantinedAccount: quarantine)
        }
    }

    func save(_ clients: [MacPairingClient]) throws {
        let data = try encoder.encode(clients)
        try keychain.save(data, account: account)
    }

    func clear() throws {
        try keychain.delete(account: account)
    }
}
