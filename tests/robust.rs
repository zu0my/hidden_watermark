use hidden_watermark::{
    BackendChoice, DecodeOptions, DecodeStatus, EncodeOptions, RobustWatermarkOptions,
    decode_image, encode_image, estimate_capacity,
};
use image::imageops;
use image::{ImageBuffer, ImageFormat, Rgb, RgbImage};
use tempfile::tempdir;

fn test_options() -> RobustWatermarkOptions {
    RobustWatermarkOptions {
        key: Some("test-secret".to_string()),
        strength: 0.25,
        tile_size: 512,
        overlap: 0.0,
        cross_band_count: 3,
        ..RobustWatermarkOptions::default()
    }
}

fn fixture_image(width: u32, height: u32) -> RgbImage {
    ImageBuffer::from_fn(width, height, |x, y| {
        let r = 40 + ((x * 170) / width) as u8;
        let g = 50 + ((y * 150) / height) as u8;
        let b = 90 + (((x + y) * 90) / (width + height)) as u8;
        Rgb([r, g, b])
    })
}

fn encode_fixture(id: &str) -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempdir().expect("tempdir");
    let input = dir.path().join("input.png");
    let output = dir.path().join("marked.png");
    fixture_image(1024, 1024).save(&input).expect("save input");
    encode_image(
        &input,
        &output,
        id,
        EncodeOptions {
            watermark: test_options(),
            jpeg_quality: None,
            ..Default::default()
        },
    )
    .expect("encode");
    (dir, output)
}

#[test]
fn default_preset_keeps_psnr_high() {
    let dir = tempdir().expect("tempdir");
    let input = dir.path().join("input.png");
    let output = dir.path().join("marked.png");
    fixture_image(1024, 1024).save(&input).expect("save input");
    let report = encode_image(
        &input,
        &output,
        "asset-123",
        EncodeOptions {
            watermark: test_options(),
            jpeg_quality: None,
            ..Default::default()
        },
    )
    .expect("encode");
    assert!(report.psnr >= 40.0, "psnr={}", report.psnr);
    assert_eq!(report.algorithm, "frequency_v2");
}

#[test]
fn encodes_and_decodes_png() {
    let (_dir, marked) = encode_fixture("asset-123");
    let report = decode_image(
        marked,
        DecodeOptions {
            watermark: test_options(),
            enable_diagnostics: false,
            ..Default::default()
        },
    )
    .expect("decode");
    assert_eq!(report.status, DecodeStatus::Decoded);
    assert_eq!(report.id.as_deref(), Some("asset-123"));
    assert!(report.confidence > 0.1);
    assert!(report.tile_hits >= 1);
}

#[test]
fn wrong_key_does_not_decode() {
    let (_dir, marked) = encode_fixture("asset-123");
    let mut options = test_options();
    options.key = Some("wrong-secret".to_string());
    let report = decode_image(
        marked,
        DecodeOptions {
            watermark: options,
            enable_diagnostics: false,
            ..Default::default()
        },
    )
    .expect("decode");
    assert_ne!(report.status, DecodeStatus::Decoded);
    assert!(report.id.is_none());
}

#[test]
fn decodes_after_jpeg_reencode() {
    let (dir, marked) = encode_fixture("asset-123");
    let jpeg = dir.path().join("reencoded.jpg");
    image::open(&marked)
        .expect("open marked")
        .save_with_format(&jpeg, ImageFormat::Jpeg)
        .expect("save jpeg");
    let report = decode_image(
        jpeg,
        DecodeOptions {
            watermark: test_options(),
            enable_diagnostics: false,
            ..Default::default()
        },
    )
    .expect("decode");
    assert_eq!(report.id.as_deref(), Some("asset-123"));
}

#[test]
fn decodes_after_cardinal_rotation() {
    let (dir, marked) = encode_fixture("asset-123");
    let image = image::open(&marked).expect("open").to_rgb8();
    let rotated_path = dir.path().join("rotated.png");
    imageops::rotate90(&image)
        .save(&rotated_path)
        .expect("save");
    let report = decode_image(
        rotated_path,
        DecodeOptions {
            watermark: test_options(),
            enable_diagnostics: false,
            ..Default::default()
        },
    )
    .expect("decode");
    assert_eq!(report.id.as_deref(), Some("asset-123"));
}

#[test]
fn decodes_after_quarter_area_crop() {
    let (dir, marked) = encode_fixture("asset-123");
    let image = image::open(&marked).expect("open").to_rgb8();
    let crop_path = dir.path().join("crop.png");
    let crop = imageops::crop_imm(&image, 0, 0, 512, 512).to_image();
    crop.save(&crop_path).expect("save crop");
    let report = decode_image(
        crop_path,
        DecodeOptions {
            watermark: test_options(),
            enable_diagnostics: false,
            ..Default::default()
        },
    )
    .expect("decode crop");
    assert_eq!(report.id.as_deref(), Some("asset-123"));
}

