import Foundation

struct MacRemoteMouseInput: Equatable {
    let sequence: UInt64
    let timestampUs: UInt64
    let displayID: UInt32
    let x: Float
    let y: Float
    let wheelDeltaX: Float
    let wheelDeltaY: Float
    let buttonFlags: UInt32
}

struct MacRemoteKeyboardInput: Equatable {
    let sequence: UInt64
    let timestampUs: UInt64
    let scanCode: UInt32
    let virtualKey: UInt32
    let pressed: Bool
    let modifiers: UInt32
}

struct MacRemoteTouchSample: Equatable {
    let sequence: UInt64
    let timestampUs: UInt64
    let pointerID: UInt32
    let action: UInt32
    let x: Float
    let y: Float
    let pressure: Float
    let major: Float
    let minor: Float
    let orientationDegrees: Float
    let flags: UInt32
}

struct MacRemoteTouchBatch: Equatable {
    let batchSequence: UInt64
    let monotonicTimestampUs: UInt64
    let displayID: UInt32
    let samples: [MacRemoteTouchSample]
}

enum MacRemoteInputDecoder {
    static func decodeMouse(_ payload: Data) throws -> MacRemoteMouseInput {
        var reader = MacInputBinaryReader(payload)
        guard try reader.readUInt32() == 11 else {
            throw MacHostError.transportUnavailable("Payload did not contain MouseInput")
        }
        let input = MacRemoteMouseInput(
            sequence: try reader.readUInt64(),
            timestampUs: try reader.readUInt64(),
            displayID: try reader.readUInt32(),
            x: try reader.readFloat(),
            y: try reader.readFloat(),
            wheelDeltaX: try reader.readFloat(),
            wheelDeltaY: try reader.readFloat(),
            buttonFlags: try reader.readUInt32()
        )
        try reader.requireEnd()
        return input
    }

    static func decodeKeyboard(_ payload: Data) throws -> MacRemoteKeyboardInput {
        var reader = MacInputBinaryReader(payload)
        guard try reader.readUInt32() == 12 else {
            throw MacHostError.transportUnavailable("Payload did not contain KeyboardInput")
        }
        let input = MacRemoteKeyboardInput(
            sequence: try reader.readUInt64(),
            timestampUs: try reader.readUInt64(),
            scanCode: try reader.readUInt32(),
            virtualKey: try reader.readUInt32(),
            pressed: try reader.readUInt8() != 0,
            modifiers: try reader.readUInt32()
        )
        try reader.requireEnd()
        return input
    }

    static func decodeTouchBatch(_ payload: Data) throws -> MacRemoteTouchBatch {
        var reader = MacInputBinaryReader(payload)
        guard try reader.readUInt32() == 18 else {
            throw MacHostError.transportUnavailable("Payload did not contain TouchInputBatch")
        }
        let batchSequence = try reader.readUInt64()
        let timestampUs = try reader.readUInt64()
        let displayID = try reader.readUInt32()
        let count = try reader.readUInt64()
        guard count <= 256 else {
            throw MacHostError.transportUnavailable("Touch batch exceeds sample limit")
        }
        var samples: [MacRemoteTouchSample] = []
        samples.reserveCapacity(Int(count))
        for _ in 0..<count {
            samples.append(MacRemoteTouchSample(
                sequence: try reader.readUInt64(),
                timestampUs: try reader.readUInt64(),
                pointerID: try reader.readUInt32(),
                action: try reader.readUInt32(),
                x: try reader.readFloat(),
                y: try reader.readFloat(),
                pressure: try reader.readFloat(),
                major: try reader.readFloat(),
                minor: try reader.readFloat(),
                orientationDegrees: try reader.readFloat(),
                flags: try reader.readUInt32()
            ))
        }
        try reader.requireEnd()
        return MacRemoteTouchBatch(
            batchSequence: batchSequence,
            monotonicTimestampUs: timestampUs,
            displayID: displayID,
            samples: samples
        )
    }
}

private struct MacInputBinaryReader {
    private let bytes: [UInt8]
    private var offset = 0

    init(_ data: Data) { bytes = Array(data) }

    mutating func readUInt8() throws -> UInt8 { try readInteger() }
    mutating func readUInt32() throws -> UInt32 { try readInteger() }
    mutating func readUInt64() throws -> UInt64 { try readInteger() }

    mutating func readFloat() throws -> Float {
        Float(bitPattern: try readUInt32())
    }

    mutating func requireEnd() throws {
        guard offset == bytes.count else {
            throw MacHostError.transportUnavailable("Input payload has trailing bytes")
        }
    }

    private mutating func readInteger<T: FixedWidthInteger>() throws -> T {
        let size = MemoryLayout<T>.size
        guard offset + size <= bytes.count else {
            throw MacHostError.transportUnavailable("Unexpected end of input payload")
        }
        var value: T = 0
        for index in 0..<size {
            value |= T(bytes[offset + index]) << T(index * 8)
        }
        offset += size
        return value
    }
}
