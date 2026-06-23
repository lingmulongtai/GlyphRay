import Foundation

#if canImport(CoreGraphics)
import CoreGraphics
#endif

struct RemotePointerEvent {
    let x: Double
    let y: Double
    let wheelDeltaX: Double
    let wheelDeltaY: Double
    let buttonFlags: UInt32
}

final class InputEventController {
    private var previousButtonFlags: UInt32 = 0

    func postMouse(event: RemotePointerEvent) {
        #if canImport(CoreGraphics)
        let point = CGPoint(x: event.x, y: event.y)
        postButtonTransition(mask: 1, button: .left, down: .leftMouseDown, up: .leftMouseUp, at: point, flags: event.buttonFlags)
        postButtonTransition(mask: 2, button: .right, down: .rightMouseDown, up: .rightMouseUp, at: point, flags: event.buttonFlags)
        postButtonTransition(mask: 4, button: .center, down: .otherMouseDown, up: .otherMouseUp, at: point, flags: event.buttonFlags)

        let type: CGEventType
        let button: CGMouseButton
        if event.buttonFlags & 1 != 0 {
            type = .leftMouseDragged
            button = .left
        } else if event.buttonFlags & 2 != 0 {
            type = .rightMouseDragged
            button = .right
        } else {
            type = .mouseMoved
            button = .left
        }
        let cgEvent = CGEvent(
            mouseEventSource: nil,
            mouseType: type,
            mouseCursorPosition: point,
            mouseButton: button
        )
        cgEvent?.post(tap: .cghidEventTap)

        let wheelX = Int32((event.wheelDeltaX * 32).rounded())
        let wheelY = Int32((event.wheelDeltaY * 32).rounded())
        if wheelX != 0 || wheelY != 0 {
            CGEvent(
                scrollWheelEvent2Source: nil,
                units: .pixel,
                wheelCount: 2,
                wheel1: wheelY,
                wheel2: wheelX,
                wheel3: 0
            )?.post(tap: .cghidEventTap)
        }
        previousButtonFlags = event.buttonFlags
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

    func postKeyboard(event: MacRemoteKeyboardInput) -> Bool {
        #if canImport(CoreGraphics)
        guard let keyCode = MacVirtualKeyMapper.keyCode(forWindowsVirtualKey: event.virtualKey) else {
            return false
        }
        let cgEvent = CGEvent(
            keyboardEventSource: nil,
            virtualKey: CGKeyCode(keyCode),
            keyDown: event.pressed
        )
        cgEvent?.flags = MacVirtualKeyMapper.flags(forAndroidModifiers: event.modifiers)
        cgEvent?.post(tap: .cghidEventTap)
        return cgEvent != nil
        #else
        return false
        #endif
    }

    func postTouch(batch: MacRemoteTouchBatch) {
        #if canImport(CoreGraphics)
        guard let primaryPointerID = batch.samples.first?.pointerID else { return }
        for sample in batch.samples where sample.pointerID == primaryPointerID {
            let point = CGPoint(x: Double(sample.x), y: Double(sample.y))
            switch sample.action {
            case 0:
                postMouse(event: RemotePointerEvent(
                    x: point.x,
                    y: point.y,
                    wheelDeltaX: 0,
                    wheelDeltaY: 0,
                    buttonFlags: 0
                ))
                postLeftClick(at: point, down: true)
            case 1:
                CGEvent(
                    mouseEventSource: nil,
                    mouseType: .leftMouseDragged,
                    mouseCursorPosition: point,
                    mouseButton: .left
                )?.post(tap: .cghidEventTap)
            case 2, 3:
                postLeftClick(at: point, down: false)
            default:
                break
            }
        }
        #endif
    }

    #if canImport(CoreGraphics)
    private func postButtonTransition(
        mask: UInt32,
        button: CGMouseButton,
        down: CGEventType,
        up: CGEventType,
        at point: CGPoint,
        flags: UInt32
    ) {
        let wasPressed = previousButtonFlags & mask != 0
        let isPressed = flags & mask != 0
        guard wasPressed != isPressed else { return }
        CGEvent(
            mouseEventSource: nil,
            mouseType: isPressed ? down : up,
            mouseCursorPosition: point,
            mouseButton: button
        )?.post(tap: .cghidEventTap)
    }
    #endif
}

#if canImport(CoreGraphics)
private enum MacVirtualKeyMapper {
    private static let windowsToMac: [UInt32: UInt16] = [
        0x08: 51, 0x09: 48, 0x0D: 36, 0x1B: 53, 0x20: 49,
        0x25: 123, 0x26: 126, 0x27: 124, 0x28: 125,
        0x2E: 117, 0x30: 29, 0x31: 18, 0x32: 19, 0x33: 20,
        0x34: 21, 0x35: 23, 0x36: 22, 0x37: 26, 0x38: 28, 0x39: 25,
        0x41: 0, 0x42: 11, 0x43: 8, 0x44: 2, 0x45: 14, 0x46: 3,
        0x47: 5, 0x48: 4, 0x49: 34, 0x4A: 38, 0x4B: 40, 0x4C: 37,
        0x4D: 46, 0x4E: 45, 0x4F: 31, 0x50: 35, 0x51: 12, 0x52: 15,
        0x53: 1, 0x54: 17, 0x55: 32, 0x56: 9, 0x57: 13, 0x58: 7,
        0x59: 16, 0x5A: 6,
        0x70: 122, 0x71: 120, 0x72: 99, 0x73: 118, 0x74: 96, 0x75: 97,
        0x76: 98, 0x77: 100, 0x78: 101, 0x79: 109, 0x7A: 103, 0x7B: 111
    ]

    static func keyCode(forWindowsVirtualKey virtualKey: UInt32) -> UInt16? {
        windowsToMac[virtualKey]
    }

    static func flags(forAndroidModifiers modifiers: UInt32) -> CGEventFlags {
        var flags: CGEventFlags = []
        if modifiers & 0x0000_00C1 != 0 { flags.insert(.maskShift) }
        if modifiers & 0x0000_0032 != 0 { flags.insert(.maskAlternate) }
        if modifiers & 0x0000_7000 != 0 { flags.insert(.maskControl) }
        if modifiers & 0x0007_0000 != 0 { flags.insert(.maskCommand) }
        return flags
    }
}
#endif
