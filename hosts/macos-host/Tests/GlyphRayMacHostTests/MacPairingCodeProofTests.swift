import Foundation
import XCTest
@testable import GlyphRayMacHost

final class MacPairingCodeProofTests: XCTestCase {
    func testProofMatchesRustAndAndroidVector() throws {
        let salt = Data((0..<32).map { UInt8($0) })
        let proof = try XCTUnwrap(MacPairingCodeProof.create(code: "123-456", salt: salt))

        XCTAssertEqual(
            proof.map { String(format: "%02x", $0) }.joined(),
            "f9b2e23be7a5543d2f02ce8063bf94df5c74485737dee573cc8bd3802d29d280"
        )
        XCTAssertTrue(MacPairingCodeProof.verify(code: "123 456", salt: salt, proof: proof))
        XCTAssertFalse(MacPairingCodeProof.verify(code: "123-457", salt: salt, proof: proof))
    }

    func testGeneratedCodeHasSixDigits() {
        let code = MacOneTimePairingCode.generate()
        XCTAssertEqual(code.utf8.filter { $0 >= 48 && $0 <= 57 }.count, 6)
        XCTAssertEqual(code.count, 7)
    }
}
