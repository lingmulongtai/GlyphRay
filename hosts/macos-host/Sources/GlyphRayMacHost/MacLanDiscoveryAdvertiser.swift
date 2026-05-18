import Foundation

#if canImport(Network)
import Network
#endif

struct MacDiscoverySnapshot: Equatable {
    let advertising: Bool
    let discoveryPort: UInt16
    let controlPort: UInt16
    let announcementsSent: UInt64
    let lastEvent: String
}

final class MacLanDiscoveryAdvertiser {
    var onSnapshot: ((MacDiscoverySnapshot) -> Void)?

    private let queue = DispatchQueue(label: "com.glyphray.mac.discovery")
    private var discoveryPort: UInt16 = 44_998
    private var controlPort: UInt16 = 44_999
    private var videoPort: UInt16 = 45_000
    private var announcementsSent: UInt64 = 0
    private var lastEvent = "Discovery advertiser idle"

    #if canImport(Network)
    private var connection: NWConnection?
    private var timer: DispatchSourceTimer?
    #endif

    func start(
        discoveryPort: UInt16 = 44_998,
        controlPort: UInt16 = 44_999,
        videoPort: UInt16 = 45_000
    ) throws {
        #if canImport(Network)
        guard connection == nil else {
            publishSnapshot()
            return
        }
        guard let nwPort = NWEndpoint.Port(rawValue: discoveryPort) else {
            throw MacHostError.transportUnavailable("Invalid discovery port \(discoveryPort)")
        }

        self.discoveryPort = discoveryPort
        self.controlPort = controlPort
        self.videoPort = videoPort
        let connection = NWConnection(
            host: NWEndpoint.Host("255.255.255.255"),
            port: nwPort,
            using: .udp
        )
        connection.stateUpdateHandler = { [weak self] state in
            self?.queue.async {
                self?.handleConnectionState(state)
            }
        }
        connection.start(queue: queue)
        self.connection = connection

        let timer = DispatchSource.makeTimerSource(queue: queue)
        timer.schedule(deadline: .now(), repeating: .seconds(1))
        timer.setEventHandler { [weak self] in
            self?.sendAnnouncement()
        }
        self.timer = timer
        timer.resume()
        lastEvent = "Discovery advertiser starting on UDP \(discoveryPort)"
        publishSnapshot()
        #else
        throw MacHostError.frameworkUnavailable("Network")
        #endif
    }

    func stop() {
        #if canImport(Network)
        queue.async {
            self.timer?.cancel()
            self.timer = nil
            self.connection?.cancel()
            self.connection = nil
            self.lastEvent = "Discovery advertiser stopped"
            self.publishSnapshot()
        }
        #endif
    }

    func snapshot() -> MacDiscoverySnapshot {
        queue.sync {
            makeSnapshot()
        }
    }

    #if canImport(Network)
    private func handleConnectionState(_ state: NWConnection.State) {
        switch state {
        case .ready:
            lastEvent = "Discovery advertiser broadcasting on UDP \(discoveryPort)"
        case .failed(let error):
            lastEvent = "Discovery advertiser failed: \(error)"
        case .cancelled:
            lastEvent = "Discovery advertiser stopped"
        default:
            break
        }
        publishSnapshot()
    }

    private func sendAnnouncement() {
        guard let connection else {
            return
        }
        let payload = MacDiscoveryAdvertisement(
            hostName: "GlyphRay Mac \(ProcessInfo.processInfo.hostName)",
            controlPort: controlPort,
            videoPort: videoPort
        ).encode()
        connection.send(content: payload, completion: .contentProcessed { [weak self] error in
            self?.queue.async {
                if let error {
                    self?.lastEvent = "Discovery send failed: \(error)"
                } else {
                    self?.announcementsSent += 1
                    self?.lastEvent = "Discovery announcement sent"
                }
                self?.publishSnapshot()
            }
        })
    }
    #endif

    private func makeSnapshot() -> MacDiscoverySnapshot {
        #if canImport(Network)
        let advertising = connection != nil
        #else
        let advertising = false
        #endif
        return MacDiscoverySnapshot(
            advertising: advertising,
            discoveryPort: discoveryPort,
            controlPort: controlPort,
            announcementsSent: announcementsSent,
            lastEvent: lastEvent
        )
    }

    private func publishSnapshot() {
        onSnapshot?(makeSnapshot())
    }
}

private struct MacDiscoveryAdvertisement {
    let hostName: String
    let controlPort: UInt16
    let videoPort: UInt16

    func encode() -> Data {
        let nameBytes = Array(hostName.utf8.prefix(255))
        var out = Data(capacity: 33 + nameBytes.count)
        out.append(contentsOf: [0x47, 0x4c, 0x59, 0x44])
        out.appendLittleEndian(UInt16(1))
        out.append(hostID(nameBytes: nameBytes))
        out.appendLittleEndian(UInt16(1))
        out.appendLittleEndian(controlPort)
        out.appendLittleEndian(videoPort)
        out.append(UInt8(0b0000_0110))
        out.append(UInt8(0))
        out.append(UInt8(nameBytes.count))
        out.append(contentsOf: [0, 0])
        out.append(contentsOf: nameBytes)
        return out
    }

    private func hostID(nameBytes: [UInt8]) -> Data {
        var seed = Data(nameBytes)
        seed.appendLittleEndian(controlPort)
        let first = macDiscoveryCRC32(seed)
        seed.appendLittleEndian(videoPort)
        let second = macDiscoveryCRC32(seed)
        var out = Data(capacity: 16)
        out.appendLittleEndian(first)
        out.appendLittleEndian(second)
        out.appendLittleEndian(first ^ 0xa5a5_5a5a)
        out.appendLittleEndian(second ^ 0x5a5a_a5a5)
        return out
    }
}

private extension Data {
    mutating func appendLittleEndian<T: FixedWidthInteger>(_ value: T) {
        var littleEndian = value.littleEndian
        Swift.withUnsafeBytes(of: &littleEndian) { bytes in
            append(contentsOf: bytes)
        }
    }
}

private func macDiscoveryCRC32(_ data: Data) -> UInt32 {
    var crc = UInt32.max
    for byte in data {
        let index = Int((crc ^ UInt32(byte)) & 0xff)
        crc = (crc >> 8) ^ MacDiscoveryCRC32.table[index]
    }
    return crc ^ UInt32.max
}

private enum MacDiscoveryCRC32 {
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
