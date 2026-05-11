import Foundation

#if canImport(CoreGraphics)
import CoreGraphics
#endif

struct RemotePointerEvent {
    let x: Double
    let y: Double
    let pressed: Bool
}

final class InputEventController {
    func postMouse(event: RemotePointerEvent) {
        #if canImport(CoreGraphics)
        let type: CGEventType = event.pressed ? .leftMouseDragged : .mouseMoved
        let cgEvent = CGEvent(
            mouseEventSource: nil,
            mouseType: type,
            mouseCursorPosition: CGPoint(x: event.x, y: event.y),
            mouseButton: .left
        )
        cgEvent?.post(tap: .cghidEventTap)
        #endif
    }
}

