import Foundation

struct MacUdpSendTarget: Equatable, Hashable, Codable, Identifiable {
    var host: String
    var port: UInt16

    var id: String {
        "\(host):\(port)"
    }
}
