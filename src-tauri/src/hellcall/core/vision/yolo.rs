use super::corrector::Detection;
use anyhow::{Context, Result};
use image::RgbImage;
use ndarray::Array4;
use ort::session::{builder::GraphOptimizationLevel, Session};
use ort::value::Tensor;
use std::sync::Mutex;

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
            .map_err(|e| anyhow::anyhow!("Failed to set optimization level: {}", e))?
            .with_execution_providers([ort::ep::CUDA::default().build()])
            .map_err(|e| anyhow::anyhow!("Failed to configure execution providers: {}", e))?;

        let mut session = builder
            .commit_from_file(model_path)
            .context("Failed to load ONNX model")?;

        let dummy_tensor = ndarray::Array4::<f32>::zeros((1, 3, 640, 640));
        let input =
            Tensor::from_array(dummy_tensor).context("Failed to create Tensor for warmup")?;
        let _ = session
            .run(ort::inputs![input])
            .context("Warm-up inference failed")?;
        log::info!("CUDA/YOLO Engine warmed up and locked into VRAM.");

        log::info!("YOLO Engine created, attempted to use CUDA execution provider.");

        Ok(Self {
            session: Mutex::new(session),
        })
    }

    /// 对已调整大小的 640×640 RGB 图像执行推理，并返回解析后的 Detection 列表。
    pub fn infer(&self, img: RgbImage) -> Result<Vec<Detection>> {
        let tensor = preprocess_image(img);

        // Tensor::from_array takes an owned Array4 via the OwnedTensorArrayData trait.
        let input = Tensor::from_array(tensor).context("Failed to create Tensor from ndarray")?;

        let mut session = self
            .session
            .lock()
            .map_err(|_| anyhow::anyhow!("Session mutex poisoned"))?;

        let result = session
            .run(ort::inputs![input])
            .context("ONNX session run failed")?;

        // Phase 3: 解析预测结果 (1x300x6)
        let conf_threshold = 0.25;
        let mut all_detections = Vec::new();

        for (name, output) in result.iter() {
            if let Ok((shape, data)) = output.try_extract_tensor::<f32>() {
                if shape.len() == 3 && shape[1] == 300 && shape[2] == 6 {
                    for chunk in data.chunks_exact(6) {
                        let conf = chunk[4];
                        if conf >= conf_threshold {
                            all_detections.push(Detection {
                                x_center: chunk[0],
                                y_center: chunk[1],
                                width: chunk[2],
                                height: chunk[3],
                                confidence: conf,
                                class_id: chunk[5] as usize,
                            });
                        }
                    }
                } else {
                    anyhow::bail!(
                        "YOLO output '{}' shape unsupported for parsing: {:?}",
                        name,
                        shape
                    );
                }
            } else {
                anyhow::bail!("Failed to extract f32 tensor from YOLO output '{}'", name);
            }
        }

        Ok(all_detections)
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
