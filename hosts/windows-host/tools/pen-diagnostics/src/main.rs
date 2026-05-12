use glyphray_core::{CoordinateMapper, DisplayRect, MappingMode, PressureMapper, SourceRect};
use glyphray_protocol::{StylusAction, StylusInputBatch, StylusSample, StylusToolType};
use glyphray_windows_host::input::create_pen_injector;

fn main() {
    let mapper = CoordinateMapper::new(
        SourceRect::new(1600.0, 1000.0).expect("source"),
        DisplayRect::new(0.0, 0.0, 1920.0, 1080.0, 0, 1.0).expect("display"),
        MappingMode::Fit,
    );
    let pressure = PressureMapper::default();
    let batch = diagnostic_stroke();

    match create_pen_injector() {
        Ok(mut injector) => match injector.inject_batch(&batch, &mapper, &pressure) {
            Ok(report) => {
                println!(
                    "Injected {} synthetic pen samples using native path: {}",
                    report.injected_samples, report.used_pen_path
                );
            }
            Err(err) => {
                eprintln!("Pen injection failed: {err}");
                std::process::exit(2);
            }
        },
        Err(err) => {
            eprintln!("Pen injector unavailable: {err}");
            std::process::exit(1);
        }
    }
}

fn diagnostic_stroke() -> StylusInputBatch {
    let mut samples = Vec::new();
    samples.push(sample(0, StylusAction::HoverMove, 100.0, 100.0, 0.0, true));
    samples.push(sample(1, StylusAction::Down, 120.0, 120.0, 0.15, false));
    for index in 2..18 {
        let t = index as f32 / 17.0;
        samples.push(sample(
            index,
            StylusAction::Move,
            120.0 + 900.0 * t,
            120.0 + 420.0 * t,
            0.2 + 0.65 * t,
            false,
        ));
    }
    samples.push(sample(18, StylusAction::Up, 1040.0, 560.0, 0.0, false));

    StylusInputBatch {
        batch_sequence: 1,
        monotonic_timestamp_us: 0,
        samples,
    }
}

fn sample(
    sequence: u64,
    action: StylusAction,
    x: f32,
    y: f32,
    pressure: f32,
    hover: bool,
) -> StylusSample {
    StylusSample {
        sequence,
        timestamp_us: sequence * 1_000,
        display_id: 0,
        pointer_id: 1,
        tool_type: StylusToolType::Stylus,
        action,
        x,
        y,
        pressure,
        tilt_x_degrees: 10.0,
        tilt_y_degrees: -18.0,
        orientation_degrees: 35.0,
        button_flags: 0,
        hover,
        eraser: false,
        predicted: false,
    }
}
