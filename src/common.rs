use std::collections::BTreeMap;
use std::fs::File;
use std::io::{BufWriter, Cursor};
use std::path::Path;

use image::codecs::jpeg::JpegEncoder;
use image::imageops;
use image::{DynamicImage, ImageFormat, Rgb, RgbImage};
use serde::Serialize;
use thiserror::Error;

use crate::frame::{ParseFrameResult, bits_to_bytes, parse_frame};

pub(crate) const DEFAULT_KEY: &str = "hidden-watermark-default-key";
pub(crate) const DCT_BLOCK: u32 = 8;
pub(crate) const NON_DIAGNOSTIC_ATTEMPT_BUDGET: usize = 4_000;
pub(crate) const CROSS_BAND_PAIRS: [(usize, usize, usize, usize); 3] = [
    (1, 0, 0, 1), // low:  (1,0) vs (0,1)
    (2, 1, 1, 2), // mid:  (2,1) vs (1,2)
    (3, 2, 2, 3), // high: (3,2) vs (2,3)
];
pub(crate) const CROSS_BAND_WEIGHTS: [f32; 3] = [1.2, 1.0, 0.8];

pub(crate) fn cross_band_pairs(count: usize) -> &'static [(usize, usize, usize, usize)] {
    &CROSS_BAND_PAIRS[..count.min(3)]
}

pub(crate) fn cross_band_weights(count: usize) -> &'static [f32] {
    &CROSS_BAND_WEIGHTS[..count.min(3)]
}

pub(crate) const SCALE_FACTORS: [f32; 7] = [1.0, 0.5, 2.0, 0.75, 1.5, 0.67, 1.25];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BackendChoice {
    #[default]
    Auto,
    FrequencyV2,
    JpegDct,
}

#[derive(Clone, Debug)]
pub struct RobustWatermarkOptions {
    pub key: Option<String>,
    pub strength: f32,
    pub tile_size: u32,
    pub overlap: f32,
    pub preset: WatermarkPreset,
    pub cross_band_count: usize,
}