#[test]
fn capacity_rejects_small_images() {
    let dir = tempdir().expect("tempdir");
    let input = dir.path().join("small.png");
    fixture_image(64, 64).save(&input).expect("save input");
    let err = estimate_capacity(input, test_options()).expect_err("small image should fail");
    assert!(err.to_string().contains("smaller than tile size"));
}

// --- jpeg-dct backend tests ---

fn encode_fixture_jpeg(id: &str) -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempdir().expect("tempdir");
    let input = dir.path().join("input.png");
    let output = dir.path().join("marked.jpg");
    fixture_image(1024, 1024).save(&input).expect("save input");
    encode_image(
        &input,
        &output,
        id,
        EncodeOptions {
            watermark: test_options(),
            jpeg_quality: Some(95),
            backend: BackendChoice::JpegDct,
        },
    )
    .expect("encode");
    (dir, output)
}

#[test]
fn jpeg_dct_encodes_and_decodes() {
    let (_dir, marked) = encode_fixture_jpeg("asset-jpeg-1");
    let report = decode_image(
        &marked,
        DecodeOptions {
            watermark: test_options(),
            enable_diagnostics: false,
            backend: BackendChoice::JpegDct,
        },
    )
    .expect("decode");
    assert_eq!(report.status, DecodeStatus::Decoded);
    assert_eq!(report.id.as_deref(), Some("asset-jpeg-1"));
    assert!(report.confidence > 0.05);
    assert!(report.tile_hits >= 1);
    assert_eq!(report.best_rotation_degrees, 0);
}

#[test]
fn jpeg_dct_wrong_key_does_not_decode() {
    let (_dir, marked) = encode_fixture_jpeg("asset-jpeg-1");
    let mut options = test_options();
    options.key = Some("wrong-secret".to_string());
    let report = decode_image(
        &marked,
        DecodeOptions {
            watermark: options,
            enable_diagnostics: false,
            backend: BackendChoice::JpegDct,
        },
    )
    .expect("decode");
    assert_ne!(report.status, DecodeStatus::Decoded);
    assert!(report.id.is_none());
}

#[test]
fn jpeg_dct_psnr_acceptable() {
    let dir = tempdir().expect("tempdir");
    let input = dir.path().join("input.png");
    let output = dir.path().join("marked.jpg");
    fixture_image(1024, 1024).save(&input).expect("save input");
    let report = encode_image(
        &input,
        &output,
        "asset-123",
        EncodeOptions {
            watermark: test_options(),
            jpeg_quality: Some(95),
            backend: BackendChoice::JpegDct,
        },
    )
    .expect("encode");
    assert!(report.psnr >= 35.0, "psnr={}", report.psnr);
    assert_eq!(report.algorithm, "jpeg_dct");
    assert_eq!(report.backend, "jpeg_dct");
}

#[test]
fn jpeg_dct_decodes_after_cardinal_rotation() {
    let (dir, marked) = encode_fixture_jpeg("asset-jpeg-rot");
    let image = image::open(&marked).expect("open").to_rgb8();
    let rotated_path = dir.path().join("rotated.jpg");
    imageops::rotate90(&image)
        .save_with_format(&rotated_path, ImageFormat::Jpeg)
        .expect("save");
    let report = decode_image(
        &rotated_path,
        DecodeOptions {
            watermark: test_options(),
            enable_diagnostics: false,
            backend: BackendChoice::JpegDct,
        },
    )
    .expect("decode");
    assert_eq!(report.id.as_deref(), Some("asset-jpeg-rot"));
}

#[test]
fn auto_backend_selects_jpeg_dct_for_jpg_output() {
    let dir = tempdir().expect("tempdir");
    let input = dir.path().join("input.png");
    let output = dir.path().join("marked.jpg");
    fixture_image(1024, 1024).save(&input).expect("save input");
    let report = encode_image(
        &input,
        &output,
        "auto-test",
        EncodeOptions {
            watermark: test_options(),
            jpeg_quality: Some(95),
            backend: BackendChoice::Auto,
        },
    )
    .expect("encode");
    assert_eq!(report.backend, "jpeg_dct");
}

#[test]
fn auto_backend_selects_frequency_v2_for_png_output() {
    let dir = tempdir().expect("tempdir");
    let input = dir.path().join("input.png");
    let output = dir.path().join("marked.png");
    fixture_image(1024, 1024).save(&input).expect("save input");
    let report = encode_image(
        &input,
        &output,
        "auto-test",
        EncodeOptions {
            watermark: test_options(),
            jpeg_quality: None,
            backend: BackendChoice::Auto,
        },
    )
    .expect("encode");
    assert_eq!(report.backend, "frequency_v2");
}

#[test]
fn auto_decode_finds_jpeg_dct_watermark() {
    let (_dir, marked) = encode_fixture_jpeg("auto-decode-test");
    let report = decode_image(
        &marked,
        DecodeOptions {
            watermark: test_options(),
            enable_diagnostics: false,
            backend: BackendChoice::Auto,
        },
    )
    .expect("decode");
    assert_eq!(report.status, DecodeStatus::Decoded);
    assert_eq!(report.id.as_deref(), Some("auto-decode-test"));
}
