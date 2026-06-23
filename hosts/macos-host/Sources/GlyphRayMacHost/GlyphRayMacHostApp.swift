import Foundation
import SwiftUI

@main
struct GlyphRayMacHostApp: App {
    @StateObject private var model = HostStatusModel()

    var body: some Scene {
        WindowGroup {
            ContentView(model: model)
        }
    }
}

@MainActor
final class HostStatusModel: ObservableObject {
    private let captureController = ScreenCaptureController()
    private let liveCaptureController = MacLiveCaptureController()
    private let controlRuntime = MacControlRuntime()
    private let discoveryAdvertiser = MacLanDiscoveryAdvertiser()
    private let permissionController = MacPermissionController()
    private let keychainStore = KeychainSecretStore()

    @Published var status: String = "Idle"
    @Published var captureStatus: String = "Capture idle"
    @Published var liveCaptureStatus: String = "Live capture idle"
    @Published var controlStatus: String = "Control runtime idle"
    @Published var discoveryStatus: String = "Discovery advertiser idle"
    @Published var encoderStatus: String = "Encoder idle"
    @Published var keychainStatus: String = "Keychain idle"
    @Published var controlPort: String = "44999"
    @Published var approvedClients: [MacPairingClient] = []
    @Published var secureTargets: [MacUdpSendTarget] = []
    @Published var pairingCode: String?
    @Published var permissions = MacPermissionSnapshot(
        screenRecording: "unknown",
        accessibility: "unknown",
        audio: "unknown",
        inputMonitoring: "manual review"
    )
    @Published var displays: [MacDisplayDescriptor] = []
    private var latestVideoPreference: MacClientVideoPreference?

    init() {
        controlRuntime.onSnapshot = { [weak self] snapshot in
            Task { @MainActor in
                self?.applyControlSnapshot(snapshot)
            }
        }
        discoveryAdvertiser.onSnapshot = { [weak self] snapshot in
            Task { @MainActor in
                self?.applyDiscoverySnapshot(snapshot)
            }
        }
        applyControlSnapshot(controlRuntime.snapshot())
        applyDiscoverySnapshot(discoveryAdvertiser.snapshot())
        refreshReadiness()
    }

    func refreshReadiness() {
        permissions = permissionController.snapshot()
        status = permissions.readyForSmokeTest ? "Ready for local smoke tests" : "Needs permissions"
    }

    func requestScreenRecording() {
        _ = permissionController.requestScreenRecordingAccess()
        refreshReadiness()
    }

    func requestAccessibility() {
        _ = permissionController.requestAccessibilityPrompt()
        refreshReadiness()
    }

    func requestAudioAccess() {
        permissionController.requestAudioAccess { [weak self] _ in
            Task { @MainActor in
                self?.refreshReadiness()
            }
        }
    }

    func refreshDisplays() async {
        captureStatus = "Scanning displays..."
        do {
            displays = try await captureController.availableDisplays()
            captureStatus = displays.isEmpty ? "No displays found" : "\(displays.count) display(s) available"
        } catch {
            captureStatus = "Display scan failed: \(error)"
        }
    }

    func startEncoderSmokeTest() {
        encoderStatus = "Starting VideoToolbox..."
        do {
            let encoder = VideoToolboxEncoder(settings: .lowLatencyPreview)
            try encoder.start()
            encoder.stop()
            encoderStatus = "VideoToolbox H.264 low-latency session created"
        } catch {
            encoderStatus = "Encoder unavailable: \(error)"
        }
    }

    func startLiveCaptureProbe() {
        liveCaptureStatus = "Starting ScreenCaptureKit stream..."
        Task {
            do {
                let result = try await liveCaptureController.startFirstDisplayProbe()
                liveCaptureStatus = "Captured \(result.frameCount) frame(s) from display \(result.displayID) at \(result.width)x\(result.height)"
            } catch {
                liveCaptureStatus = "Live capture unavailable: \(error)"
            }
        }
    }

    func startLiveEncodeProbe() {
        liveCaptureStatus = "Starting ScreenCaptureKit -> VideoToolbox probe..."
        Task {
            do {
                let result = try await liveCaptureController.startFirstDisplayEncodeProbe()
                liveCaptureStatus = "Captured \(result.capturedFrames), encoded \(result.encodedFrames) frame(s) / \(result.encodedBytes) bytes from display \(result.displayID) at \(result.width)x\(result.height)"
            } catch {
                liveCaptureStatus = "Live encode unavailable: \(error)"
            }
        }
    }

