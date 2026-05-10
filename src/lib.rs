mod align;
mod midfreq;

use image::RgbImage;
use rayon::prelude::*;
use std::path::Path;

pub use align::{AlignmentResult, align_images, normalize_histogram};
pub use midfreq::{
    BLOCK_SIZE, combined_prn, compute_texture_weight, dct_2d, generate_prng_sequence,
    get_mid_freq_positions, idct_2d,
};

/// Watermark detection result
#[derive(Debug)]
pub struct DetectionResult {
    pub detected: bool,
    pub score: f64,
    pub threshold: f64,
    pub alignment: AlignmentResult,
}

/// Crop image to multiples of BLOCK_SIZE
fn crop_to_multiple(image: RgbImage) -> RgbImage {
    let (w, h) = image.dimensions();
    let new_w = w - w % BLOCK_SIZE as u32;
    let new_h = h - h % BLOCK_SIZE as u32;
    if new_w == w && new_h == h {
        return image;
    }
    let mut result = RgbImage::new(new_w, new_h);
    for y in 0..new_h {
        for x in 0..new_w {
            result.put_pixel(x, y, *image.get_pixel(x, y));
        }
    }
    result
}

/// Return center region blocks (ratio: 0.0-1.0, e.g. 0.5 = center 50%)
fn center_blocks_region(blocks_x: usize, blocks_y: usize, ratio: f64) -> Vec<(usize, usize)> {
    let margin_x = (blocks_x as f64 * (1.0 - ratio) / 2.0).round() as usize;
    let margin_y = (blocks_y as f64 * (1.0 - ratio) / 2.0).round() as usize;
    let start_x = margin_x.min(blocks_x / 2);
    let start_y = margin_y.min(blocks_y / 2);
    let end_x = blocks_x - start_x;
    let end_y = blocks_y - start_y;
    let mut blocks = Vec::new();
    for by in start_y..end_y {
        for bx in start_x..end_x {
            blocks.push((by, bx));
        }
    }
    blocks
}

/// Embed watermark into image using mid-frequency spread spectrum
pub fn embed_watermark(image: &RgbImage, key: &str, strength: f64) -> (RgbImage, f64) {
    let image = crop_to_multiple(image.clone());
    let (w, h) = image.dimensions();
    let blocks_x = w as usize / BLOCK_SIZE;
    let blocks_y = h as usize / BLOCK_SIZE;

    let y_channel = rgb_to_y_channel(&image);

    let mid_freq = get_mid_freq_positions();

    let total_mid = blocks_x * blocks_y * mid_freq.len();
    let prn = generate_prng_sequence(&format!("{}_mid", key), total_mid);

    let blocks: Vec<(usize, usize)> = (0..blocks_y)
        .flat_map(|by| (0..blocks_x).map(move |bx| (by, bx)))
        .collect();

    let modified_blocks: Vec<Vec<u8>> = blocks
        .par_iter()
        .map(|&(by, bx)| {
            let x0 = bx * BLOCK_SIZE;
            let y0 = by * BLOCK_SIZE;

            let mut block = vec![0.0f64; BLOCK_SIZE * BLOCK_SIZE];
            for dy in 0..BLOCK_SIZE {
                for dx in 0..BLOCK_SIZE {
                    block[dy * BLOCK_SIZE + dx] =
                        y_channel[(y0 + dy) * w as usize + (x0 + dx)] as f64;
                }
            }

            let weight = compute_texture_weight(&block);
            let mut dct_block = dct_2d(&block, BLOCK_SIZE);

            // Embed helper: cross-shaped redundancy (5 blocks)
            let mut embed_cross = |band: &[(usize, usize)], prn: &[f64], band_strength: f64| {
                let band_len = band.len();
                for (i, &(r, c)) in band.iter().enumerate() {
                    let combined = combined_prn(prn, blocks_x, blocks_y, band_len, by, bx, i);
                    let alpha = strength * weight * band_strength * 0.5;
                    dct_block[r * BLOCK_SIZE + c] += alpha * combined;
                }
            };

            embed_cross(&mid_freq, &prn, 2.2);

            let modified_block = idct_2d(&dct_block, BLOCK_SIZE);
            modified_block
                .iter()
                .map(|&x| x.clamp(0.0, 255.0) as u8)
                .collect()
        })
        .collect();

    let mut y_modified = y_channel.clone();
    for (i, (by, bx)) in blocks.iter().enumerate() {
        let x0 = bx * BLOCK_SIZE;
        let y0 = by * BLOCK_SIZE;
        for dy in 0..BLOCK_SIZE {
            for dx in 0..BLOCK_SIZE {
                y_modified[(y0 + dy) * w as usize + (x0 + dx)] =
                    modified_blocks[i][dy * BLOCK_SIZE + dx];
            }
        }
    }

    let result = y_channel_to_rgb(&y_modified, &image);
    let psnr = calculate_psnr(&image, &result);
    (result, psnr)
}

