import Foundation

#if canImport(ScreenCaptureKit)
import ScreenCaptureKit
#endif

final class ScreenCaptureController {
    func availableDisplays() async throws -> [String] {
        #if canImport(ScreenCaptureKit)
        let content = try await SCShareableContent.current
        return content.displays.map { display in
            "Display \(display.displayID) \(display.width)x\(display.height)"
        }
        #else
        return []
        #endif
    }
}

