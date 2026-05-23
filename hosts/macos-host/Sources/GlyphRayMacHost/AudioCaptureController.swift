import Foundation

#if canImport(AVFoundation)
import AVFoundation
#endif

final class AudioCaptureController {
    func authorizationStatusDescription() -> String {
        #if canImport(AVFoundation)
        switch AVCaptureDevice.authorizationStatus(for: .audio) {
        case .authorized:
            return "authorized"
        case .denied:
            return "denied"
        case .restricted:
            return "restricted"
        case .notDetermined:
            return "not determined"
        @unknown default:
            return "unknown"
        }
        #else
        return "AVFoundation unavailable"
        #endif
    }

    func requestAuthorization(completion: @escaping (Bool) -> Void) {
        #if canImport(AVFoundation)
        AVCaptureDevice.requestAccess(for: .audio) { granted in
            completion(granted)
        }
        #else
        completion(false)
        #endif
    }
}
