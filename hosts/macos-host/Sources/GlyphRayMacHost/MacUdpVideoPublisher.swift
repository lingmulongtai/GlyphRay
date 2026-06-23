import Foundation

#if canImport(Network)
import Network
#endif

struct MacUdpPublisherSnapshot: Equatable {
    let target: MacUdpSendTarget
    let scheduledDatagrams: Int
    let scheduledBytes: Int
    let sentDatagrams: Int
    let sentBytes: Int
    let droppedDatagrams: Int
    let droppedBytes: Int
    let inFlightDatagrams: Int
    let maxInFlightDatagrams: Int
    let highWatermarkDatagrams: Int
    let lastError: String?
}

final class MacUdpVideoPublisher {
    private let target: MacUdpSendTarget
    private let maxInFlightDatagrams: Int
    private let transformDatagram: (Data) throws -> Data
    private let queue = DispatchQueue(label: "com.glyphray.mac.udp-video-publisher")
    private var scheduledDatagrams = 0
    private var scheduledBytes = 0
    private var sentDatagrams = 0
    private var sentBytes = 0
    private var droppedDatagrams = 0
    private var droppedBytes = 0
    private var inFlightDatagrams = 0
    private var highWatermarkDatagrams = 0
    private var lastError: String?

    #if canImport(Network)
    private var connection: NWConnection?
    #endif

    init(
        target: MacUdpSendTarget,
        maxInFlightDatagrams: Int = 96,
        transformDatagram: @escaping (Data) throws -> Data
    ) throws {
        self.target = target
        self.maxInFlightDatagrams = max(1, maxInFlightDatagrams)
        self.transformDatagram = transformDatagram

        #if canImport(Network)
        guard let port = NWEndpoint.Port(rawValue: target.port) else {
            throw MacHostError.transportUnavailable("Invalid UDP port \(target.port)")
        }
        let connection = NWConnection(
            host: NWEndpoint.Host(target.host),
            port: port,
            using: .udp
        )
        connection.start(queue: queue)
        self.connection = connection
        #else
        throw MacHostError.frameworkUnavailable("Network")
        #endif
    }

    func publish(_ datagram: MacVideoTransportDatagram) {
        #if canImport(Network)
        let plaintextBytes = datagram.bytes
        queue.async { [weak self] in
            guard let self else {
                return
            }
            guard let connection = self.connection else {
                self.droppedDatagrams += 1
                self.droppedBytes += plaintextBytes.count
                self.lastError = "video publisher is stopped"
                return
            }
            guard self.inFlightDatagrams < self.maxInFlightDatagrams else {
                self.droppedDatagrams += 1
                self.droppedBytes += plaintextBytes.count
                self.lastError = "video send backlog capped at \(self.maxInFlightDatagrams) datagrams"
                return
            }
            let bytes: Data
            do {
                bytes = try self.transformDatagram(plaintextBytes)
            } catch {
                self.droppedDatagrams += 1
                self.droppedBytes += plaintextBytes.count
                self.lastError = "video datagram transform failed: \(error)"
                return
            }
            self.scheduledDatagrams += 1
            self.scheduledBytes += bytes.count
            self.inFlightDatagrams += 1
            self.highWatermarkDatagrams = max(self.highWatermarkDatagrams, self.inFlightDatagrams)
            connection.send(content: bytes, completion: .contentProcessed { [weak self] error in
                self?.queue.async {
                    guard let self else {
                        return
                    }
                    self.inFlightDatagrams = max(0, self.inFlightDatagrams - 1)
                    if let error {
                        self.droppedDatagrams += 1
                        self.droppedBytes += bytes.count
                        self.lastError = "\(error)"
                    } else {
                        self.sentDatagrams += 1
                        self.sentBytes += bytes.count
                        self.lastError = nil
                    }
                }
            })
        }
        #endif
    }

    func snapshot() -> MacUdpPublisherSnapshot {
        queue.sync {
            snapshotLocked()
        }
    }

    func stop() -> MacUdpPublisherSnapshot {
        queue.sync {
            #if canImport(Network)
            connection?.cancel()
            connection = nil
            #endif
            return snapshotLocked()
        }
    }

    private func snapshotLocked() -> MacUdpPublisherSnapshot {
        MacUdpPublisherSnapshot(
            target: target,
            scheduledDatagrams: scheduledDatagrams,
            scheduledBytes: scheduledBytes,
            sentDatagrams: sentDatagrams,
            sentBytes: sentBytes,
            droppedDatagrams: droppedDatagrams,
            droppedBytes: droppedBytes,
            inFlightDatagrams: inFlightDatagrams,
            maxInFlightDatagrams: maxInFlightDatagrams,
            highWatermarkDatagrams: highWatermarkDatagrams,
            lastError: lastError
        )
    }
}
