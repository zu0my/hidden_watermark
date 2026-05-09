mod backend;
#[allow(dead_code)]
pub(crate) mod bch;
mod common;
mod frame;

use std::path::Path;

use image::{GenericImageView, ImageFormat};

use backend::WatermarkBackend;
use backend::frequency_v2::FrequencyV2Backend;
use backend::jpeg_dct::JpegDctBackend;
use common::{
    NON_DIAGNOSTIC_ATTEMPT_BUDGET, Rotation, SCALE_FACTORS, calculate_psnr, changed_pixels_ratio,
    grid_search_origins, load_rgb_image, normalized_key, recover_from_crc_hits, report_from_hits,
    rotate_rgb, save_rgb_image, search_origins, tile_origins, to_luma_f32, validate_options,
};
use frame::{MAX_ID_BYTES, build_frame, frame_bits};

pub use common::{
    BackendChoice, CapacityReport, DecodeOptions, DecodeReport, DecodeStatus, EncodeOptions,
    EncodeReport, Result, RobustWatermarkOptions, TileDiagnostic, WatermarkError, WatermarkPreset,
};

use std::process::Command;

const ROTATIONS: [Rotation; 4] = [
    Rotation::Deg0,
    Rotation::Deg90,
    Rotation::Deg180,
    Rotation::Deg270,
];

fn select_backend_for_path(path: &Path, choice: BackendChoice) -> Box<dyn WatermarkBackend> {
    match choice {
        BackendChoice::FrequencyV2 => Box::new(FrequencyV2Backend),
        BackendChoice::JpegDct => Box::new(JpegDctBackend),
        BackendChoice::Auto => {
            let format = ImageFormat::from_path(path).ok();
            match format {
                Some(ImageFormat::Jpeg) => Box::new(JpegDctBackend),
                _ => Box::new(FrequencyV2Backend),
            }
        }
    }
}

fn decode_with_backend(
    backend: &dyn WatermarkBackend,
    source: &image::RgbImage,
    key: &str,
    options: &DecodeOptions,
) -> Option<DecodeReport> {
    let mut attempts = 0_usize;
    let mut valid_hits = Vec::new();
    let mut recoverable_hits = Vec::new();
    let mut diagnostics = Vec::new();
    let mut crc_like_failures = 0_usize;
    let header_probe_bits = frame::HEADER_BYTES * 8;

    for exhaustive in [false, true] {
        for rotation in ROTATIONS {
            let rotated = rotate_rgb(source, rotation);
            let gray = to_luma_f32(&rotated);
            let (width, height) = rotated.dimensions();

            for scale in SCALE_FACTORS {
                let candidate_tile = ((options.watermark.tile_size as f32) * scale).round() as u32;
                if candidate_tile < 24 || candidate_tile > width || candidate_tile > height {
                    continue;
                }

                let origins = if exhaustive {
                    search_origins(width, height, candidate_tile)
                } else {
                    let stride = ((options.watermark.tile_size as f32)
                        * scale
                        * (1.0 - options.watermark.overlap))
                        .round()
                        .max(1.0) as u32;
                    grid_search_origins(width, height, candidate_tile, stride)
                };

                for (x, y) in origins {
                    attempts += 1;
                    let attempt = backend.decode_tile(
                        &gray,
                        width,
                        height,
                        x,
                        y,
                        candidate_tile,
                        options.watermark.tile_size,
                        scale,
                        key,
                        header_probe_bits,
                        options.watermark.cross_band_count,
                    );

                    match &attempt.status {
                        DecodeStatus::Decoded => {
                            valid_hits.push((attempt.clone(), rotation, scale))
                        }
                        DecodeStatus::CrcMismatch | DecodeStatus::UnsupportedPayload => {
                            crc_like_failures += 1;
                            if matches!(attempt.status, DecodeStatus::CrcMismatch) {
                                recoverable_hits.push((attempt.clone(), rotation, scale));
                            }
                        }
                        DecodeStatus::NoWatermark => {}
                    }

                    if options.enable_diagnostics && diagnostics.len() < 256 {
                        diagnostics.push(TileDiagnostic {
                            x,
                            y,
                            rotation_degrees: rotation.degrees(),
                            scale,
                            confidence: attempt.confidence,
                            corrected_bytes: attempt.corrected_bytes,
                            status: attempt.status,
                        });
                    }

                    if !options.enable_diagnostics && !valid_hits.is_empty() {
                        return Some(report_from_hits(valid_hits, diagnostics, attempts));
                    }
                    if !options.enable_diagnostics
                        && attempts >= NON_DIAGNOSTIC_ATTEMPT_BUDGET
                        && let Some(report) = recover_from_crc_hits(
                            recoverable_hits.clone(),
                            diagnostics.clone(),
                            attempts,
                        )
                    {
                        return Some(report);
                    }
                    if !options.enable_diagnostics && attempts >= NON_DIAGNOSTIC_ATTEMPT_BUDGET {
                        return Some(DecodeReport {
                            id: None,
                            confidence: 0.0,
                            corrected_bytes: 0,
                            tile_hits: 0,
                            best_rotation_degrees: rotation.degrees(),
                            best_scale: scale,
                            attempts,
                            status: DecodeStatus::NoWatermark,
                            diagnostics,
                        });
                    }
                }
            }
        }

        if !valid_hits.is_empty() {
            break;
        }
    }

    if valid_hits.is_empty() {
        if let Some(report) = recover_from_crc_hits(recoverable_hits, diagnostics.clone(), attempts)
        {
            return Some(report);
        }

        diagnostics.sort_by(|a, b| b.confidence.total_cmp(&a.confidence));
        let status = if crc_like_failures > 0 {
            DecodeStatus::CrcMismatch
        } else {
            DecodeStatus::NoWatermark
        };

        return Some(DecodeReport {
            id: None,
            confidence: 0.0,
            corrected_bytes: 0,
            tile_hits: 0,
            best_rotation_degrees: 0,
            best_scale: 1.0,
            attempts,
            status,
            diagnostics,
        });
    }

    Some(report_from_hits(valid_hits, diagnostics, attempts))
}

