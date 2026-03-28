use anyhow::{Context, Result};
use image::imageops::{self, FilterType};
use image::{RgbImage, RgbaImage};
use log::warn;
use windows_capture::{
    capture::{Context as CaptureContext, GraphicsCaptureApiHandler},
    frame::Frame,
    graphics_capture_api::{GraphicsCaptureApi, InternalCaptureControl},
    monitor::Monitor,
    settings::{
        ColorFormat, CursorCaptureSettings, DirtyRegionSettings, DrawBorderSettings,
        MinimumUpdateIntervalSettings, SecondaryWindowSettings, Settings,
    },
};

struct SingleFrameHandler {
    captured_image: Option<RgbImage>,
    capture_ratio: f32,
}

impl GraphicsCaptureApiHandler for SingleFrameHandler {
    type Flags = f32;
    type Error = anyhow::Error;

    fn new(ctx: CaptureContext<Self::Flags>) -> Result<Self, Self::Error> {
        // Will adjust depending on how flags are exposed (could be ctx.flags() or just .flags depending on crate vers)
        Ok(Self {
            captured_image: None,
            capture_ratio: ctx.flags,
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

        let base_s = width.min(height) as f32;
        let clamped_ratio = self.capture_ratio.clamp(0.1, 1.0);
        let s = (base_s * clamped_ratio) as u32;

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

/// 从主显示器捕获一帧，根据 capture_ratio 裁剪中心区域正方形，缩放到 640×640，返回 RgbImage。
pub fn capture_frame(capture_ratio: f32) -> Result<RgbImage> {
    let monitor = Monitor::primary().context("Failed to get primary monitor")?;
    let draw_border_settings = if GraphicsCaptureApi::is_border_settings_supported()
        .context("Failed to determine capture border support")?
    {
        DrawBorderSettings::WithoutBorder
    } else {
        warn!(
            "Capture border toggle is not supported on this system; falling back to default border behavior"
        );
        DrawBorderSettings::Default
    };

    let settings = Settings::new(
        monitor,
        CursorCaptureSettings::WithoutCursor,
        draw_border_settings,
        SecondaryWindowSettings::Default,
        MinimumUpdateIntervalSettings::Default,
        DirtyRegionSettings::Default,
        ColorFormat::Rgba8,
        capture_ratio,
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
