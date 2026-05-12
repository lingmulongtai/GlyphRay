import Foundation

#if canImport(CoreGraphics)
import CoreGraphics
#endif

struct RemotePointerEvent {
    let x: Double
    let y: Double
    let pressed: Bool
}

struct RemoteKeyboardEvent {
    let keyCode: UInt16
    let pressed: Bool
    let flags: CGEventFlags

    init(keyCode: UInt16, pressed: Bool, flags: CGEventFlags = []) {
        self.keyCode = keyCode
        self.pressed = pressed
        self.flags = flags
    }
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

    func postLeftClick(at point: CGPoint, down: Bool) {
        #if canImport(CoreGraphics)
        let type: CGEventType = down ? .leftMouseDown : .leftMouseUp
        let cgEvent = CGEvent(
            mouseEventSource: nil,
            mouseType: type,
            mouseCursorPosition: point,
            mouseButton: .left
        )
        cgEvent?.post(tap: .cghidEventTap)
        #endif
    }

    func postKeyboard(event: RemoteKeyboardEvent) {
        #if canImport(CoreGraphics)
        let cgEvent = CGEvent(
            keyboardEventSource: nil,
            virtualKey: CGKeyCode(event.keyCode),
            keyDown: event.pressed
        )
        cgEvent?.flags = event.flags
        cgEvent?.post(tap: .cghidEventTap)
        #endif
    }
}
