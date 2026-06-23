import Foundation
import XCTest
@testable import GlyphRayMacHost

final class MacRemoteInputTests: XCTestCase {
    func testMousePayloadMatchesAndroidBincodeLayout() throws {
        var payload = Data()
        payload.appendLE(UInt32(11))
        payload.appendLE(UInt64(7))
        payload.appendLE(UInt64(8))
        payload.appendLE(UInt32(2))
        payload.appendFloat(120.5)
        payload.appendFloat(92.25)
        payload.appendFloat(-1.0)
        payload.appendFloat(2.0)
        payload.appendLE(UInt32(3))

        XCTAssertEqual(
            try MacRemoteInputDecoder.decodeMouse(payload),
            MacRemoteMouseInput(
                sequence: 7,
                timestampUs: 8,
                displayID: 2,
                x: 120.5,
                y: 92.25,
                wheelDeltaX: -1,
                wheelDeltaY: 2,
                buttonFlags: 3
            )
        )
    }

    func testKeyboardPayloadMatchesAndroidBincodeLayout() throws {
        var payload = Data()
        payload.appendLE(UInt32(12))
        payload.appendLE(UInt64(9))
        payload.appendLE(UInt64(10))
        payload.appendLE(UInt32(30))
        payload.appendLE(UInt32(0x41))
        payload.append(1)
        payload.appendLE(UInt32(0x1001))

        XCTAssertEqual(
            try MacRemoteInputDecoder.decodeKeyboard(payload),
            MacRemoteKeyboardInput(
                sequence: 9,
                timestampUs: 10,
                scanCode: 30,
                virtualKey: 0x41,
                pressed: true,
                modifiers: 0x1001
            )
        )
    }

    func testTouchPayloadMatchesAndroidBincodeLayout() throws {
        var payload = Data()
        payload.appendLE(UInt32(18))
        payload.appendLE(UInt64(11))
        payload.appendLE(UInt64(12))
        payload.appendLE(UInt32(1))
        payload.appendLE(UInt64(1))
        payload.appendLE(UInt64(13))
        payload.appendLE(UInt64(14))
        payload.appendLE(UInt32(4))
        payload.appendLE(UInt32(1))
        payload.appendFloat(10)
        payload.appendFloat(20)
        payload.appendFloat(0.5)
        payload.appendFloat(8)
        payload.appendFloat(6)
        payload.appendFloat(15)
        payload.appendLE(UInt32(0))

        let decoded = try MacRemoteInputDecoder.decodeTouchBatch(payload)
        XCTAssertEqual(decoded.batchSequence, 11)
        XCTAssertEqual(decoded.displayID, 1)
        XCTAssertEqual(decoded.samples.count, 1)
        XCTAssertEqual(decoded.samples[0].pointerID, 4)
        XCTAssertEqual(decoded.samples[0].pressure, 0.5)
    }

    func testTouchDecoderRejectsUnboundedBatch() {
        var payload = Data()
        payload.appendLE(UInt32(18))
        payload.appendLE(UInt64(1))
        payload.appendLE(UInt64(2))
        payload.appendLE(UInt32(0))
        payload.appendLE(UInt64(257))
        XCTAssertThrowsError(try MacRemoteInputDecoder.decodeTouchBatch(payload))
    }
}

private extension Data {
    mutating func appendLE<T: FixedWidthInteger>(_ value: T) {
        var little = value.littleEndian
        Swift.withUnsafeBytes(of: &little) { append(contentsOf: $0) }
    }

    mutating func appendFloat(_ value: Float) {
        appendLE(value.bitPattern)
    }
}
