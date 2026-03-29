use anyhow::{Context, Result};
use image::imageops::{self, FilterType};
use image::{Rgb, RgbImage, RgbaImage};
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

const MODEL_IMAGE_SIZE: u32 = 640;

struct SingleFrameHandler {
    captured_image: Option<RgbImage>,
    capture_ratio: f32,
}

impl GraphicsCaptureApiHandler for SingleFrameHandler {
    type Flags = f32;
    type Error = anyhow::Error;

    fn new(ctx: CaptureContext<Self::Flags>) -> Result<Self, Self::Error> {
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

        let (start_x, start_y, crop_size) =
            center_crop_square(frame.width(), frame.height(), self.capture_ratio);

        let mut buffer = frame
            .buffer_crop(start_x, start_y, start_x + crop_size, start_y + crop_size)
            .context("Failed to get cropped frame buffer")?;
        let raw = buffer
            .as_nopadding_buffer()
            .context("Failed to get nopadding buffer")?;

        let rgba = RgbaImage::from_raw(crop_size, crop_size, raw.to_vec())
            .context("Failed to build cropped RgbaImage from frame buffer")?;
        let resized_rgba = imageops::resize(
            &rgba,
            MODEL_IMAGE_SIZE,
            MODEL_IMAGE_SIZE,
            FilterType::Triangle,
        );
        self.captured_image = Some(rgba_to_rgb(&resized_rgba));

        capture_control.stop();
        Ok(())
    }

    fn on_closed(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

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

fn center_crop_square(width: u32, height: u32, capture_ratio: f32) -> (u32, u32, u32) {
    let base_s = width.min(height) as f32;
    let clamped_ratio = capture_ratio.clamp(0.1, 1.0);
    let crop_size = ((base_s * clamped_ratio).round() as u32).clamp(1, width.min(height));
    let start_x = (width - crop_size) / 2;
    let start_y = (height - crop_size) / 2;
    (start_x, start_y, crop_size)
}

fn rgba_to_rgb(image: &RgbaImage) -> RgbImage {
    let (width, height) = image.dimensions();
    let mut rgb = RgbImage::new(width, height);

    for (src, dst) in image.pixels().zip(rgb.pixels_mut()) {
        let [r, g, b, _] = src.0;
        *dst = Rgb([r, g, b]);
    }

    rgb
}
