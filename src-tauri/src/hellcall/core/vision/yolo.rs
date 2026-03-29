use super::corrector::Detection;
use anyhow::{Context, Result};
use image::RgbImage;
use ndarray::Array4;
use ort::ep::ExecutionProvider;
use ort::session::{builder::GraphOptimizationLevel, Session};
use ort::value::TensorRef;
use rayon::prelude::*;
use std::sync::Mutex;

/// 持久化的 YOLO 推理引擎。
/// `Session` 是 Send + Sync，但 `run()` 需要 `&mut self`，因此用 `Mutex` 包装。
pub struct YoloEngine {
    session: Mutex<Session>,
    input_buffer: Mutex<Array4<f32>>,
}

// SAFETY: Session is Send + Sync per ort docs (https://github.com/microsoft/onnxruntime/issues/114).
unsafe impl Send for YoloEngine {}
unsafe impl Sync for YoloEngine {}

impl YoloEngine {
    /// 从 `.onnx` 文件路径加载模型。
    pub fn new(model_path: &str) -> Result<Self> {
        let cuda = ort::ep::CUDA::default();
        let mut builder = Session::builder()
            .context("Failed to create ort session builder")?
            .with_optimization_level(GraphOptimizationLevel::Level3)
            .map_err(|e| anyhow::anyhow!("Failed to set optimization level: {}", e))?;

        let backend_log = match cuda.is_available() {
            Ok(true) => match cuda.register(&mut builder) {
                Ok(()) => {
                    "CUDA execution provider registered successfully for the YOLO session."
                }
                Err(e) => {
                    log::warn!(
                        "CUDA execution provider is available but failed to register: {}. YOLO will fall back to CPU.",
                        e
                    );
                    "CUDA execution provider registration failed; YOLO will use CPU fallback."
                }
            },
            Ok(false) => {
                "CUDA execution provider is unavailable in the current ONNX Runtime environment; YOLO will use CPU fallback."
            }
            Err(e) => {
                log::warn!(
                    "Failed to query CUDA execution provider availability: {}. YOLO will continue without explicitly enabling CUDA.",
                    e
                );
                "CUDA execution provider availability check failed; YOLO will use CPU fallback."
            }
        };

        let mut session = builder
            .commit_from_file(model_path)
            .context("Failed to load ONNX model")?;

        let input_buffer = ndarray::Array4::<f32>::zeros((1, 3, 640, 640));
        let input = TensorRef::from_array_view(input_buffer.view())
            .context("Failed to create TensorRef for warmup")?;
        let _ = session
            .run(ort::inputs![input])
            .context("Warm-up inference failed")?;
        log::info!("YOLO engine warm-up inference completed.");
        log::info!("{}", backend_log);

        Ok(Self {
            session: Mutex::new(session),
            input_buffer: Mutex::new(input_buffer),
        })
    }

    /// 对已调整大小的 640×640 RGB 图像执行推理，并返回解析后的 Detection 列表。
    pub fn infer(&self, img: RgbImage) -> Result<Vec<Detection>> {
        let mut input_buffer = self
            .input_buffer
            .lock()
            .map_err(|_| anyhow::anyhow!("Input buffer mutex poisoned"))?;
        preprocess_image_into(&img, &mut input_buffer);
        let input = TensorRef::from_array_view(input_buffer.view())
            .context("Failed to create TensorRef from ndarray view")?;

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

fn preprocess_image_into(img: &RgbImage, tensor: &mut Array4<f32>) {
    let (w, h) = img.dimensions();
    debug_assert_eq!(w, 640);
    debug_assert_eq!(h, 640);
    debug_assert_eq!(tensor.shape(), &[1, 3, h as usize, w as usize]);

    let width = w as usize;
    let height = h as usize;
    let plane_len = width * height;
    let row_stride = width * 3;
    let scale = 1.0 / 255.0;

    let raw = img.as_raw();
    let data = tensor
        .as_slice_mut()
        .expect("YOLO input buffer must be contiguous in memory");
    let (r_plane, gb_plane) = data.split_at_mut(plane_len);
    let (g_plane, b_plane) = gb_plane.split_at_mut(plane_len);

    raw.par_chunks_exact(row_stride)
        .zip(r_plane.par_chunks_mut(width))
        .zip(g_plane.par_chunks_mut(width))
        .zip(b_plane.par_chunks_mut(width))
        .for_each(|(((src_row, r_row), g_row), b_row)| {
            src_row.chunks_exact(3).enumerate().for_each(|(x, rgb)| {
                r_row[x] = rgb[0] as f32 * scale;
                g_row[x] = rgb[1] as f32 * scale;
                b_row[x] = rgb[2] as f32 * scale;
            });
        });
}
