pub mod capture;
pub mod corrector;
pub mod yolo;

pub use capture::*;
pub use corrector::*;
pub use yolo::*;

use anyhow::Result;

/// The master orchestrator for Vision-based Stratagem Recognition.
/// 1. Triggers screen capture.
/// 2. Feeds the cropped 640x640 frame into YOLO.
/// 3. Corrects and filters the raw bounding boxes.
/// 4. Returns the final valid command sequence (e.g., ["UP", "DOWN", ...]).
pub fn recognize_console_arrows(yolo_engine: &YoloEngine) -> Result<Vec<String>> {
    // Step 1: Trigger single-frame screen capture
    let rgb_image = capture::capture_frame()?;

    // Step 2 & 3: Run YOLO Inference
    let raw_detections = yolo_engine.infer(rgb_image)?;

    // Step 4: Run Post-Processing Corrector
    let final_sequence = corrector::correct_arrows(raw_detections);

    Ok(final_sequence)
}
