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
            let hardwareStatus = encoder.hardwareAccelerationLabel
            encoder.stop()
            encoderStatus = "VideoToolbox \(hardwareStatus) · \(MacEncoderSettings.lowLatencyPreview.displaySummary)"
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
                liveCaptureStatus = "Captured \(result.capturedFrames), encoded \(result.encodedFrames) frame(s) / \(result.encodedBytes) bytes from display \(result.displayID) at \(result.width)x\(result.height) · \(result.encoderStatus) · \(result.settingsSummary)"
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
                liveCaptureStatus = "Captured \(result.capturedFrames), encoded \(result.encodedFrames), packetized \(result.videoDatagrams) datagram(s) / \(result.transportBytes) bytes from display \(result.displayID) · \(result.encoderStatus) · \(result.settingsSummary)"
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
                liveCaptureStatus = "Streaming \(source) display \(result.displayID) at \(result.width)x\(result.height) to \(result.target.host):\(result.target.port) · stream \(result.streamID.uuidString.prefix(8)) · \(result.encoderStatus) · \(result.settingsSummary) · high \(result.highWatermarkDatagrams) · reconnects \(result.reconnectCount)\(backpressure)"
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
                liveCaptureStatus = "Stopped stream: encoded \(result.encodedFrames) frame(s), sent \(result.sentDatagrams)/\(result.scheduledDatagrams) datagram(s), dropped \(result.droppedDatagrams), high \(result.highWatermarkDatagrams), in-flight \(result.inFlightDatagrams) · \(result.encoderStatus)"
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

enum MacHostSection: String, CaseIterable, Identifiable, Hashable {
    case overview
    case stream
    case permissions
    case clients
    case diagnostics

    var id: String { rawValue }

    var title: String {
        switch self {
        case .overview:
            return "Overview"
        case .stream:
            return "Stream"
        case .permissions:
            return "Permissions"
        case .clients:
            return "Clients"
        case .diagnostics:
            return "Diagnostics"
        }
    }

    var systemImage: String {
        switch self {
        case .overview:
            return "gauge.with.dots.needle.67percent"
        case .stream:
            return "dot.radiowaves.left.and.right"
        case .permissions:
            return "lock.shield"
        case .clients:
            return "ipad.and.iphone"
        case .diagnostics:
            return "waveform.path.ecg"
        }
    }
}

struct ContentView: View {
    @ObservedObject var model: HostStatusModel
    @State private var selection: MacHostSection? = .overview

    var body: some View {
        NavigationSplitView {
            List(selection: $selection) {
                Section("GlyphRay Host") {
                    ForEach(MacHostSection.allCases) { item in
                        Label(item.title, systemImage: item.systemImage)
                            .tag(item)
                    }
                }
            }
            .navigationTitle("GlyphRay")
        } detail: {
            ScrollView {
                detailContent
                    .padding(24)
            }
            .navigationTitle((selection ?? .overview).title)
            .toolbar {
                ToolbarItemGroup {
                    Button {
                        refreshAll()
                    } label: {
                        Label("Refresh", systemImage: "arrow.clockwise")
                    }
                    Button {
                        model.startControlRuntime()
                    } label: {
                        Label("Start", systemImage: "play.fill")
                    }
                    Button {
                        model.stopControlRuntime()
                    } label: {
                        Label("Stop", systemImage: "stop.fill")
                    }
                }
            }
        }
        .frame(minWidth: 900, minHeight: 620)
        .task {
            await model.refreshDisplays()
        }
    }

    @ViewBuilder
    private var detailContent: some View {
        switch selection ?? .overview {
        case .overview:
            overviewPane
        case .stream:
            streamPane
        case .permissions:
            permissionsPane
        case .clients:
            clientsPane
        case .diagnostics:
            diagnosticsPane
        }
    }