/// Detect watermark by comparing suspect to original (non-blind detection)
pub fn detect_watermark(
    original: &RgbImage,
    suspect: &RgbImage,
    key: &str,
    fpr: f64,
) -> DetectionResult {
    let original = crop_to_multiple(original.clone());
    let suspect = crop_to_multiple(suspect.clone());

    let (aligned, alignment) = align_images(&original, &suspect);

    if alignment.confidence < 0.2 {
        return DetectionResult {
            detected: false,
            score: 0.0,
            threshold: 0.0,
            alignment: AlignmentResult {
                rotation: alignment.rotation,
                scale: alignment.scale,
                shift_x: alignment.shift_x,
                shift_y: alignment.shift_y,
                score: alignment.score,
                confidence: alignment.confidence,
                transform: alignment.transform,
            },
        };
    }

    // Routing: warp for small rotation + non-geometric, PRN correction for large rotation + scale
    let has_transform = alignment.transform.is_some();
    let angle_deg = alignment.rotation.abs();
    let scale_diff = (alignment.scale - 1.0).abs();
    let is_geometric = angle_deg > 7.0 || scale_diff > 0.05;

    if has_transform && is_geometric {
        return detect_via_prn_correction(&original, &suspect, key, fpr, alignment);
    }

    // Warp path (rotation ≤ 5°, non-geometric, blur)
    detect_via_warp(&original, &aligned, key, fpr, alignment)
}

