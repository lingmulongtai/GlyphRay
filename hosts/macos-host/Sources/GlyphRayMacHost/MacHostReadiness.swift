import Foundation

#if canImport(ApplicationServices)
import ApplicationServices
#endif

struct MacPermissionSnapshot: Equatable {
    let screenRecording: String
    let accessibility: String
    let audio: String
    let inputMonitoring: String

    var readyForSmokeTest: Bool {
        screenRecording == "authorized" && accessibility == "authorized"
    }
}

final class MacPermissionController {
    private let audioController = AudioCaptureController()

    func snapshot() -> MacPermissionSnapshot {
        MacPermissionSnapshot(
            screenRecording: screenRecordingStatus(),
            accessibility: accessibilityStatus(),
            audio: audioController.authorizationStatusDescription(),
            inputMonitoring: "manual review"
        )
    }

    @discardableResult
    func requestScreenRecordingAccess() -> Bool {
        #if canImport(ApplicationServices)
        if #available(macOS 10.15, *) {
            return CGRequestScreenCaptureAccess()
        }
        return false
        #else
        return false
        #endif
    }

    @discardableResult
    func requestAccessibilityPrompt() -> Bool {
        #if canImport(ApplicationServices)
        let options = [
            kAXTrustedCheckOptionPrompt.takeUnretainedValue() as String: true
        ] as CFDictionary
        return AXIsProcessTrustedWithOptions(options)
        #else
        return false
        #endif
    }

    func requestAudioAccess(completion: @escaping (Bool) -> Void) {
        audioController.requestAuthorization(completion: completion)
    }

    private func screenRecordingStatus() -> String {
        #if canImport(ApplicationServices)
        if #available(macOS 10.15, *) {
            return CGPreflightScreenCaptureAccess() ? "authorized" : "not authorized"
        }
        return "unavailable"
        #else
        return "ApplicationServices unavailable"
        #endif
    }

    private func accessibilityStatus() -> String {
        #if canImport(ApplicationServices)
        return AXIsProcessTrusted() ? "authorized" : "not authorized"
        #else
        return "ApplicationServices unavailable"
        #endif
    }
}