impl Default for RobustWatermarkOptions {
    fn default() -> Self {
        Self {
            key: None,
            strength: 0.25,
            tile_size: 512,
            overlap: 0.0,
            preset: WatermarkPreset::Invisible,
            cross_band_count: 3,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WatermarkPreset {
    #[default]
    Invisible,
    Balanced,
    Robust,
}

#[derive(Clone, Debug, Default)]
pub struct EncodeOptions {
    pub watermark: RobustWatermarkOptions,
    pub jpeg_quality: Option<u8>,
    pub backend: BackendChoice,
}

#[derive(Clone, Debug, Default)]
pub struct DecodeOptions {
    pub watermark: RobustWatermarkOptions,
    pub enable_diagnostics: bool,
    pub backend: BackendChoice,
}

#[derive(Clone, Debug, Serialize)]
pub struct EncodeReport {
    pub width: u32,
    pub height: u32,
    pub tile_size: u32,
    pub tile_count: usize,
    pub id_bytes: usize,
    pub strength: f32,
    pub psnr: f32,
    pub changed_pixels_ratio: f32,
    pub algorithm: &'static str,
    pub backend: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct DecodeReport {
    pub id: Option<String>,
    pub confidence: f32,
    pub corrected_bytes: usize,
    pub tile_hits: usize,
    pub best_rotation_degrees: i32,
    pub best_scale: f32,
    pub attempts: usize,
    pub status: DecodeStatus,
    pub diagnostics: Vec<TileDiagnostic>,
}

#[derive(Clone, Debug, Default, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DecodeStatus {
    Decoded,
    #[default]
    NoWatermark,
    CrcMismatch,
    UnsupportedPayload,
}

#[derive(Clone, Debug, Serialize)]
pub struct TileDiagnostic {
    pub x: u32,
    pub y: u32,
    pub rotation_degrees: i32,
    pub scale: f32,
    pub confidence: f32,
    pub corrected_bytes: usize,
    pub status: DecodeStatus,
}

#[derive(Clone, Debug, Serialize)]
pub struct CapacityReport {
    pub width: u32,
    pub height: u32,
    pub tile_size: u32,
    pub tile_count: usize,
    pub max_id_bytes: usize,
    pub recommended_id_bytes: usize,
}

#[derive(Debug, Error)]
pub enum WatermarkError {
    #[error("ID is {actual} bytes, but the maximum supported ID length is {max} bytes")]
    IdTooLong { actual: usize, max: usize },
    #[error("image is {width}x{height}, smaller than tile size {tile_size}")]
    ImageTooSmall {
        width: u32,
        height: u32,
        tile_size: u32,
    },
    #[error("tile size must be at least 64 pixels")]
    TileTooSmall,
    #[error("tile size must be divisible by 16 for frequency_v2")]
    InvalidTileSize,
    #[error("strength must be a finite positive number")]
    InvalidStrength,
    #[error("overlap must be in the range [0.0, 0.9)")]
    InvalidOverlap,
    #[error("JPEG quality must be between 1 and 100")]
    InvalidJpegQuality,
    #[error(
        "payload needs {required_bits} bits, but tile size {tile_size} can carry {capacity_bits} bits"
    )]
    PayloadTooLarge {
        required_bits: usize,
        capacity_bits: usize,
        tile_size: u32,
    },
    #[error("image error: {0}")]
    Image(#[from] image::ImageError),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("unsupported output format for path {0}")]
    UnsupportedOutputFormat(String),
}

pub type Result<T> = std::result::Result<T, WatermarkError>;

// --- Internal types ---

#[derive(Clone, Copy, Debug)]
pub(crate) enum Rotation {
    Deg0,
    Deg90,
    Deg180,
    Deg270,
}

impl Rotation {
    pub(crate) fn degrees(self) -> i32 {
        match self {
            Self::Deg0 => 0,
            Self::Deg90 => 90,
            Self::Deg180 => 180,
            Self::Deg270 => 270,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct FrequencyBlock {
    pub(crate) subband: FrequencySubband,
    pub(crate) x: u32,
    pub(crate) y: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FrequencySubband {
    Ll,
}

#[derive(Clone, Debug)]
pub(crate) struct WaveletTile {
    pub(crate) ll: Vec<f32>,
    pub(crate) lh: Vec<f32>,
    pub(crate) hl: Vec<f32>,
    pub(crate) hh: Vec<f32>,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct TileAttempt {
    pub(crate) id: Option<String>,
    pub(crate) status: DecodeStatus,
    pub(crate) confidence: f32,
    pub(crate) corrected_bytes: usize,
    pub(crate) bits: Vec<bool>,
    pub(crate) raw_bytes: usize,
}

#[derive(Clone, Debug)]
pub(crate) struct DecodedBits {
    pub(crate) bits: Vec<bool>,
    pub(crate) confidence: f32,
}

// --- Validation ---

pub(crate) fn validate_options(options: &RobustWatermarkOptions) -> Result<()> {
    if options.tile_size < 64 {
        return Err(WatermarkError::TileTooSmall);
    }
    if !options.tile_size.is_multiple_of(16) {
        return Err(WatermarkError::InvalidTileSize);
    }
    if !options.strength.is_finite() || options.strength <= 0.0 {
        return Err(WatermarkError::InvalidStrength);
    }
    if !(0.0..0.9).contains(&options.overlap) {
        return Err(WatermarkError::InvalidOverlap);
    }
    Ok(())
}

pub(crate) fn normalized_key(key: &Option<String>) -> &str {
    key.as_deref()
        .filter(|value| !value.is_empty())
        .unwrap_or(DEFAULT_KEY)
}

// --- Tile geometry ---

pub(crate) fn tile_origins(
    width: u32,
    height: u32,
    options: &RobustWatermarkOptions,
) -> Result<Vec<(u32, u32)>> {
    if width < options.tile_size || height < options.tile_size {
        return Err(WatermarkError::ImageTooSmall {
            width,
            height,
            tile_size: options.tile_size,
        });
    }
    let stride = ((options.tile_size as f32) * (1.0 - options.overlap))
        .round()
        .max(1.0) as u32;
    let xs = axis_origins(width, options.tile_size, stride);
    let ys = axis_origins(height, options.tile_size, stride);
    let mut origins = Vec::with_capacity(xs.len() * ys.len());
    for y in ys {
        for &x in &xs {
            origins.push((x, y));
        }
    }
    Ok(origins)
}

pub(crate) fn axis_origins(length: u32, tile_size: u32, stride: u32) -> Vec<u32> {
    let mut values = Vec::new();
    let mut value = 0_u32;
    while value + tile_size < length {
        values.push(value);
        value = value.saturating_add(stride);
    }
    values.push(length - tile_size);
    values.sort_unstable();
    values.dedup();
    values
}

pub(crate) fn search_origins(width: u32, height: u32, candidate_tile: u32) -> Vec<(u32, u32)> {
    let step = (candidate_tile / 4).max(8);
    grid_search_origins(width, height, candidate_tile, step)
}

pub(crate) fn grid_search_origins(
    width: u32,
    height: u32,
    candidate_tile: u32,
    step: u32,
) -> Vec<(u32, u32)> {
    let xs = axis_origins(width, candidate_tile, step);
    let ys = axis_origins(height, candidate_tile, step);
    let mut origins = Vec::with_capacity(xs.len() * ys.len());
    for y in ys {
        for &x in &xs {
            origins.push((x, y));
        }
    }
    origins
}

// --- Image I/O ---

pub(crate) fn load_rgb_image(path: &Path) -> Result<RgbImage> {
    Ok(load_dynamic_image(path)?.to_rgb8())
}

pub(crate) fn load_dynamic_image(path: &Path) -> Result<DynamicImage> {
    let bytes = std::fs::read(path)?;
    let format = image::guess_format(&bytes)?;
    Ok(image::load(Cursor::new(bytes), format)?)
}

pub(crate) fn save_rgb_image(
    image: &RgbImage,
    path: &Path,
    jpeg_quality: Option<u8>,
) -> Result<()> {
    let format = ImageFormat::from_path(path)
        .map_err(|_| WatermarkError::UnsupportedOutputFormat(path.display().to_string()))?;
    match format {
        ImageFormat::Jpeg => {
            let quality = jpeg_quality.unwrap_or(92);
            if !(1..=100).contains(&quality) {
                return Err(WatermarkError::InvalidJpegQuality);
            }
            let file = File::create(path)?;
            let writer = BufWriter::new(file);
            let mut encoder = JpegEncoder::new_with_quality(writer, quality);
            encoder.encode_image(&DynamicImage::ImageRgb8(image.clone()))?;
        }
        _ => image.save_with_format(path, format)?,
    }
    Ok(())
}

// --- Color conversion ---

pub(crate) fn rgb_to_ycbcr(pixel: &Rgb<u8>) -> (f32, f32, f32) {
    let r = f32::from(pixel[0]);
    let g = f32::from(pixel[1]);
    let b = f32::from(pixel[2]);
    let y = 0.299 * r + 0.587 * g + 0.114 * b;
    let cb = 128.0 - 0.168_736 * r - 0.331_264 * g + 0.5 * b;
    let cr = 128.0 + 0.5 * r - 0.418_688 * g - 0.081_312 * b;
    (y, cb, cr)
}

pub(crate) fn ycbcr_to_rgb(y: f32, cb: f32, cr: f32) -> Rgb<u8> {
    let cb = cb - 128.0;
    let cr = cr - 128.0;
    Rgb([
        clamp_u8(y + 1.402 * cr),
        clamp_u8(y - 0.344_136 * cb - 0.714_136 * cr),
        clamp_u8(y + 1.772 * cb),
    ])
}

pub(crate) fn clamp_u8(value: f32) -> u8 {
    value.round().clamp(0.0, 255.0) as u8
}

pub(crate) fn to_luma_f32(image: &RgbImage) -> Vec<f32> {
    image
        .pixels()
        .map(|pixel| {
            0.299 * f32::from(pixel[0]) + 0.587 * f32::from(pixel[1]) + 0.114 * f32::from(pixel[2])
        })
        .collect()
}

// --- Image transforms ---

pub(crate) fn rotate_rgb(image: &RgbImage, rotation: Rotation) -> RgbImage {
    match rotation {
        Rotation::Deg0 => image.clone(),
        Rotation::Deg90 => imageops::rotate90(image),
        Rotation::Deg180 => imageops::rotate180(image),
        Rotation::Deg270 => imageops::rotate270(image),
    }
}

// --- Quality metrics ---

pub(crate) fn calculate_psnr(original: &RgbImage, watermarked: &RgbImage) -> f32 {
    let mut mse = 0.0_f64;
    let samples = f64::from(original.width() * original.height() * 3);
    for (left, right) in original.pixels().zip(watermarked.pixels()) {
        for channel in 0..3 {
            let diff = f64::from(left[channel]) - f64::from(right[channel]);
            mse += diff * diff;
        }
    }
    mse /= samples.max(1.0);
    if mse == 0.0 {
        f32::INFINITY
    } else {
        (10.0 * ((255.0 * 255.0) / mse).log10()) as f32
    }
}

pub(crate) fn changed_pixels_ratio(original: &RgbImage, watermarked: &RgbImage) -> f32 {
    let changed = original
        .pixels()
        .zip(watermarked.pixels())
        .filter(|(left, right)| left != right)
        .count();
    changed as f32 / (original.width() * original.height()).max(1) as f32
}

// --- Luma tile I/O ---

pub(crate) fn read_luma_tile(
    channel: &[f32],
    width: u32,
    origin_x: u32,
    origin_y: u32,
    tile_size: u32,
) -> Vec<f32> {
    let mut tile = vec![0.0_f32; (tile_size * tile_size) as usize];
    for y in 0..tile_size {
        for x in 0..tile_size {
            tile[(y * tile_size + x) as usize] =
                channel[((origin_y + y) * width + origin_x + x) as usize];
        }
    }
    tile
}

pub(crate) fn accumulate_luma_tile(
    accum: &mut [f32],
    weights: &mut [u16],
    width: u32,
    origin_x: u32,
    origin_y: u32,
    tile_size: u32,
    tile: &[f32],
) {
    for y in 0..tile_size {
        for x in 0..tile_size {
            let dst = ((origin_y + y) * width + origin_x + x) as usize;
            accum[dst] += tile[(y * tile_size + x) as usize].clamp(0.0, 255.0);
            weights[dst] = weights[dst].saturating_add(1);
        }
    }
}

pub(crate) fn normalize_tile(
    gray: &[f32],
    width: u32,
    origin_x: u32,
    origin_y: u32,
    candidate_tile: u32,
    base_tile_size: u32,
) -> Vec<f32> {
    let mut normalized = vec![0.0_f32; (base_tile_size * base_tile_size) as usize];
    let ratio = candidate_tile as f32 / base_tile_size as f32;
    for y in 0..base_tile_size {
        for x in 0..base_tile_size {
            let sx = origin_x
                + (((x as f32) + 0.5) * ratio)
                    .floor()
                    .clamp(0.0, (candidate_tile - 1) as f32) as u32;
            let sy = origin_y
                + (((y as f32) + 0.5) * ratio)
                    .floor()
                    .clamp(0.0, (candidate_tile - 1) as f32) as u32;
            normalized[(y * base_tile_size + x) as usize] = gray[(sy * width + sx) as usize];
        }
    }
    normalized
}

// --- DCT primitives ---

pub(crate) fn dct_2d(block: &mut [f32; 64]) {
    let input = *block;
    for v in 0..8 {
        for u in 0..8 {
            let mut sum = 0.0;
            for y in 0..8 {
                for x in 0..8 {
                    let cx = (((2 * x + 1) as f32 * u as f32 * std::f32::consts::PI) / 16.0).cos();
                    let cy = (((2 * y + 1) as f32 * v as f32 * std::f32::consts::PI) / 16.0).cos();
                    sum += input[y * 8 + x] * cx * cy;
                }
            }
            block[v * 8 + u] = 0.25 * dct_alpha(u) * dct_alpha(v) * sum;
        }
    }
}

pub(crate) fn idct_2d(block: &mut [f32; 64]) {
    let input = *block;
    for y in 0..8 {
        for x in 0..8 {
            let mut sum = 0.0;
            for v in 0..8 {
                for u in 0..8 {
                    let cx = (((2 * x + 1) as f32 * u as f32 * std::f32::consts::PI) / 16.0).cos();
                    let cy = (((2 * y + 1) as f32 * v as f32 * std::f32::consts::PI) / 16.0).cos();
                    sum += dct_alpha(u) * dct_alpha(v) * input[v * 8 + u] * cx * cy;
                }
            }
            block[y * 8 + x] = 0.25 * sum;
        }
    }
}

pub(crate) fn dct_alpha(index: usize) -> f32 {
    if index == 0 {
        1.0 / 2.0_f32.sqrt()
    } else {
        1.0
    }
}

pub(crate) fn dct_index(u: usize, v: usize) -> usize {
    v * 8 + u
}

pub(crate) fn read_block8(data: &[f32], width: u32, block_x: u32, block_y: u32) -> [f32; 64] {
    let mut block = [0.0_f32; 64];
    for y in 0..DCT_BLOCK {
        for x in 0..DCT_BLOCK {
            block[(y * DCT_BLOCK + x) as usize] =
                data[((block_y + y) * width + block_x + x) as usize];
        }
    }
    block
}

#[allow(dead_code)]
pub(crate) fn dct_pair_diff(data: &[f32], width: u32, block_x: u32, block_y: u32) -> f32 {
    let mut block = read_block8(data, width, block_x, block_y);
    dct_2d(&mut block);
    block[dct_index(3, 2)] - block[dct_index(2, 3)]
}

pub(crate) fn write_block8(
    data: &mut [f32],
    width: u32,
    block_x: u32,
    block_y: u32,
    block: &[f32; 64],
) {
    for y in 0..DCT_BLOCK {
        for x in 0..DCT_BLOCK {
            data[((block_y + y) * width + block_x + x) as usize] =
                block[(y * DCT_BLOCK + x) as usize];
        }
    }
}

// --- Haar DWT ---

pub(crate) fn haar_forward(tile: &[f32], tile_size: u32) -> WaveletTile {
    let sub_size = tile_size / 2;
    let mut ll = vec![0.0; (sub_size * sub_size) as usize];
    let mut lh = vec![0.0; (sub_size * sub_size) as usize];
    let mut hl = vec![0.0; (sub_size * sub_size) as usize];
    let mut hh = vec![0.0; (sub_size * sub_size) as usize];
    for y in 0..sub_size {
        for x in 0..sub_size {
            let a = tile[((2 * y) * tile_size + 2 * x) as usize];
            let b = tile[((2 * y) * tile_size + 2 * x + 1) as usize];
            let c = tile[((2 * y + 1) * tile_size + 2 * x) as usize];
            let d = tile[((2 * y + 1) * tile_size + 2 * x + 1) as usize];
            let idx = (y * sub_size + x) as usize;
            ll[idx] = (a + b + c + d) * 0.5;
            hl[idx] = (a - b + c - d) * 0.5;
            lh[idx] = (a + b - c - d) * 0.5;
            hh[idx] = (a - b - c + d) * 0.5;
        }
    }
    WaveletTile { ll, lh, hl, hh }
}

pub(crate) fn haar_inverse(wavelet: &WaveletTile, tile_size: u32) -> Vec<f32> {
    let sub_size = tile_size / 2;
    let mut tile = vec![0.0; (tile_size * tile_size) as usize];
    for y in 0..sub_size {
        for x in 0..sub_size {
            let idx = (y * sub_size + x) as usize;
            let ll = wavelet.ll[idx];
            let hl = wavelet.hl[idx];
            let lh = wavelet.lh[idx];
            let hh = wavelet.hh[idx];
            tile[((2 * y) * tile_size + 2 * x) as usize] = (ll + hl + lh + hh) * 0.5;
            tile[((2 * y) * tile_size + 2 * x + 1) as usize] = (ll - hl + lh - hh) * 0.5;
            tile[((2 * y + 1) * tile_size + 2 * x) as usize] = (ll + hl - lh - hh) * 0.5;
            tile[((2 * y + 1) * tile_size + 2 * x + 1) as usize] = (ll - hl - lh + hh) * 0.5;
        }
    }
    tile
}

// --- Perceptual masking ---

pub(crate) fn block_variance(data: &[f32], width: u32, block_x: u32, block_y: u32) -> f32 {
    let mut sum = 0.0;
    let mut sum_sq = 0.0;
    for y in 0..DCT_BLOCK {
        for x in 0..DCT_BLOCK {
            let value = data[((block_y + y) * width + block_x + x) as usize];
            sum += value;
            sum_sq += value * value;
        }
    }
    let mean = sum / 64.0;
    (sum_sq / 64.0) - mean * mean
}

pub(crate) fn perceptual_mask(variance: f32) -> f32 {
    if variance < 4.0 {
        0.35
    } else if variance < 32.0 {
        0.55
    } else if variance < 256.0 {
        0.8
    } else {
        1.0
    }
}

// --- Decode report assembly ---

pub(crate) fn tile_group_score(hits: &[(TileAttempt, Rotation, f32)]) -> f32 {
    hits.iter().map(|h| h.0.confidence).sum::<f32>() + hits.len() as f32
}

pub(crate) fn report_from_hits(
    valid_hits: Vec<(TileAttempt, Rotation, f32)>,
    mut diagnostics: Vec<TileDiagnostic>,
    attempts: usize,
) -> DecodeReport {
    let mut grouped: BTreeMap<String, Vec<(TileAttempt, Rotation, f32)>> = BTreeMap::new();
    for hit in valid_hits {
        if let Some(id) = hit.0.id.clone() {
            grouped.entry(id).or_default().push(hit);
        }
    }

    let (best_id, best_hits) = grouped
        .into_iter()
        .max_by(|(_, a), (_, b)| {
            let score_a = tile_group_score(a);
            let score_b = tile_group_score(b);
            score_a.total_cmp(&score_b)
        })
        .expect("valid hits must contain an ID");

    let best_hit = best_hits
        .iter()
        .max_by(|a, b| a.0.confidence.total_cmp(&b.0.confidence))
        .expect("group is non-empty");
    let avg_confidence =
        best_hits.iter().map(|h| h.0.confidence).sum::<f32>() / best_hits.len() as f32;

    diagnostics.sort_by(|a, b| b.confidence.total_cmp(&a.confidence));

    DecodeReport {
        id: Some(best_id),
        confidence: avg_confidence.clamp(0.0, 1.0),
        corrected_bytes: best_hits
            .iter()
            .map(|h| h.0.corrected_bytes)
            .min()
            .unwrap_or(0),
        tile_hits: best_hits.len(),
        best_rotation_degrees: best_hit.1.degrees(),
        best_scale: best_hit.2,
        attempts,
        status: DecodeStatus::Decoded,
        diagnostics,
    }
}

pub(crate) fn recover_from_crc_hits(
    mut hits: Vec<(TileAttempt, Rotation, f32)>,
    mut diagnostics: Vec<TileDiagnostic>,
    attempts: usize,
) -> Option<DecodeReport> {
    hits.retain(|hit| !hit.0.bits.is_empty() && hit.0.confidence > 0.03);
    hits.sort_by(|a, b| b.0.confidence.total_cmp(&a.0.confidence));

    let mut by_len: BTreeMap<usize, Vec<(TileAttempt, Rotation, f32)>> = BTreeMap::new();
    for hit in hits.into_iter().take(64) {
        by_len.entry(hit.0.raw_bytes).or_default().push(hit);
    }

    for (_raw_bytes, group) in by_len.into_iter().rev() {
        if group.len() < 2 {
            continue;
        }
        let bit_len = group[0].0.bits.len();
        if bit_len == 0 || group.iter().any(|hit| hit.0.bits.len() != bit_len) {
            continue;
        }

        let mut merged_bits = Vec::with_capacity(bit_len);
        for bit_index in 0..bit_len {
            let ones = group.iter().filter(|hit| hit.0.bits[bit_index]).count();
            merged_bits.push(ones * 2 >= group.len());
        }

        let raw_bytes = group[0].0.raw_bytes;
        let frame = bits_to_bytes(&merged_bits, raw_bytes);
        if let ParseFrameResult::Decoded(id) = parse_frame(&frame) {
            let best_hit = group
                .iter()
                .max_by(|a, b| a.0.confidence.total_cmp(&b.0.confidence))?;
            diagnostics.sort_by(|a, b| b.confidence.total_cmp(&a.confidence));
            let confidence = (group.iter().map(|hit| hit.0.confidence).sum::<f32>()
                / group.len() as f32)
                .clamp(0.0, 1.0);
            return Some(DecodeReport {
                id: Some(id),
                confidence,
                corrected_bytes: 0,
                tile_hits: group.len(),
                best_rotation_degrees: best_hit.1.degrees(),
                best_scale: best_hit.2,
                attempts,
                status: DecodeStatus::Decoded,
                diagnostics,
            });
        }
    }

    None
}