/// Traditional warp-based detection
fn detect_via_warp(
    original: &RgbImage,
    aligned: &RgbImage,
    key: &str,
    fpr: f64,
    alignment: AlignmentResult,
) -> DetectionResult {
    let aligned = normalize_histogram(original, aligned);

    let y_orig = rgb_to_y_channel(original);
    let y_suspect = rgb_to_y_channel(&aligned);

    let (w, h) = original.dimensions();
    let blocks_x = w as usize / BLOCK_SIZE;
    let blocks_y = h as usize / BLOCK_SIZE;

    let center_blocks = center_blocks_region(blocks_x, blocks_y, 0.5);

    let mid_freq = get_mid_freq_positions();
    let total_coeffs = blocks_x * blocks_y * mid_freq.len();
    let prn = generate_prng_sequence(&format!("{}_mid", key), total_coeffs);

    // Compute block scores with cross-shaped redundancy (5 blocks)
    let block_scores: Vec<f64> = center_blocks
        .par_iter()
        .map(|&(by, bx)| {
            let x0 = bx * BLOCK_SIZE;
            let y0 = by * BLOCK_SIZE;

            let mut block_orig = vec![0.0f64; BLOCK_SIZE * BLOCK_SIZE];
            let mut block_suspect = vec![0.0f64; BLOCK_SIZE * BLOCK_SIZE];
            for dy in 0..BLOCK_SIZE {
                for dx in 0..BLOCK_SIZE {
                    let idx = (y0 + dy) * w as usize + (x0 + dx);
                    block_orig[dy * BLOCK_SIZE + dx] = y_orig[idx] as f64;
                    block_suspect[dy * BLOCK_SIZE + dx] = y_suspect[idx] as f64;
                }
            }

            let dct_orig = dct_2d(&block_orig, BLOCK_SIZE);
            let dct_suspect = dct_2d(&block_suspect, BLOCK_SIZE);

            let band_len = mid_freq.len();
            let mut block_score = 0.0;
            for (i, &(r, c)) in mid_freq.iter().enumerate() {
                let diff = dct_suspect[r * BLOCK_SIZE + c] - dct_orig[r * BLOCK_SIZE + c];
                let combined = combined_prn(&prn, blocks_x, blocks_y, band_len, by, bx, i);
                block_score += diff * combined;
            }
            block_score / band_len as f64
        })
        .collect();

    let score = block_scores.iter().sum::<f64>() / block_scores.len() as f64;

    // Estimate threshold from mid-frequency band
    let mut noise_levels: Vec<f64> = center_blocks
        .par_iter()
        .map(|&(by, bx)| {
            let x0 = bx * BLOCK_SIZE;
            let y0 = by * BLOCK_SIZE;

            let mut block_orig = vec![0.0f64; BLOCK_SIZE * BLOCK_SIZE];
            let mut block_suspect = vec![0.0f64; BLOCK_SIZE * BLOCK_SIZE];
            for dy in 0..BLOCK_SIZE {
                for dx in 0..BLOCK_SIZE {
                    let idx = (y0 + dy) * w as usize + (x0 + dx);
                    block_orig[dy * BLOCK_SIZE + dx] = y_orig[idx] as f64;
                    block_suspect[dy * BLOCK_SIZE + dx] = y_suspect[idx] as f64;
                }
            }

            let dct_orig = dct_2d(&block_orig, BLOCK_SIZE);
            let dct_suspect = dct_2d(&block_suspect, BLOCK_SIZE);

            mid_freq
                .iter()
                .map(move |&(r, c)| {
                    (dct_suspect[r * BLOCK_SIZE + c] - dct_orig[r * BLOCK_SIZE + c]).abs()
                })
                .collect::<Vec<f64>>()
        })
        .flatten()
        .collect();

    noise_levels.sort_by(|a, b| a.partial_cmp(b).unwrap());
    if noise_levels.is_empty() {
        return DetectionResult {
            detected: false,
            score,
            threshold: 0.0,
            alignment,
        };
    }
    let median = noise_levels[noise_levels.len() / 2];
    let sigma_noise = median * 1.4826;
    let n_blocks = block_scores.len() as f64;
    let z_score = normal_ppf(1.0 - fpr);
    let threshold = z_score * sigma_noise / n_blocks.sqrt();

    DetectionResult {
        detected: score > threshold,
        score,
        threshold,
        alignment,
    }
}

/// Map suspect block corners through transform to original space,
/// return area-weighted list of overlapping original block indices
fn map_corners_to_weights(
    bx: usize,
    by: usize,
    transform: &[f32; 9],
    blocks_x: usize,
    blocks_y: usize,
) -> Option<Vec<(usize, f64)>> {
    let map_pt = |sx: f32, sy: f32| -> (f32, f32) {
        let w = transform[6] * sx + transform[7] * sy + transform[8];
        if w.abs() < 1e-10 {
            return (sx, sy);
        }
        let ox = (transform[0] * sx + transform[1] * sy + transform[2]) / w;
        let oy = (transform[3] * sx + transform[4] * sy + transform[5]) / w;
        (ox, oy)
    };

    let s16 = BLOCK_SIZE as f32;
    let s15 = s16 - 1.0;
    let corners = [
        map_pt(bx as f32 * s16, by as f32 * s16),
        map_pt(bx as f32 * s16 + s15, by as f32 * s16),
        map_pt(bx as f32 * s16, by as f32 * s16 + s15),
        map_pt(bx as f32 * s16 + s15, by as f32 * s16 + s15),
    ];

    let min_x = corners.iter().map(|p| p.0).fold(f32::MAX, f32::min);
    let max_x = corners.iter().map(|p| p.0).fold(f32::MIN, f32::max);
    let min_y = corners.iter().map(|p| p.1).fold(f32::MAX, f32::min);
    let max_y = corners.iter().map(|p| p.1).fold(f32::MIN, f32::max);

    let bx_min = ((min_x / s16).floor() as isize)
        .max(0)
        .min(blocks_x as isize - 1) as usize;
    let bx_max = (((max_x - 1.0) / s16).floor() as isize)
        .max(0)
        .min(blocks_x as isize - 1) as usize;
    let by_min = ((min_y / s16).floor() as isize)
        .max(0)
        .min(blocks_y as isize - 1) as usize;
    let by_max = (((max_y - 1.0) / s16).floor() as isize)
        .max(0)
        .min(blocks_y as isize - 1) as usize;

    let mut weights: Vec<(usize, f64)> = Vec::new();
    let mut total_area = 0.0f64;
    for oy in by_min..=by_max {
        for ox in bx_min..=bx_max {
            let ox0 = ox as f32 * s16;
            let ox1 = (ox + 1) as f32 * s16;
            let oy0 = oy as f32 * s16;
            let oy1 = (oy + 1) as f32 * s16;
            let overlap_w = (max_x.min(ox1) - min_x.max(ox0)).max(0.0);
            let overlap_h = (max_y.min(oy1) - min_y.max(oy0)).max(0.0);
            let area = overlap_w * overlap_h;
            if area > 0.0 {
                weights.push((oy * blocks_x + ox, area as f64));
                total_area += area as f64;
            }
        }
    }
    if total_area < 1e-10 {
        return None;
    }
    for (_, w) in &mut weights {
        *w /= total_area;
    }
    Some(weights)
}

