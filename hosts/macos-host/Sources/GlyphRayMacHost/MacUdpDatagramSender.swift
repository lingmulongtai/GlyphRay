import Foundation

#if canImport(Network)
import Network
#endif

struct MacUdpSendTarget: Equatable, Codable {
    var host: String
    var port: UInt16

    static let localPreview = MacUdpSendTarget(host: "127.0.0.1", port: 44999)
}

struct MacUdpSendReport: Equatable {
    let datagrams: Int
    let bytes: Int
    let target: MacUdpSendTarget
}

final class MacUdpDatagramSender {
    private let queue = DispatchQueue(label: "com.glyphray.mac.udp-sender")

    func send(
        datagrams: [MacVideoTransportDatagram],
        to target: MacUdpSendTarget
    ) async throws -> MacUdpSendReport {
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
        defer {
            connection.cancel()
        }

        var sentBytes = 0
        for datagram in datagrams {
            try await send(datagram.bytes, connection: connection)
            sentBytes += datagram.bytes.count
        }

        return MacUdpSendReport(
            datagrams: datagrams.count,
            bytes: sentBytes,
            target: target
        )
        #else
        throw MacHostError.frameworkUnavailable("Network")
        #endif
    }

    #if canImport(Network)
    private func send(_ payload: Data, connection: NWConnection) async throws {
        try await withCheckedThrowingContinuation { (continuation: CheckedContinuation<Void, Error>) in
            connection.send(content: payload, completion: .contentProcessed { error in
                if let error {
                    continuation.resume(throwing: error)
                } else {
                    continuation.resume()
                }
            })
        }
    }
    #endif
}
