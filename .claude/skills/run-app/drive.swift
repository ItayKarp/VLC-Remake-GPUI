// Drives the running app like a person: moves, clicks and keys, posted as Quartz events.
//
//   drive click X Y          left-click at screen point (X, Y)
//   drive hover X Y          move the pointer there without clicking (opens submenus)
//   drive drag X1 Y1 X2 Y2   press at (X1, Y1), drag to (X2, Y2) and release
//   drive key CODE [cmd]     press a virtual key code, optionally with Command held
//
// Points are macOS screen points (a Retina capture's pixels divided by its scale).
import CoreGraphics
import Foundation

let args = CommandLine.arguments
let source = CGEventSource(stateID: .hidSystemState)

func post(_ event: CGEvent?) {
    event?.post(tap: .cghidEventTap)
}

func point() -> CGPoint {
    guard args.count >= 4, let x = Double(args[2]), let y = Double(args[3]) else {
        fatalError("usage: drive click|hover X Y")
    }
    return CGPoint(x: x, y: y)
}

func mouse(_ type: CGEventType, _ at: CGPoint) {
    post(CGEvent(mouseEventSource: source, mouseType: type, mouseCursorPosition: at, mouseButton: .left))
}

switch args.count > 1 ? args[1] : "" {
case "hover":
    mouse(.mouseMoved, point())
case "click":
    let at = point()
    mouse(.mouseMoved, at)
    usleep(150_000)
    mouse(.leftMouseDown, at)
    usleep(60_000)
    mouse(.leftMouseUp, at)
case "drag":
    guard args.count >= 6, let x = Double(args[4]), let y = Double(args[5]) else {
        fatalError("usage: drive drag X1 Y1 X2 Y2")
    }
    let (from, to) = (point(), CGPoint(x: x, y: y))
    mouse(.mouseMoved, from)
    usleep(150_000)
    mouse(.leftMouseDown, from)
    // Moved in steps, so the app sees a drag start before the drop.
    for step in 1...20 {
        let t = Double(step) / 20
        let at = CGPoint(x: from.x + (to.x - from.x) * t, y: from.y + (to.y - from.y) * t)
        mouse(.leftMouseDragged, at)
        usleep(25_000)
    }
    usleep(150_000)
    mouse(.leftMouseUp, to)
case "key":
    guard args.count >= 3, let code = CGKeyCode(args[2]) else { fatalError("usage: drive key CODE [cmd]") }
    let flags: CGEventFlags = args.count > 3 && args[3] == "cmd" ? .maskCommand : []
    for down in [true, false] {
        let event = CGEvent(keyboardEventSource: source, virtualKey: code, keyDown: down)
        event?.flags = flags
        post(event)
        usleep(40_000)
    }
default:
    fatalError("usage: drive click|hover X Y | drive key CODE [cmd]")
}
