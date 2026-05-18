import Foundation

#if canImport(Network)
import Network
#endif

struct MacUdpPublisherSnapshot: Equatable {
    let target: MacUdpSendTarget
    let scheduledDatagrams: Int
    let scheduledBytes: Int
}

final class MacUdpVideoPublisher {
    private let target: MacUdpSendTarget
    private let queue = DispatchQueue(label: "com.glyphray.mac.udp-video-publisher")
    private var scheduledDatagrams = 0
    private var scheduledBytes = 0

    #if canImport(Network)
    private var connection: NWConnection?
    #endif

    init(target: MacUdpSendTarget) throws {
        self.target = target

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
        let bytes = datagram.bytes
        queue.async { [weak self] in
            guard let self else {
                return
            }
            self.scheduledDatagrams += 1
            self.scheduledBytes += bytes.count
            self.connection?.send(content: bytes, completion: .contentProcessed { _ in })
        }
        #endif
    }

    func snapshot() -> MacUdpPublisherSnapshot {
        queue.sync {
            MacUdpPublisherSnapshot(
                target: target,
                scheduledDatagrams: scheduledDatagrams,
                scheduledBytes: scheduledBytes
            )
        }
    }

    func stop() -> MacUdpPublisherSnapshot {
        queue.sync {
            #if canImport(Network)
            connection?.cancel()
            connection = nil
            #endif
            return MacUdpPublisherSnapshot(
                target: target,
                scheduledDatagrams: scheduledDatagrams,
                scheduledBytes: scheduledBytes
            )
        }
    }
}