    private var overviewPane: some View {
        VStack(alignment: .leading, spacing: 18) {
            HostHeroStatus(
                status: model.status,
                controlStatus: model.controlStatus,
                discoveryStatus: model.discoveryStatus
            )
            if let pairingCode = model.pairingCode {
                PairingCodePanel(code: pairingCode)
            }
            GroupBox {
                VStack(alignment: .leading, spacing: 12) {
                    SettingRow("Control port") {
                        TextField("Port", text: $model.controlPort)
                            .textFieldStyle(.roundedBorder)
                            .frame(width: 96)
                    }
                    SettingRow("Secure clients") {
                        Text("\(model.secureTargets.count)")
                            .foregroundStyle(.secondary)
                    }
                    SettingRow("Trusted clients") {
                        Text("\(model.approvedClients.count)")
                            .foregroundStyle(.secondary)
                    }
                }
            } label: {
                Label("Connection", systemImage: "network")
            }
            GroupBox {
                VStack(alignment: .leading, spacing: 10) {
                    Label(model.encoderStatus, systemImage: "video")
                    Label("Live sessions require VideoToolbox hardware acceleration before streaming.", systemImage: "checkmark.seal")
                        .foregroundStyle(.secondary)
                }
            } label: {
                Label("Low-latency video path", systemImage: "bolt.horizontal")
            }
        }
    }

    private var streamPane: some View {
        VStack(alignment: .leading, spacing: 18) {
            GroupBox {
                VStack(alignment: .leading, spacing: 12) {
                    Label(model.liveCaptureStatus, systemImage: "dot.radiowaves.left.and.right")
                    LazyVGrid(columns: [GridItem(.adaptive(minimum: 180), spacing: 10)], spacing: 10) {
                        HostActionButton("Capture Probe", systemImage: "display") { model.startLiveCaptureProbe() }
                        HostActionButton("Encode Probe", systemImage: "video") { model.startLiveEncodeProbe() }
                        HostActionButton("Transport Probe", systemImage: "network") { model.startLiveTransportProbe() }
                        HostActionButton("Start Stream", systemImage: "play.fill") { model.startApprovedUdpStream() }
                        HostActionButton("Stop Stream", systemImage: "stop.fill") { model.stopUdpStream() }
                    }
                }
            } label: {
                Label("Streaming", systemImage: "play.rectangle")
            }
            GroupBox {
                VStack(alignment: .leading, spacing: 10) {
                    if model.secureTargets.isEmpty {
                        Text("No encrypted Android endpoint is ready.")
                            .foregroundStyle(.secondary)
                    } else {
                        ForEach(model.secureTargets) { target in
                            SettingRow("\(target.host):\(target.port)") {
                                Text("encrypted")
                                    .foregroundStyle(.secondary)
                            }
                        }
                    }
                }
            } label: {
                Label("Available secure targets", systemImage: "lock.open.display")
            }
        }
    }

    private var permissionsPane: some View {
        VStack(alignment: .leading, spacing: 18) {
            GroupBox {
                VStack(alignment: .leading, spacing: 12) {
                    PermissionRow(
                        title: "Screen Recording",
                        value: model.permissions.screenRecording,
                        systemImage: "rectangle.on.rectangle",
                        actionTitle: "Request",
                        action: { model.requestScreenRecording() }
                    )
                    PermissionRow(
                        title: "Accessibility",
                        value: model.permissions.accessibility,
                        systemImage: "keyboard",
                        actionTitle: "Request",
                        action: { model.requestAccessibility() }
                    )
                    PermissionRow(
                        title: "Audio",
                        value: model.permissions.audio,
                        systemImage: "waveform",
                        actionTitle: "Request",
                        action: { model.requestAudioAccess() }
                    )
                    SettingRow("Input Monitoring") {
                        Text(model.permissions.inputMonitoring)
                            .foregroundStyle(.secondary)
                    }
                }
            } label: {
                Label("macOS Privacy", systemImage: "hand.raised")
            }
            Text("Mouse, keyboard, and touch-pointer injection are available on macOS. Windows Ink-style native pen injection remains Windows-specific.")
                .foregroundStyle(.secondary)
        }
    }

    private var clientsPane: some View {
        VStack(alignment: .leading, spacing: 18) {
            GroupBox {
                VStack(alignment: .leading, spacing: 12) {
                    if model.approvedClients.isEmpty {
                        Text("No trusted Android clients yet.")
                            .foregroundStyle(.secondary)
                    } else {
                        ForEach(model.approvedClients) { client in
                            ClientRow(
                                client: client,
                                isSecure: model.secureTargets.contains(client.target)
                            )
                        }
                    }
                }
            } label: {
                Label("Trusted clients", systemImage: "person.crop.rectangle.stack")
            }
            Button(role: .destructive) {
                model.clearTrustedClients()
            } label: {
                Label("Clear Trust", systemImage: "trash")
            }
            .buttonStyle(.bordered)
        }
    }