    func startLiveTransportProbe() {
        liveCaptureStatus = "Starting capture -> encode -> video packetizer probe..."
        Task {
            do {
                let result = try await liveCaptureController.startFirstDisplayTransportProbe()
                liveCaptureStatus = "Captured \(result.capturedFrames), encoded \(result.encodedFrames), packetized \(result.videoDatagrams) datagram(s) / \(result.transportBytes) bytes from display \(result.displayID)"
            } catch {
                liveCaptureStatus = "Live transport unavailable: \(error)"
            }
        }
    }

    func startApprovedUdpStream() {
        guard let target = controlRuntime.preferredSecureTarget() else {
            liveCaptureStatus = "Approved stream unavailable: no encrypted trusted client endpoint"
            return
        }
        startUdpStream(
            to: target,
            source: "encrypted approved client",
            transformDatagram: { [controlRuntime = self.controlRuntime] datagram in
                try controlRuntime.sealVideoDatagram(datagram, for: target)
            }
        )
    }

    private func startUdpStream(
        to target: MacUdpSendTarget,
        source: String,
        transformDatagram: @escaping (Data) throws -> Data
    ) {
        liveCaptureStatus = "Starting continuous UDP video stream..."
        Task {
            do {
                let result = try await liveCaptureController.startFirstDisplayUdpStream(
                    to: target,
                    preference: latestVideoPreference,
                    transformDatagram: transformDatagram
                )
                let backpressure = result.backpressureLimited ? " · backpressure limited" : ""
                liveCaptureStatus = "Streaming \(source) display \(result.displayID) at \(result.width)x\(result.height) to \(result.target.host):\(result.target.port) · stream \(result.streamID.uuidString.prefix(8)) · high \(result.highWatermarkDatagrams) · reconnects \(result.reconnectCount)\(backpressure)"
            } catch {
                liveCaptureStatus = "UDP stream unavailable: \(error)"
            }
        }
    }

    func startControlRuntime() {
        guard let port = UInt16(controlPort.trimmingCharacters(in: .whitespacesAndNewlines)) else {
            controlStatus = "Control runtime unavailable: invalid port"
            return
        }

        do {
            try controlRuntime.start(port: port)
            try discoveryAdvertiser.start(controlPort: port)
            controlStatus = "Starting control runtime on UDP \(port)..."
        } catch {
            controlStatus = "Control runtime unavailable: \(error)"
        }
    }

    func stopControlRuntime() {
        controlRuntime.stop()
        discoveryAdvertiser.stop()
    }

    func clearTrustedClients() {
        controlRuntime.clearTrustedClients()
    }

    func stopUdpStream() {
        liveCaptureStatus = "Stopping UDP video stream..."
        Task {
            do {
                let result = try await liveCaptureController.stopUdpStream()
                liveCaptureStatus = "Stopped stream: encoded \(result.encodedFrames) frame(s), sent \(result.sentDatagrams)/\(result.scheduledDatagrams) datagram(s), dropped \(result.droppedDatagrams), high \(result.highWatermarkDatagrams), in-flight \(result.inFlightDatagrams)"
            } catch {
                liveCaptureStatus = "UDP stream stop unavailable: \(error)"
            }
        }
    }

    func runKeychainSmokeTest() {
        keychainStatus = "Testing Keychain..."
        let account = "diagnostics-smoke-test"
        let payload = Data("glyphray-keychain-smoke".utf8)
        do {
            try keychainStore.save(payload, account: account)
            let loaded = try keychainStore.load(account: account)
            try keychainStore.delete(account: account)
            keychainStatus = loaded == payload ? "Keychain save/load/delete passed" : "Keychain returned unexpected data"
        } catch {
            keychainStatus = "Keychain unavailable: \(error)"
        }
    }

    private func applyControlSnapshot(_ snapshot: MacControlRuntimeSnapshot) {
        approvedClients = snapshot.acceptedClients
        secureTargets = snapshot.secureTargets
        pairingCode = snapshot.pairingCode
        latestVideoPreference = snapshot.lastVideoPreference
        let hostKey = snapshot.hostIdentityFingerprint.map { String($0.prefix(12)) } ?? "unavailable"
        controlStatus = "\(snapshot.lastEvent) · requests \(snapshot.pairingRequestsReceived) · clients \(snapshot.acceptedClients.count) · secure \(snapshot.secureClients) · streamable \(snapshot.secureTargets.count) · input M\(snapshot.mouseEventsInjected)/K\(snapshot.keyboardEventsInjected)/T\(snapshot.touchBatchesInjected) · auth \(snapshot.pendingAuthChallenges) · key \(hostKey)"
    }

