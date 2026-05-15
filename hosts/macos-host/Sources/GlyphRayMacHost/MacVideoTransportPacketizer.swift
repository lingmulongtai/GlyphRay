import Foundation

enum MacTransportPacketizerError: Error, CustomStringConvertible {
    case payloadTooLarge
    case invalidSequence

    var description: String {
        switch self {
        case .payloadTooLarge:
            return "video payload is too large to fragment"
        case .invalidSequence:
            return "encoded frame sequence must be positive"
        }
    }
}

enum MacTransportVideoCodec: UInt8 {
    case h264 = 1
    case h265 = 2
    case av1 = 3
}

struct MacVideoTransportDatagram: Equatable {
    let frameSequence: UInt64
    let fragmentIndex: UInt16
    let fragmentCount: UInt16
    let bytes: Data
}

struct MacVideoPacketizationReport: Equatable {
    let datagrams: [MacVideoTransportDatagram]

    var datagramCount: Int {
        datagrams.count
    }

    var byteCount: Int {
        datagrams.reduce(0) { partial, datagram in
            partial + datagram.bytes.count
        }
    }
}

final class MacVideoTransportPacketizer {
    private let maxFragmentPayload: Int

    init(maxFragmentPayload: Int = 1_200) {
        self.maxFragmentPayload = max(1, maxFragmentPayload)
    }

    func packetize(
        frame: MacEncodedFrame,
        codec: MacTransportVideoCodec = .h264
    ) throws -> MacVideoPacketizationReport {
        guard frame.sequence > 0 else {
            throw MacTransportPacketizerError.invalidSequence
        }

        let frameSequence = UInt64(frame.sequence)
        let accessUnit = encodeAccessUnit(
            frame: frame,
            frameSequence: frameSequence,
            codec: codec
        )
        let fragments = try fragmentPayload(frameSequence: frameSequence, payload: accessUnit)
        let datagrams = fragments.map { fragment in
            MacVideoTransportDatagram(
                frameSequence: frameSequence,
                fragmentIndex: fragment.index,
                fragmentCount: fragment.count,
                bytes: encodeVideoDatagram(
                    frameSequence: frameSequence,
                    presentationTimeUs: UInt64(max(0, frame.presentationTimeUs)),
                    fragmentPayload: fragment.bytes
                )
            )
        }
        return MacVideoPacketizationReport(datagrams: datagrams)
    }

    private func encodeAccessUnit(
        frame: MacEncodedFrame,
        frameSequence: UInt64,
        codec: MacTransportVideoCodec
    ) -> Data {
        var out = Data(capacity: 22 + frame.payload.count)
        out.append(codec.rawValue)
        out.append(frame.isKeyframe ? 1 : 0)
        out.appendLittleEndian(frameSequence)
        out.appendLittleEndian(UInt64(max(0, frame.presentationTimeUs)))
        out.appendLittleEndian(UInt32(frame.payload.count))
        out.append(frame.payload)
        return out
    }

    private func fragmentPayload(
        frameSequence: UInt64,
        payload: Data
    ) throws -> [(index: UInt16, count: UInt16, bytes: Data)] {
        let fragmentCount = max(1, (payload.count + maxFragmentPayload - 1) / maxFragmentPayload)
        guard fragmentCount <= Int(UInt16.max) else {
            throw MacTransportPacketizerError.payloadTooLarge
        }

        var fragments: [(index: UInt16, count: UInt16, bytes: Data)] = []
        if payload.isEmpty {
            fragments.append((0, 1, encodeFragment(frameSequence: frameSequence, index: 0, count: 1, payload: Data())))
            return fragments
        }

        for index in 0..<fragmentCount {
            let start = index * maxFragmentPayload
            let end = min(payload.count, start + maxFragmentPayload)
            let chunk = payload.subdata(in: start..<end)
            fragments.append((
                UInt16(index),
                UInt16(fragmentCount),
                encodeFragment(
                    frameSequence: frameSequence,
                    index: UInt16(index),
                    count: UInt16(fragmentCount),
                    payload: chunk
                )
            ))
        }
        return fragments
    }

    private func encodeFragment(
        frameSequence: UInt64,
        index: UInt16,
        count: UInt16,
        payload: Data
    ) -> Data {
        var out = Data(capacity: 20 + payload.count)
        out.append(contentsOf: [0x47, 0x4c, 0x59, 0x46])
        out.appendLittleEndian(frameSequence)
        out.appendLittleEndian(index)
        out.appendLittleEndian(count)
        out.appendLittleEndian(UInt32(payload.count))
        out.append(payload)
        return out
    }

    private func encodeVideoDatagram(
        frameSequence: UInt64,
        presentationTimeUs: UInt64,
        fragmentPayload: Data
    ) -> Data {
        var out = Data(capacity: 33 + fragmentPayload.count)
        out.append(contentsOf: [0x47, 0x4c, 0x59, 0x54])
        out.appendLittleEndian(UInt16(1))
        out.append(UInt8(1))
        out.appendLittleEndian(UInt16(9))
        out.appendLittleEndian(frameSequence)
        out.appendLittleEndian(presentationTimeUs)
        out.appendLittleEndian(UInt32(fragmentPayload.count))
        out.appendLittleEndian(fragmentPayload.crc32())
        out.append(fragmentPayload)
        return out
    }
}

private extension Data {
    mutating func appendLittleEndian<T: FixedWidthInteger>(_ value: T) {
        var littleEndian = value.littleEndian
        withUnsafeBytes(of: &littleEndian) { bytes in
            append(contentsOf: bytes)
        }
    }

    func crc32() -> UInt32 {
        var crc = UInt32.max
        for byte in self {
            let index = Int((crc ^ UInt32(byte)) & 0xff)
            crc = (crc >> 8) ^ Crc32.table[index]
        }
        return crc ^ UInt32.max
    }
}

private enum Crc32 {
    static let table: [UInt32] = (0..<256).map { value in
        var crc = UInt32(value)
        for _ in 0..<8 {
            if crc & 1 == 1 {
                crc = 0xedb88320 ^ (crc >> 1)
            } else {
                crc >>= 1
            }
        }
        return crc
    }
}
