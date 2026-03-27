#[derive(Debug, Clone)]
pub struct Detection {
    pub x_center: f32,
    pub y_center: f32,
    pub width: f32,
    pub height: f32,
    pub confidence: f32,
    pub class_id: usize, // e.g., 0: UP, 1: DOWN, 2: LEFT, 3: RIGHT
}

fn iou(a: &Detection, b: &Detection) -> f32 {
    let a_xmin = a.x_center - a.width / 2.0;
    let a_ymin = a.y_center - a.height / 2.0;
    let a_xmax = a.x_center + a.width / 2.0;
    let a_ymax = a.y_center + a.height / 2.0;

    let b_xmin = b.x_center - b.width / 2.0;
    let b_ymin = b.y_center - b.height / 2.0;
    let b_xmax = b.x_center + b.width / 2.0;
    let b_ymax = b.y_center + b.height / 2.0;

    let inter_xmin = a_xmin.max(b_xmin);
    let inter_ymin = a_ymin.max(b_ymin);
    let inter_xmax = a_xmax.min(b_xmax);
    let inter_ymax = a_ymax.min(b_ymax);

    let inter_width = (inter_xmax - inter_xmin).max(0.0);
    let inter_height = (inter_ymax - inter_ymin).max(0.0);
    let inter_area = inter_width * inter_height;

    let a_area = a.width * a.height;
    let b_area = b.width * b.height;

    let union_area = a_area + b_area - inter_area;

    if union_area > 0.0 {
        inter_area / union_area
    } else {
        0.0
    }
}

pub fn correct_arrows(mut raw_detections: Vec<Detection>) -> Vec<String> {
    // Step 1: Initial Confidence Filter
    raw_detections.retain(|d| d.confidence >= 0.25);

    let raw_len = raw_detections.len();

    // Step 2: NMS (Non-Maximum Suppression) for High Overlap
    raw_detections.sort_by(|a, b| {
        b.confidence
            .partial_cmp(&a.confidence)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut kept_detections: Vec<Detection> = Vec::new();
    for det in raw_detections {
        let mut overlap = false;
        for kept in &kept_detections {
            if iou(&det, kept) > 0.90 {
                overlap = true;
                break;
            }
        }
        if !overlap {
            kept_detections.push(det);
        } else {
            log::debug!("Dropped due to NMS IoU: {:?}", det.class_id);
        }
    }

    let nms_len = kept_detections.len();

    // Step 3: Collinearity Check (Median Y Alignment)
    if kept_detections.is_empty() {
        log::debug!(
            "Corrector Funnel: Raw({}), After NMS({}), After Y-Align(0), Final: []",
            raw_len,
            nms_len
        );
        return Vec::new();
    }

    let mut y_centers: Vec<f32> = kept_detections.iter().map(|d| d.y_center).collect();
    y_centers.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    let median_y = y_centers[y_centers.len() / 2];

    let mut heights: Vec<f32> = kept_detections.iter().map(|d| d.height).collect();
    heights.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let median_height = heights[heights.len() / 2];

    let tolerance = median_height * 0.9; // 90% of median height as tolerance

    kept_detections.retain(|d| {
        let keep = (d.y_center - median_y).abs() <= tolerance;
        if !keep {
            log::debug!("Dropped due to Y-Alignment (Outlier): {:?}", d.class_id);
        }
        keep
    });

    let align_len = kept_detections.len();

    // Step 4: Left-to-Right Sort and Map
    kept_detections.sort_by(|a, b| {
        a.x_center
            .partial_cmp(&b.x_center)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let final_sequence: Vec<String> = kept_detections
        .into_iter()
        .map(|d| match d.class_id {
            0 => "UP".to_string(),
            1 => "DOWN".to_string(),
            2 => "LEFT".to_string(),
            3 => "RIGHT".to_string(),
            _ => "UNKNOWN".to_string(),
        })
        .collect();

    log::debug!(
        "Corrector Funnel: Raw({}), After NMS({}), After Y-Align({}), Final: {:?}",
        raw_len,
        nms_len,
        align_len,
        final_sequence
    );

    final_sequence
}