    private func applyDiscoverySnapshot(_ snapshot: MacDiscoverySnapshot) {
        discoveryStatus = "\(snapshot.lastEvent) · announcements \(snapshot.announcementsSent)"
    }
}

struct ContentView: View {
    @ObservedObject var model: HostStatusModel

    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            Text("GlyphRay Host")
                .font(.largeTitle)
                .fontWeight(.semibold)
            Text(model.status)
                .foregroundStyle(.secondary)
            Divider()
            Label(model.captureStatus, systemImage: "display")
            Label(model.liveCaptureStatus, systemImage: "dot.radiowaves.left.and.right")
            Label(model.controlStatus, systemImage: "network")
            if let pairingCode = model.pairingCode {
                HStack {
                    Label("Pairing code", systemImage: "number.square")
                    Spacer()
                    Text(pairingCode)
                        .font(.system(size: 28, weight: .semibold, design: .monospaced))
                        .textSelection(.enabled)
                }
                .padding(10)
                .background(.quaternary, in: RoundedRectangle(cornerRadius: 6))
            }
            Label(model.discoveryStatus, systemImage: "antenna.radiowaves.left.and.right")
            Label(model.encoderStatus, systemImage: "video")
            Label(model.keychainStatus, systemImage: "key")
            Label("Screen Recording: \(model.permissions.screenRecording)", systemImage: "rectangle.on.rectangle")
            Label("Accessibility: \(model.permissions.accessibility)", systemImage: "keyboard")
            Label("Input Monitoring: \(model.permissions.inputMonitoring)", systemImage: "lock")
            Label("Audio: \(model.permissions.audio)", systemImage: "waveform")
            Label("Mouse and keyboard input first; Windows Ink-style pen injection is Windows-specific.", systemImage: "hand.draw")
            HStack {
                TextField("Control port", text: $model.controlPort)
                    .textFieldStyle(.roundedBorder)
                    .frame(width: 100)
            }
            VStack(alignment: .leading, spacing: 8) {
                HStack {
                    Button("Refresh") {
                        model.refreshReadiness()
                        Task { await model.refreshDisplays() }
                    }
                    Button("Request Screen Recording") {
                        model.requestScreenRecording()
                    }
                    Button("Request Accessibility") {
                        model.requestAccessibility()
                    }
                    Button("Request Audio") {
                        model.requestAudioAccess()
                    }
                    Button("Encoder Smoke Test") {
                        model.startEncoderSmokeTest()
                    }
                    Button("Keychain Smoke Test") {
                        model.runKeychainSmokeTest()
                    }
                }
                HStack {
                    Button("Live Capture Probe") {
                        model.startLiveCaptureProbe()
                    }
                    Button("Live Encode Probe") {
                        model.startLiveEncodeProbe()
                    }
                    Button("Live Transport Probe") {
                        model.startLiveTransportProbe()
                    }
                    Button("Start Control") {
                        model.startControlRuntime()
                    }
                    Button("Stop Control") {
                        model.stopControlRuntime()
                    }
                    Button("Clear Trust") {
                        model.clearTrustedClients()
                    }
                    Button("Start Approved Stream") {
                        model.startApprovedUdpStream()
                    }
                    Button("Stop Stream") {
                        model.stopUdpStream()
                    }
                }
            }
            .buttonStyle(.bordered)

            if !model.displays.isEmpty {
                Divider()
                ForEach(model.displays) { display in
                    Text(display.label)
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
            }

            if !model.approvedClients.isEmpty {
                Divider()
                ForEach(model.approvedClients) { client in
                    let secure = model.secureTargets.contains(client.target) ? "encrypted" : "trusted"
                    Text("\(client.deviceName) · \(client.target.host):\(client.target.port) · \(secure) · \(client.id) · key \(client.publicKeyStatusLabel)")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
            }
        }
        .padding(24)
        .frame(minWidth: 680, minHeight: 380)
        .task {
            await model.refreshDisplays()
        }
    }
}