pub fn encode_image(
    input: impl AsRef<Path>,
    output: impl AsRef<Path>,
    id: &str,
    options: EncodeOptions,
) -> Result<EncodeReport> {
    validate_options(&options.watermark)?;
    if let Some(quality) = options.jpeg_quality
        && !(1..=100).contains(&quality)
    {
        return Err(WatermarkError::InvalidJpegQuality);
    }

    let id_bytes = id.as_bytes();
    if id_bytes.len() > MAX_ID_BYTES {
        return Err(WatermarkError::IdTooLong {
            actual: id_bytes.len(),
            max: MAX_ID_BYTES,
        });
    }

    let image = load_rgb_image(input.as_ref())?;
    let (width, height) = image.dimensions();
    let origins = tile_origins(width, height, &options.watermark)?;
    let frame = build_frame(id_bytes);
    let bits = frame_bits(&frame);

    let backend = select_backend_for_path(output.as_ref(), options.backend);
    let capacity_bits = backend.capacity_bits(
        options.watermark.tile_size,
        options.watermark.cross_band_count,
    );
    if bits.len() > capacity_bits {
        return Err(WatermarkError::PayloadTooLarge {
            required_bits: bits.len(),
            capacity_bits,
            tile_size: options.watermark.tile_size,
        });
    }

    let key = normalized_key(&options.watermark.key);
    let output_image = backend.embed(&image, &origins, key, &bits, &options.watermark);
    let psnr = calculate_psnr(&image, &output_image);
    let ratio = changed_pixels_ratio(&image, &output_image);
    save_rgb_image(&output_image, output.as_ref(), options.jpeg_quality)?;

    Ok(EncodeReport {
        width,
        height,
        tile_size: options.watermark.tile_size,
        tile_count: origins.len(),
        id_bytes: id_bytes.len(),
        strength: options.watermark.strength,
        psnr,
        changed_pixels_ratio: ratio,
        algorithm: backend.name(),
        backend: backend.name(),
    })
}