    private var diagnosticsPane: some View {
        VStack(alignment: .leading, spacing: 18) {
            GroupBox {
                VStack(alignment: .leading, spacing: 12) {
                    Label(model.captureStatus, systemImage: "display")
                    if model.displays.isEmpty {
                        Text("No displays have been enumerated.")
                            .foregroundStyle(.secondary)
                    } else {
                        ForEach(model.displays) { display in
                            Text(display.label)
                                .font(.callout)
                                .foregroundStyle(.secondary)
                        }
                    }
                }
            } label: {
                Label("Displays", systemImage: "rectangle.connected.to.line.below")
            }
            GroupBox {
                VStack(alignment: .leading, spacing: 12) {
                    Label(model.keychainStatus, systemImage: "key")
                    Label(model.discoveryStatus, systemImage: "antenna.radiowaves.left.and.right")
                    Label(model.controlStatus, systemImage: "network")
                    HostActionButton("Encoder Smoke Test", systemImage: "video.badge.checkmark") { model.startEncoderSmokeTest() }
                    HostActionButton("Keychain Smoke Test", systemImage: "key.horizontal") { model.runKeychainSmokeTest() }
                }
            } label: {
                Label("Checks", systemImage: "stethoscope")
            }
        }
    }

    private func refreshAll() {
        model.refreshReadiness()
        Task { await model.refreshDisplays() }
    }
}

private struct HostHeroStatus: View {
    let status: String
    let controlStatus: String
    let discoveryStatus: String

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Label(status, systemImage: status == "Needs permissions" ? "exclamationmark.triangle" : "checkmark.circle")
                .font(.title2.weight(.semibold))
            Text(controlStatus)
                .foregroundStyle(.secondary)
            Text(discoveryStatus)
                .foregroundStyle(.secondary)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(16)
        .background(.regularMaterial, in: RoundedRectangle(cornerRadius: 8))
    }
}

private struct PairingCodePanel: View {
    let code: String

    var body: some View {
        GroupBox {
            HStack {
                Text("Enter this code on Android")
                    .foregroundStyle(.secondary)
                Spacer()
                Text(code)
                    .font(.system(size: 32, weight: .semibold, design: .monospaced))
                    .textSelection(.enabled)
            }
        } label: {
            Label("Pairing Code", systemImage: "number.square")
        }
    }
}

private struct SettingRow<Content: View>: View {
    let title: String
    let content: Content

    init(_ title: String, @ViewBuilder content: () -> Content) {
        self.title = title
        self.content = content()
    }

    var body: some View {
        HStack(alignment: .firstTextBaseline) {
            Text(title)
            Spacer(minLength: 16)
            content
        }
    }
}

private struct PermissionRow: View {
    let title: String
    let value: String
    let systemImage: String
    let actionTitle: String
    let action: () -> Void

    var body: some View {
        HStack {
            Label(title, systemImage: systemImage)
            Spacer()
            Text(value)
                .foregroundStyle(value == "authorized" ? .primary : .secondary)
            Button(actionTitle, action: action)
                .buttonStyle(.bordered)
        }
    }
}

private struct ClientRow: View {
    let client: MacPairingClient
    let isSecure: Bool

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            HStack {
                Label(client.deviceName, systemImage: isSecure ? "lock.shield" : "checkmark.shield")
                Spacer()
                Text(isSecure ? "encrypted" : "trusted")
                    .foregroundStyle(.secondary)
            }
            Text("\(client.target.host):\(client.target.port) · \(client.id) · key \(client.publicKeyStatusLabel)")
                .font(.caption)
                .foregroundStyle(.secondary)
                .textSelection(.enabled)
        }
    }
}

private struct HostActionButton: View {
    let title: String
    let systemImage: String
    let action: () -> Void

    init(_ title: String, systemImage: String, action: @escaping () -> Void) {
        self.title = title
        self.systemImage = systemImage
        self.action = action
    }

    var body: some View {
        Button(action: action) {
            Label(title, systemImage: systemImage)
                .frame(maxWidth: .infinity)
        }
        .buttonStyle(.bordered)
        .controlSize(.large)
    }
}
