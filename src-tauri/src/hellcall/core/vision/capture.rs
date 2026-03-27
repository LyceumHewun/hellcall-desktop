use anyhow::{Context, Result};
use image::imageops::{self, FilterType};
use image::{RgbImage, RgbaImage};
use std::sync::mpsc;
use windows_capture::{
    capture::{Context as CaptureContext, GraphicsCaptureApiHandler},
    frame::Frame,
    graphics_capture_api::InternalCaptureControl,
    monitor::Monitor,
    settings::{
        ColorFormat, CursorCaptureSettings, DirtyRegionSettings, DrawBorderSettings,
        MinimumUpdateIntervalSettings, SecondaryWindowSettings, Settings,
    },
};

/// 从主显示器捕获一帧，裁剪中心 1:1 正方形，缩放到 640×640，返回 RgbImage。
pub fn capture_frame() -> Result<RgbImage> {
    let (tx, rx) = mpsc::sync_channel::<(Vec<u8>, u32, u32)>(1);

    let monitor = Monitor::primary().context("Failed to get primary monitor")?;
    let settings = Settings::new(
        monitor,
        CursorCaptureSettings::WithoutCursor,
        DrawBorderSettings::WithoutBorder,
        SecondaryWindowSettings::Default,
        MinimumUpdateIntervalSettings::Default,
        DirtyRegionSettings::Default,
        ColorFormat::Bgra8,
        tx,
    );

    let capture_thread = std::thread::spawn(move || SingleFrameHandler::start(settings));

    let (bgra_buf, width, height) = rx.recv().context("Frame channel closed unexpectedly")?;
    let _ = capture_thread.join();

    process_frame(&bgra_buf, width, height)
}

/// BGRA buffer → center 1:1 crop → 640×640 RgbImage。
pub fn process_frame(bgra_buf: &[u8], width: u32, height: u32) -> Result<RgbImage> {
    // BGRA → RGBA (swap B and R)
    let mut rgba_buf = bgra_buf.to_vec();
    for chunk in rgba_buf.chunks_exact_mut(4) {
        chunk.swap(0, 2);
    }

    let rgba = RgbaImage::from_raw(width, height, rgba_buf)
        .context("Failed to build RgbaImage from frame buffer")?;

    let s = width.min(height);
    let start_x = (width - s) / 2;
    let start_y = (height - s) / 2;
    let cropped = imageops::crop_imm(&rgba, start_x, start_y, s, s).to_image();

    let resized_rgba = imageops::resize(&cropped, 640, 640, FilterType::Triangle);

    // Drop alpha channel → RGB
    Ok(image::DynamicImage::ImageRgba8(resized_rgba).into_rgb8())
}

/// 调试用：捕获截图并保存到磁盘（Phase 1 兼容）。
#[allow(dead_code)]
pub fn capture_and_save() -> Result<()> {
    let img = capture_frame()?;
    img.save("debug_capture.png")
        .context("Failed to save debug_capture.png")?;
    Ok(())
}

// ─────────── windows-capture 1.5.0 handler ───────────

type FrameSender = mpsc::SyncSender<(Vec<u8>, u32, u32)>;

struct SingleFrameHandler {
    tx: FrameSender,
    done: bool,
}

impl GraphicsCaptureApiHandler for SingleFrameHandler {
    type Flags = FrameSender;
    type Error = anyhow::Error;

    fn new(ctx: CaptureContext<Self::Flags>) -> Result<Self, Self::Error> {
        Ok(Self { tx: ctx.flags, done: false })
    }

    fn on_frame_arrived(
        &mut self,
        frame: &mut Frame,
        capture_control: InternalCaptureControl,
    ) -> Result<(), Self::Error> {
        if self.done {
            return Ok(());
        }
        self.done = true;

        let width = frame.width();
        let height = frame.height();
        let mut buffer = frame.buffer().context("Failed to get frame buffer")?;
        let raw = buffer.as_raw_buffer();
        let _ = self.tx.try_send((raw.to_vec(), width, height));

        capture_control.stop();
        Ok(())
    }

    fn on_closed(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}