/// Level 3: Area-weighted PRN correction using 4-corner mapping.
/// Maps each suspect block's corners through the ORB transform to find
/// the exact quadrilateral footprint in original space, then blends
/// overlapping original DCT blocks by overlap area.
fn detect_via_prn_correction(
    original: &RgbImage,
    suspect: &RgbImage,
    key: &str,
    fpr: f64,
    alignment: AlignmentResult,
) -> DetectionResult {
    let transform = alignment.transform.as_ref().unwrap();

    let (w, h) = original.dimensions();
    let (sw, sh) = suspect.dimensions();
    let suspect_resized = if (sw, sh) != (w, h) {
        image::imageops::resize(suspect, w, h, image::imageops::FilterType::Lanczos3)
    } else {
        suspect.clone()
    };

    let blocks_x = w as usize / BLOCK_SIZE;
    let blocks_y = h as usize / BLOCK_SIZE;
    let center_blocks = center_blocks_region(blocks_x, blocks_y, 0.5);

    let mid_freq = get_mid_freq_positions();
    let total_mid = blocks_x * blocks_y * mid_freq.len();
    let prn = generate_prng_sequence(&format!("{}_mid", key), total_mid);

    let y_orig = rgb_to_y_channel(original);
    let suspect_norm = normalize_histogram(original, &suspect_resized);
    let y_suspect = rgb_to_y_channel(&suspect_norm);

    // Precompute DCT for all original blocks
    let y_orig_for_dcts = y_orig.clone();
    let orig_dcts: Vec<Vec<f64>> = (0..blocks_y)
        .flat_map(|by| {
            let y_ref = &y_orig_for_dcts;
            (0..blocks_x).map(move |bx| {
                let x0 = bx * BLOCK_SIZE;
                let y0 = by * BLOCK_SIZE;
                let mut block = vec![0.0f64; BLOCK_SIZE * BLOCK_SIZE];
                for dy in 0..BLOCK_SIZE {
                    for dx in 0..BLOCK_SIZE {
                        block[dy * BLOCK_SIZE + dx] =
                            y_ref[(y0 + dy) * w as usize + (x0 + dx)] as f64;
                    }
                }
                dct_2d(&block, BLOCK_SIZE)
            })
        })
        .collect();

    let block_scores: Vec<f64> = center_blocks
        .par_iter()
        .map(|&(by, bx)| {
            let x0 = bx * BLOCK_SIZE;
            let y0 = by * BLOCK_SIZE;

            let mut block_suspect = vec![0.0f64; BLOCK_SIZE * BLOCK_SIZE];
            for dy in 0..BLOCK_SIZE {
                for dx in 0..BLOCK_SIZE {
                    let idx = (y0 + dy) * w as usize + (x0 + dx);
                    block_suspect[dy * BLOCK_SIZE + dx] = y_suspect[idx] as f64;
                }
            }
            let dct_suspect = dct_2d(&block_suspect, BLOCK_SIZE);

            let weights = match map_corners_to_weights(bx, by, transform, blocks_x, blocks_y) {
                Some(w) => w,
                None => return 0.0,
            };

            // Score mid band with area weights
            let band_compute = |band: &[(usize, usize)], prn: &[f64], band_weight: f64| -> f64 {
                let band_len = band.len();
                let mut s = 0.0;
                for (i, &(r, c)) in band.iter().enumerate() {
                    let mut weighted_diff = 0.0;
                    for &(b_idx, w) in &weights {
                        let d = dct_suspect[r * BLOCK_SIZE + c]
                            - orig_dcts[b_idx][r * BLOCK_SIZE + c];
                        weighted_diff += w * d;
                    }
                    let combined = combined_prn(prn, blocks_x, blocks_y, band_len, by, bx, i);
                    s += weighted_diff * combined;
                }
                s / band_len as f64 * band_weight
            };

            band_compute(&mid_freq, &prn, 1.0)
        })
        .collect();

    let score = block_scores.iter().sum::<f64>() / block_scores.len() as f64;

    // Estimate noise using mid-frequency band with area-weighted DCT diff
    let mut noise_levels: Vec<f64> = center_blocks
        .par_iter()
        .map(|&(by, bx)| {
            let x0 = bx * BLOCK_SIZE;
            let y0 = by * BLOCK_SIZE;

            let mut block_suspect = vec![0.0f64; BLOCK_SIZE * BLOCK_SIZE];
            for dy in 0..BLOCK_SIZE {
                for dx in 0..BLOCK_SIZE {
                    let idx = (y0 + dy) * w as usize + (x0 + dx);
                    block_suspect[dy * BLOCK_SIZE + dx] = y_suspect[idx] as f64;
                }
            }
            let dct_suspect = dct_2d(&block_suspect, BLOCK_SIZE);

            let weights = match map_corners_to_weights(bx, by, transform, blocks_x, blocks_y) {
                Some(w) => w,
                None => return Vec::new(),
            };

            let mut noises = Vec::new();
            for &(r, c) in &mid_freq {
                let mut diff_abs = 0.0;
                for &(b_idx, w) in &weights {
                    let d = (dct_suspect[r * BLOCK_SIZE + c]
                        - orig_dcts[b_idx][r * BLOCK_SIZE + c])
                        .abs();
                    diff_abs += w * d;
                }
                noises.push(diff_abs);
            }
            noises
        })
        .flatten()
        .collect();

    noise_levels.sort_by(|a, b| a.partial_cmp(b).unwrap());
    if noise_levels.is_empty() {
        return DetectionResult {
            detected: false,
            score,
            threshold: 0.0,
            alignment,
        };
    }
    let median = noise_levels[noise_levels.len() / 2];
    let sigma_noise = median * 1.4826;
    let n_blocks = block_scores.len() as f64;
    let z_score = normal_ppf(1.0 - fpr);
    let threshold = z_score * sigma_noise / n_blocks.sqrt();

    DetectionResult {
        detected: score > threshold,
        score,
        threshold,
        alignment,
    }
}

