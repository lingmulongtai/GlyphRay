import CryptoKit
import Foundation
import XCTest
@testable import GlyphRayMacHost

final class MacPersistenceRecoveryTests: XCTestCase {
    func testHostIdentityIsStableAndCorruptValueIsQuarantined() throws {
        let keychain = InMemoryKeychain()
        let store = MacHostIdentityStore(keychain: keychain)
        let first = try store.loadOrRecover()
        XCTAssertNil(first.quarantinedAccount)
        XCTAssertEqual(
            try store.loadOrRecover().identity.fingerprint,
            first.identity.fingerprint
        )

        try keychain.save(Data([1, 2, 3]), account: "host-signing-identity-p256-v1")
        let recovered = try store.loadOrRecover()
        let quarantine = try XCTUnwrap(recovered.quarantinedAccount)
        XCTAssertEqual(try keychain.load(account: quarantine), Data([1, 2, 3]))
        XCTAssertNotEqual(recovered.identity.fingerprint, first.identity.fingerprint)
    }

    func testCorruptTrustedClientsAreQuarantined() throws {
        let keychain = InMemoryKeychain()
        let store = MacTrustedClientStore(keychain: keychain)
        let corrupt = Data("not-json".utf8)
        try keychain.save(corrupt, account: "trusted-mac-clients-v1")

        let recovered = try store.loadOrRecover()
        XCTAssertTrue(recovered.clients.isEmpty)
        let quarantine = try XCTUnwrap(recovered.quarantinedAccount)
        XCTAssertEqual(try keychain.load(account: quarantine), corrupt)
        XCTAssertNil(try keychain.load(account: "trusted-mac-clients-v1"))
    }
}

private final class InMemoryKeychain: KeychainSecretStoring {
    private var values: [String: Data] = [:]

    func save(_ data: Data, account: String) throws {
        values[account] = data
    }

    func load(account: String) throws -> Data? {
        values[account]
    }

    func delete(account: String) throws {
        values.removeValue(forKey: account)
    }
}
