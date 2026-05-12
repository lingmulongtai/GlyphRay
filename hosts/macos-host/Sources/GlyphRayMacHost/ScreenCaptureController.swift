import Foundation

#if canImport(ScreenCaptureKit)
import ScreenCaptureKit
#endif

struct MacDisplayDescriptor: Identifiable, Equatable {
    let id: UInt32
    let width: Int
    let height: Int
    let originX: Int
    let originY: Int

    var label: String {
        "Display \(id) \(width)x\(height) @ \(originX),\(originY)"
    }
}

final class ScreenCaptureController {
    func availableDisplays() async throws -> [MacDisplayDescriptor] {
        #if canImport(ScreenCaptureKit)
        let content = try await SCShareableContent.current
        return content.displays.map { display in
            MacDisplayDescriptor(
                id: display.displayID,
                width: display.width,
                height: display.height,
                originX: Int(display.frame.origin.x),
                originY: Int(display.frame.origin.y)
            )
        }
        #else
        return []
        #endif
    }
}
