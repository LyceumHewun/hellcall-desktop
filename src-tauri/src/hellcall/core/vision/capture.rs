use anyhow::{Context, Result};
use image::imageops::{self, FilterType};
use image::{RgbImage, RgbaImage};
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

struct SingleFrameHandler {
    captured_image: Option<RgbImage>,
}

impl GraphicsCaptureApiHandler for SingleFrameHandler {
    type Flags = ();
    type Error = anyhow::Error;

    fn new(_ctx: CaptureContext<Self::Flags>) -> Result<Self, Self::Error> {
        Ok(Self {
            captured_image: None,
        })
    }

    fn on_frame_arrived(
        &mut self,
        frame: &mut Frame,
        capture_control: InternalCaptureControl,
    ) -> Result<(), Self::Error> {
        if self.captured_image.is_some() {
            return Ok(());
        }

        let width = frame.width();
        let height = frame.height();

        let mut buffer = frame.buffer().context("Failed to get frame buffer")?;
        let raw = buffer
            .as_nopadding_buffer()
            .context("Failed to get nopadding buffer")?;

        let rgba = RgbaImage::from_raw(width, height, raw.to_vec())
            .context("Failed to build RgbaImage from frame buffer")?;

        let s = width.min(height);
        let start_x = (width - s) / 2;
        let start_y = (height - s) / 2;
        let cropped = imageops::crop_imm(&rgba, start_x, start_y, s, s).to_image();

        let resized_rgba = imageops::resize(&cropped, 640, 640, FilterType::Triangle);

        // Drop alpha channel → RGB
        self.captured_image = Some(image::DynamicImage::ImageRgba8(resized_rgba).into_rgb8());

        capture_control.stop();
        Ok(())
    }

    fn on_closed(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

/// 从主显示器捕获一帧，裁剪中心 1:1 正方形，缩放到 640×640，返回 RgbImage。
pub fn capture_frame() -> Result<RgbImage> {
    let monitor = Monitor::primary().context("Failed to get primary monitor")?;
    let settings = Settings::new(
        monitor,
        CursorCaptureSettings::WithoutCursor,
        DrawBorderSettings::WithoutBorder,
        SecondaryWindowSettings::Default,
        MinimumUpdateIntervalSettings::Default,
        DirtyRegionSettings::Default,
        ColorFormat::Rgba8,
        (),
    );

    let control = SingleFrameHandler::start_free_threaded(settings)
        .map_err(|e| anyhow::anyhow!("Capture failed: {}", e))?;

    let callback = control.callback();

    control
        .wait()
        .map_err(|e| anyhow::anyhow!("Control wait failed: {}", e))?;

    let mut handler = callback.lock();
    let img = handler.captured_image.take().context("No frame captured")?;
    Ok(img)
}
