use glyphray_windows_host::capture::{ScreenCapture, WindowsGraphicsCaptureBackend};

fn main() {
    let mut capture = WindowsGraphicsCaptureBackend;
    let displays = match capture.list_displays() {
        Ok(displays) => displays,
        Err(err) => {
            eprintln!("Failed to list displays: {err}");
            std::process::exit(1);
        }
    };

    if displays.is_empty() {
        println!("No displays reported by this platform/backend.");
        return;
    }

    for display in &displays {
        println!(
            "Display {}: {} {}x{} at {},{} primary={}",
            display.id,
            display.name,
            display.width_px,
            display.height_px,
            display.origin_x,
            display.origin_y,
            display.primary
        );
    }

    let display_id = displays
        .iter()
        .find(|display| display.primary)
        .map(|display| display.id)
        .unwrap_or(displays[0].id);

    match capture.capture_frame(display_id) {
        Ok(frame) => {
            println!(
                "Captured display {}: {}x{}, {} bytes, crc32={:08x}",
                frame.display_id,
                frame.width,
                frame.height,
                frame.bgra.len(),
                crc32fast::hash(&frame.bgra)
            );
        }
        Err(err) => {
            eprintln!("Failed to capture display {display_id}: {err}");
            std::process::exit(2);
        }
    }
}