/// Calculate PSNR between two images
pub fn calculate_psnr(original: &RgbImage, modified: &RgbImage) -> f64 {
    let mse: f64 = original
        .pixels()
        .zip(modified.pixels())
        .map(|(a, b)| {
            let dr = a[0] as f64 - b[0] as f64;
            let dg = a[1] as f64 - b[1] as f64;
            let db = a[2] as f64 - b[2] as f64;
            (dr * dr + dg * dg + db * db) / 3.0
        })
        .sum::<f64>()
        / (original.width() * original.height()) as f64;

    if mse < 1e-10 {
        f64::INFINITY
    } else {
        10.0 * (255.0f64.powi(2) / mse).log10()
    }
}

/// Load image from path
pub fn load_image(path: &Path) -> Result<RgbImage, String> {
    image::ImageReader::open(path)
        .map_err(|e| format!("Failed to open image: {}", e))
        .and_then(|mut reader| {
            reader.no_limits();
            reader
                .with_guessed_format()
                .map_err(|e| format!("Failed to guess format: {}", e))
        })
        .and_then(|reader| {
            reader
                .decode()
                .map_err(|e| format!("Failed to decode image: {}", e))
        })
        .map(|img| img.to_rgb8())
}

/// Save image to path
pub fn save_image(path: &Path, image: &RgbImage) -> Result<(), String> {
    image
        .save(path)
        .map_err(|e| format!("Failed to save image: {}", e))
}

