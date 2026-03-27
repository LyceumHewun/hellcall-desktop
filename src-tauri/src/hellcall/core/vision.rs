use anyhow::{Context, Result};
use image::imageops::{self, FilterType};
use image::{RgbImage, RgbaImage};
use ndarray::Array4;
use ort::session::{builder::GraphOptimizationLevel, Session};
use ort::value::Tensor;
use std::sync::{Mutex, mpsc};
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

// ─────────── YoloEngine ───────────

#[derive(Debug, Clone)]
pub struct Detection {
    pub x: f32,
    pub class_id: usize,
    pub conf: f32,
}

/// 持久化的 YOLO 推理引擎。
/// `Session` 是 Send + Sync，但 `run()` 需要 `&mut self`，因此用 `Mutex` 包装。
pub struct YoloEngine {
    session: Mutex<Session>,
}

// SAFETY: Session is Send + Sync per ort docs (https://github.com/microsoft/onnxruntime/issues/114).
unsafe impl Send for YoloEngine {}
unsafe impl Sync for YoloEngine {}

impl YoloEngine {
    /// 从 `.onnx` 文件路径加载模型。
    pub fn new(model_path: &str) -> Result<Self> {
        let mut builder = Session::builder()
            .context("Failed to create ort session builder")?
            .with_optimization_level(GraphOptimizationLevel::Level3)
            .map_err(|e| anyhow::anyhow!("Failed to set optimization level: {}", e))?;

        let session = builder
            .commit_from_file(model_path)
            .context("Failed to load ONNX model")?;
        Ok(Self { session: Mutex::new(session) })
    }

    /// 对已调整大小的 640×640 RGB 图像执行推理，并记录输出张量形状。
    pub fn infer(&self, img: RgbImage) -> Result<()> {
        let tensor = preprocess_image(img);

        // Tensor::from_array takes an owned Array4 via the OwnedTensorArrayData trait.
        let input = Tensor::from_array(tensor)
            .context("Failed to create Tensor from ndarray")?;

        let mut session = self
            .session
            .lock()
            .map_err(|_| anyhow::anyhow!("Session mutex poisoned"))?;

        let result = session
            .run(ort::inputs![input])
            .context("ONNX session run failed")?;

        // Phase 3: 解析预测结果 (1x300x6)
        let conf_threshold = 0.50;
        
        for (name, output) in result.iter() {
            if let Ok((shape, data)) = output.try_extract_tensor::<f32>() {
                // Expected shape: [1, 300, 6]
                if shape.len() == 3 && shape[1] == 300 && shape[2] == 6 {
                    let mut detections = Vec::new();
                    
                    for chunk in data.chunks_exact(6) {
                        let conf = chunk[4];
                        if conf > conf_threshold {
                            detections.push(Detection {
                                x: chunk[0], // cx or xmin
                                class_id: chunk[5] as usize,
                                conf,
                            });
                        }
                    }

                    // Sort left-to-right (by x coordinate)
                    detections.sort_by(|a, b| a.x.partial_cmp(&b.x).unwrap_or(std::cmp::Ordering::Equal));

                    // Map to stratagem sequence
                    let classes = ["UP", "DOWN", "LEFT", "RIGHT"];
                    let mut sequence = Vec::new();
                    for d in &detections {
                        let class_name = classes.get(d.class_id).unwrap_or(&"UNKNOWN");
                        sequence.push(*class_name);
                    }
                    
                    if !sequence.is_empty() {
                        log::info!("Vision sequence: {:?}", sequence);
                    } else {
                        log::info!("No detections above confidence: {}", conf_threshold);
                    }
                } else {
                    log::info!("YOLO output '{}' shape unsupported for parsing: {:?}", name, shape);
                }
            }
        }

        Ok(())
    }
}

/// HWC u8 [640, 640, 3] → CHW f32 [1, 3, 640, 640] 归一化至 0..=1。
pub fn preprocess_image(img: RgbImage) -> Array4<f32> {
    let (w, h) = img.dimensions();
    debug_assert_eq!(w, 640);
    debug_assert_eq!(h, 640);

    // Array4 shape: [batch, channel, height, width]
    let mut tensor = Array4::<f32>::zeros([1, 3, h as usize, w as usize]);

    for (x, y, pixel) in img.enumerate_pixels() {
        let [r, g, b] = pixel.0;
        tensor[[0, 0, y as usize, x as usize]] = r as f32 / 255.0;
        tensor[[0, 1, y as usize, x as usize]] = g as f32 / 255.0;
        tensor[[0, 2, y as usize, x as usize]] = b as f32 / 255.0;
    }

    tensor
}

// ─────────── Screen capture helpers ───────────

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
