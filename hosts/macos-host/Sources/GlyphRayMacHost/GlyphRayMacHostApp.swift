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
    private let permissionController = MacPermissionController()
    private let keychainStore = KeychainSecretStore()

    @Published var status: String = "Idle"
    @Published var captureStatus: String = "Capture idle"
    @Published var liveCaptureStatus: String = "Live capture idle"
    @Published var encoderStatus: String = "Encoder idle"
    @Published var keychainStatus: String = "Keychain idle"
    @Published var permissions = MacPermissionSnapshot(
        screenRecording: "unknown",
        accessibility: "unknown",
        audio: "unknown",
        inputMonitoring: "manual review"
    )
    @Published var displays: [MacDisplayDescriptor] = []

    init() {
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
            Label(model.encoderStatus, systemImage: "video")
            Label(model.keychainStatus, systemImage: "key")
            Label("Screen Recording: \(model.permissions.screenRecording)", systemImage: "rectangle.on.rectangle")
            Label("Accessibility: \(model.permissions.accessibility)", systemImage: "keyboard")
            Label("Input Monitoring: \(model.permissions.inputMonitoring)", systemImage: "lock")
            Label("Audio: \(model.permissions.audio)", systemImage: "waveform")
            Label("Mouse and keyboard input first; Windows Ink-style pen injection is Windows-specific.", systemImage: "hand.draw")
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
                Button("Encoder Smoke Test") {
                    model.startEncoderSmokeTest()
                }
                Button("Live Capture Probe") {
                    model.startLiveCaptureProbe()
                }
                Button("Live Encode Probe") {
                    model.startLiveEncodeProbe()
                }
                Button("Live Transport Probe") {
                    model.startLiveTransportProbe()
                }
                Button("Keychain Smoke Test") {
                    model.runKeychainSmokeTest()
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
        }
        .padding(24)
        .frame(minWidth: 680, minHeight: 380)
        .task {
            await model.refreshDisplays()
        }
    }
}