// Helper functions

fn rgb_to_y_channel(image: &RgbImage) -> Vec<u8> {
    image
        .pixels()
        .map(|p| {
            let y = (299 * p[0] as u32 + 587 * p[1] as u32 + 114 * p[2] as u32 + 500) / 1000;
            y.clamp(0, 255) as u8
        })
        .collect()
}

fn y_channel_to_rgb(y: &[u8], original: &RgbImage) -> RgbImage {
    let (w, h) = original.dimensions();
    let mut result = RgbImage::new(w, h);

    for (i, pixel) in original.pixels().enumerate() {
        let cb = -0.168736 * pixel[0] as f64 - 0.331264 * pixel[1] as f64
            + 0.5 * pixel[2] as f64
            + 128.0;
        let cr =
            0.5 * pixel[0] as f64 - 0.418688 * pixel[1] as f64 - 0.081312 * pixel[2] as f64
                + 128.0;

        let y_val = y[i] as f64;
        let r = (y_val + 1.402 * (cr - 128.0)).clamp(0.0, 255.0) as u8;
        let g =
            (y_val - 0.344136 * (cb - 128.0) - 0.714136 * (cr - 128.0)).clamp(0.0, 255.0) as u8;
        let b = (y_val + 1.772 * (cb - 128.0)).clamp(0.0, 255.0) as u8;

        result.put_pixel(
            (i % w as usize) as u32,
            (i / w as usize) as u32,
            image::Rgb([r, g, b]),
        );
    }

    result
}

/// Normal distribution inverse CDF (approximation)
#[allow(clippy::excessive_precision)]
fn normal_ppf(p: f64) -> f64 {
    if p <= 0.0 {
        return f64::NEG_INFINITY;
    }
    if p >= 1.0 {
        return f64::INFINITY;
    }

    let a = [
        -3.969683028665376e1,
        2.209460984245205e2,
        -2.759285104469687e2,
        1.383577518672690e2,
        -3.066479806614716e1,
        2.506628277459239e0,
    ];
    let b = [
        -5.447609879822406e1,
        1.615858368580409e2,
        -1.556989798598866e2,
        6.680131188771972e1,
        -1.328068155288572e1,
    ];
    let c = [
        -7.784894002430293e-3,
        -3.223964580411365e-1,
        -2.400758277161838e0,
        -2.549732539343734e0,
        4.374664141464968e0,
        2.938163982698783e0,
    ];
    let d = [
        7.784695709041462e-3,
        3.224671290700398e-1,
        2.445134137142996e0,
        3.754408661907416e0,
    ];

    let p_low = 0.02425;
    let p_high = 1.0 - p_low;

    if p < p_low {
        let q = (-2.0 * p.ln()).sqrt();
        (((((c[0] * q + c[1]) * q + c[2]) * q + c[3]) * q + c[4]) * q + c[5])
            / ((((d[0] * q + d[1]) * q + d[2]) * q + d[3]) * q + 1.0)
    } else if p <= p_high {
        let q = p - 0.5;
        let r = q * q;
        (((((a[0] * r + a[1]) * r + a[2]) * r + a[3]) * r + a[4]) * r + a[5]) * q
            / (((((b[0] * r + b[1]) * r + b[2]) * r + b[3]) * r + b[4]) * r + 1.0)
    } else {
        let q = (-2.0 * (1.0 - p).ln()).sqrt();
        -(((((c[0] * q + c[1]) * q + c[2]) * q + c[3]) * q + c[4]) * q + c[5])
            / ((((d[0] * q + d[1]) * q + d[2]) * q + d[3]) * q + 1.0)
    }
}