pub fn decode_image(input: impl AsRef<Path>, options: DecodeOptions) -> Result<DecodeReport> {
    validate_options(&options.watermark)?;
    let source = load_rgb_image(input.as_ref())?;
    let key = normalized_key(&options.watermark.key);

    match options.backend {
        BackendChoice::Auto => {
            // Try format-detected primary backend first.
            let primary = select_backend_for_path(input.as_ref(), BackendChoice::Auto);
            if let Some(report) = decode_with_backend(primary.as_ref(), &source, key, &options)
                && report.status == DecodeStatus::Decoded
            {
                return Ok(report);
            }

            // Fall back to the other backend.
            let fallback: Box<dyn WatermarkBackend> = if primary.name() == FrequencyV2Backend.name()
            {
                Box::new(JpegDctBackend)
            } else {
                Box::new(FrequencyV2Backend)
            };
            if let Some(report) = decode_with_backend(fallback.as_ref(), &source, key, &options) {
                return Ok(report);
            }

            // Neither succeeded — return a no-watermark report.
            Ok(DecodeReport {
                id: None,
                confidence: 0.0,
                corrected_bytes: 0,
                tile_hits: 0,
                best_rotation_degrees: 0,
                best_scale: 1.0,
                attempts: 0,
                status: DecodeStatus::NoWatermark,
                diagnostics: Vec::new(),
            })
        }
        choice => {
            let backend = select_backend_for_path(input.as_ref(), choice);
            Ok(
                decode_with_backend(backend.as_ref(), &source, key, &options).unwrap_or(
                    DecodeReport {
                        id: None,
                        confidence: 0.0,
                        corrected_bytes: 0,
                        tile_hits: 0,
                        best_rotation_degrees: 0,
                        best_scale: 1.0,
                        attempts: 0,
                        status: DecodeStatus::NoWatermark,
                        diagnostics: Vec::new(),
                    },
                ),
            )
        }
    }
}

pub fn estimate_capacity(
    input: impl AsRef<Path>,
    options: RobustWatermarkOptions,
) -> Result<CapacityReport> {
    validate_options(&options)?;
    let image = common::load_dynamic_image(input.as_ref())?;
    let (width, height) = image.dimensions();
    let origins = tile_origins(width, height, &options)?;
    let backend = select_backend_for_path(input.as_ref(), BackendChoice::Auto);
    let max_id_bytes = backend
        .capacity_bits(options.tile_size, options.cross_band_count)
        .saturating_div(8)
        .saturating_sub(frame::HEADER_BYTES + frame::CRC_BYTES)
        .min(MAX_ID_BYTES);

    Ok(CapacityReport {
        width,
        height,
        tile_size: options.tile_size,
        tile_count: origins.len(),
        max_id_bytes,
        recommended_id_bytes: max_id_bytes.min(32),
    })
}

#[cfg(feature = "opencv")]
pub fn opencv_backend_available() -> bool {
    false
}

#[cfg(not(feature = "opencv"))]
pub fn opencv_backend_available() -> bool {
    false
}

const PREPROCESS_SCRIPT: &str = include_str!("../scripts/preprocess.py");

pub fn preprocess_with_opencv(input: &Path) -> Result<std::path::PathBuf> {
    let python = find_python().ok_or_else(|| {
        WatermarkError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "Python not found; install python3 or python",
        ))
    })?;

    // Check if OpenCV is available
    let check = Command::new(&python)
        .args(["-c", "import cv2"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
    if check.is_err() || !check.unwrap().success() {
        return Err(WatermarkError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "OpenCV not found; install python3-opencv or pip install opencv-python",
        )));
    }

    // Write embedded script to temp file
    let dir = tempfile::tempdir()?;
    let script_path = dir.path().join("preprocess.py");
    std::fs::write(&script_path, PREPROCESS_SCRIPT)?;

    let output = dir.path().join("preprocessed.png");

    let status = Command::new(&python)
        .arg(&script_path)
        .arg("--input")
        .arg(input)
        .arg("--output")
        .arg(&output)
        .status()?;

    if !status.success() {
        return Err(WatermarkError::Io(std::io::Error::other(format!(
            "preprocess.py exited with status {status}"
        ))));
    }

    // Leak the temp dir so the file persists for the caller to use.
    // The OS will clean it up eventually.
    let _ = dir.keep();
    Ok(output)
}

fn find_python() -> Option<String> {
    for cmd in ["python3", "python"] {
        if Command::new(cmd)
            .arg("--version")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .is_ok()
        {
            return Some(cmd.to_string());
        }
    }
    None
}
