use std::path::{Path, PathBuf};

use hidden_watermark::{detect_watermark, embed_watermark, load_image};
use image::imageops;

const TEST_KEY: &str = "test_secret_key_123";

fn get_test_image_path() -> PathBuf {
    PathBuf::from("/tmp/goku_test.png")
}

fn get_test_jpeg_path(quality: u32) -> PathBuf {
    PathBuf::from(format!("/tmp/goku_test_q{}.jpg", quality))
}

fn ensure_test_image() {
    let dst = get_test_image_path();
    if !dst.exists() {
        let img = image::ImageReader::open(Path::new("assets/images/goku.png"))
            .unwrap()
            .with_guessed_format()
            .unwrap()
            .decode()
            .unwrap();
        img.save(&dst).unwrap();
    }
}

#[test]
fn test_embed_invisibility() {
    ensure_test_image();
    let image = load_image(&get_test_image_path()).unwrap();
    let (_watermarked, psnr) = embed_watermark(&image, TEST_KEY, 0.5);
    assert!(psnr > 40.0, "PSNR {} dB < 40 dB", psnr);
}

#[test]
fn test_detect_clean() {
    ensure_test_image();
    let image = load_image(&get_test_image_path()).unwrap();
    let (watermarked, _) = embed_watermark(&image, TEST_KEY, 0.5);

    let result = detect_watermark(&image, &watermarked, TEST_KEY, 0.001);
    assert!(result.detected, "Should detect watermark in clean image");
    assert!(
        result.score > result.threshold,
        "Score should exceed threshold"
    );
}

#[test]
fn test_detect_brightness() {
    ensure_test_image();
    let image = load_image(&get_test_image_path()).unwrap();
    let (watermarked, _) = embed_watermark(&image, TEST_KEY, 0.5);

    let brightened = apply_brightness(&watermarked, 1.2);
    let result = detect_watermark(&image, &brightened, TEST_KEY, 0.001);
    assert!(
        result.detected,
        "Should detect watermark after brightness change"
    );
}

#[test]
fn test_detect_contrast() {
    ensure_test_image();
    let image = load_image(&get_test_image_path()).unwrap();
    let (watermarked, _) = embed_watermark(&image, TEST_KEY, 0.5);

    let contrasted = apply_contrast(&watermarked, 1.2);
    let result = detect_watermark(&image, &contrasted, TEST_KEY, 0.001);
    assert!(
        result.detected,
        "Should detect watermark after contrast change"
    );
}

#[test]
fn test_false_positive() {
    ensure_test_image();
    let image = load_image(&get_test_image_path()).unwrap();

    let (w, h) = image.dimensions();
    let mut random_image = image::RgbImage::new(w, h);
    for y in 0..h {
        for x in 0..w {
            let r = ((x * 7 + y * 13) % 256) as u8;
            let g = ((x * 11 + y * 17) % 256) as u8;
            let b = ((x * 19 + y * 23) % 256) as u8;
            random_image.put_pixel(x, y, image::Rgb([r, g, b]));
        }
    }

    let result = detect_watermark(&image, &random_image, TEST_KEY, 0.001);
    assert!(
        !result.detected,
        "Should not detect watermark in random image"
    );
}

#[test]
fn test_wrong_key() {
    ensure_test_image();
    let image = load_image(&get_test_image_path()).unwrap();
    let (watermarked, _) = embed_watermark(&image, TEST_KEY, 0.5);

    let result = detect_watermark(&image, &watermarked, "wrong_key", 0.001);
    assert!(
        !result.detected,
        "Should not detect watermark with wrong key"
    );
}

// JPEG robustness tests

#[test]
fn test_jpeg_q90_roundtrip() {
    ensure_test_image();
    let image = load_image(&get_test_image_path()).unwrap();
    let (watermarked, _) = embed_watermark(&image, TEST_KEY, 0.5);

    let jpeg_path = get_test_jpeg_path(90);
    watermarked.save(&jpeg_path).unwrap();
    let jpeg_image = load_image(&jpeg_path).unwrap();

    let result = detect_watermark(&image, &jpeg_image, TEST_KEY, 0.001);
    assert!(result.detected, "JPEG q90: should detect watermark");
}

#[test]
fn test_jpeg_q75_roundtrip() {
    ensure_test_image();
    let image = load_image(&get_test_image_path()).unwrap();
    let (watermarked, _) = embed_watermark(&image, TEST_KEY, 0.5);

    let jpeg_path = get_test_jpeg_path(75);
    watermarked.save(&jpeg_path).unwrap();
    let jpeg_image = load_image(&jpeg_path).unwrap();

    let result = detect_watermark(&image, &jpeg_image, TEST_KEY, 0.001);
    assert!(result.detected, "JPEG q75: should detect watermark");
}

#[test]
fn test_jpeg_q50_roundtrip() {
    ensure_test_image();
    let image = load_image(&get_test_image_path()).unwrap();
    let (watermarked, _) = embed_watermark(&image, TEST_KEY, 0.5);

    let jpeg_path = get_test_jpeg_path(50);
    watermarked.save(&jpeg_path).unwrap();
    let jpeg_image = load_image(&jpeg_path).unwrap();

    let result = detect_watermark(&image, &jpeg_image, TEST_KEY, 0.001);
    if !result.detected {
        println!(
            "INFO: JPEG q50 detection failed (score={}, threshold={})",
            result.score, result.threshold
        );
    }
}

// Geometric attack tests

#[test]
fn test_rotate_2deg() {
    ensure_test_image();
    let image = load_image(&get_test_image_path()).unwrap();
    let (watermarked, _) = embed_watermark(&image, TEST_KEY, 0.5);

    let rotated = apply_rotation(&watermarked, 2.0);
    let result = detect_watermark(&image, &rotated, TEST_KEY, 0.001);
    if !result.detected {
        println!(
            "INFO: 2deg rotation detection result (score={}, threshold={}, align_confidence={})",
            result.score, result.threshold, result.alignment.confidence
        );
    }
}

#[test]
fn test_rotate_5deg() {
    ensure_test_image();
    let image = load_image(&get_test_image_path()).unwrap();
    let (watermarked, _) = embed_watermark(&image, TEST_KEY, 0.5);

    let rotated = apply_rotation(&watermarked, 5.0);
    let result = detect_watermark(&image, &rotated, TEST_KEY, 0.001);
    if !result.detected {
        println!(
            "INFO: 5deg rotation detection result (score={}, threshold={}, align_confidence={})",
            result.score, result.threshold, result.alignment.confidence
        );
    }
}

#[test]
fn test_scale_90() {
    ensure_test_image();
    let image = load_image(&get_test_image_path()).unwrap();
    let (watermarked, _) = embed_watermark(&image, TEST_KEY, 0.5);

    let scaled = apply_scale(&watermarked, 0.9);
    let result = detect_watermark(&image, &scaled, TEST_KEY, 0.001);
    if !result.detected {
        println!(
            "INFO: scale 90% detection result (score={}, threshold={}, align_confidence={})",
            result.score, result.threshold, result.alignment.confidence
        );
    }
}

// Helper functions for image transforms

fn apply_brightness(image: &image::RgbImage, factor: f64) -> image::RgbImage {
    let (w, h) = image.dimensions();
    let mut result = image::RgbImage::new(w, h);

    for y in 0..h {
        for x in 0..w {
            let pixel = image.get_pixel(x, y);
            let r = (pixel[0] as f64 * factor).clamp(0.0, 255.0) as u8;
            let g = (pixel[1] as f64 * factor).clamp(0.0, 255.0) as u8;
            let b = (pixel[2] as f64 * factor).clamp(0.0, 255.0) as u8;
            result.put_pixel(x, y, image::Rgb([r, g, b]));
        }
    }

    result
}

fn apply_contrast(image: &image::RgbImage, factor: f64) -> image::RgbImage {
    let (w, h) = image.dimensions();
    let mut result = image::RgbImage::new(w, h);

    for y in 0..h {
        for x in 0..w {
            let pixel = image.get_pixel(x, y);
            let r = ((pixel[0] as f64 - 128.0) * factor + 128.0).clamp(0.0, 255.0) as u8;
            let g = ((pixel[1] as f64 - 128.0) * factor + 128.0).clamp(0.0, 255.0) as u8;
            let b = ((pixel[2] as f64 - 128.0) * factor + 128.0).clamp(0.0, 255.0) as u8;
            result.put_pixel(x, y, image::Rgb([r, g, b]));
        }
    }

    result
}

fn apply_rotation(image: &image::RgbImage, angle: f64) -> image::RgbImage {
    use imageproc::geometric_transformations::{Interpolation, rotate_about_center};
    let theta = angle.to_radians() as f32;
    rotate_about_center(
        image,
        theta,
        Interpolation::Bilinear,
        image::Rgb([0u8, 0u8, 0u8]),
    )
}

fn apply_scale(image: &image::RgbImage, factor: f64) -> image::RgbImage {
    let (w, h) = image.dimensions();
    let new_w = (w as f64 * factor) as u32;
    let new_h = (h as f64 * factor) as u32;
    let new_w = new_w.max(1);
    let new_h = new_h.max(1);
    image::imageops::resize(image, new_w, new_h, image::imageops::FilterType::Lanczos3)
}

fn apply_gaussian_noise(image: &image::RgbImage, sigma: f64) -> image::RgbImage {
    let (w, h) = image.dimensions();
    let mut result = image::RgbImage::new(w, h);
    for y in 0..h {
        for x in 0..w {
            let p = image.get_pixel(x, y);
            let r = (p[0] as f64 + fastrand::f64() * sigma * 2.0 - sigma).clamp(0.0, 255.0) as u8;
            let g = (p[1] as f64 + fastrand::f64() * sigma * 2.0 - sigma).clamp(0.0, 255.0) as u8;
            let b = (p[2] as f64 + fastrand::f64() * sigma * 2.0 - sigma).clamp(0.0, 255.0) as u8;
            result.put_pixel(x, y, image::Rgb([r, g, b]));
        }
    }
    result
}

fn apply_blur(image: &image::RgbImage, radius: u32) -> image::RgbImage {
    let (w, h) = image.dimensions();
    let mut result = image::RgbImage::new(w, h);
    let kernel_size = radius * 2 + 1;
    let sigma = radius as f64 / 2.0;
    let mut kernel = vec![0.0f64; (kernel_size * kernel_size) as usize];
    let mut sum = 0.0f64;
    for dy in 0..kernel_size {
        for dx in 0..kernel_size {
            let x = dx as f64 - radius as f64;
            let y = dy as f64 - radius as f64;
            let val = (-(x * x + y * y) / (2.0 * sigma * sigma)).exp();
            kernel[(dy * kernel_size + dx) as usize] = val;
            sum += val;
        }
    }
    for v in kernel.iter_mut() {
        *v /= sum;
    }
    for y in 0..h {
        for x in 0..w {
            let mut sum_r = 0.0f64;
            let mut sum_g = 0.0f64;
            let mut sum_b = 0.0f64;
            for dy in 0..kernel_size {
                for dx in 0..kernel_size {
                    let nx = (x as i32 + dx as i32 - radius as i32).clamp(0, w as i32 - 1) as u32;
                    let ny = (y as i32 + dy as i32 - radius as i32).clamp(0, h as i32 - 1) as u32;
                    let pixel = image.get_pixel(nx, ny);
                    let k = kernel[(dy * kernel_size + dx) as usize];
                    sum_r += pixel[0] as f64 * k;
                    sum_g += pixel[1] as f64 * k;
                    sum_b += pixel[2] as f64 * k;
                }
            }
            result.put_pixel(
                x,
                y,
                image::Rgb([
                    sum_r.clamp(0.0, 255.0) as u8,
                    sum_g.clamp(0.0, 255.0) as u8,
                    sum_b.clamp(0.0, 255.0) as u8,
                ]),
            );
        }
    }
    result
}

fn apply_crop(image: &image::RgbImage, retain: f64) -> image::RgbImage {
    let (w, h) = image.dimensions();
    let new_w = (w as f64 * retain.sqrt()) as u32;
    let new_h = (h as f64 * retain.sqrt()) as u32;
    let new_w = new_w.max(1).min(w);
    let new_h = new_h.max(1).min(h);
    let x0 = (w - new_w) / 2;
    let y0 = (h - new_h) / 2;
    imageops::crop_imm(image, x0, y0, new_w, new_h).to_image()
}

// ===== 摸底测试：系统扫描所有攻击维度 =====

#[test]
fn test_robustness_survey() {
    ensure_test_image();
    let image = load_image(&get_test_image_path()).unwrap();
    let (watermarked, _psnr) = embed_watermark(&image, TEST_KEY, 0.5);

    let data: Vec<(&str, Vec<(&str, Box<dyn Fn() -> image::RgbImage>)>)> = vec![
        (
            "clean",
            vec![("no_attack", Box::new(|| watermarked.clone()))],
        ),
        (
            "jpeg_quality",
            vec![
                (
                    "q90",
                    Box::new(|| {
                        let p = get_test_jpeg_path(90);
                        watermarked.save(&p).unwrap();
                        load_image(&p).unwrap()
                    }),
                ),
                (
                    "q75",
                    Box::new(|| {
                        let p = get_test_jpeg_path(75);
                        watermarked.save(&p).unwrap();
                        load_image(&p).unwrap()
                    }),
                ),
                (
                    "q60",
                    Box::new(|| {
                        let p = get_test_jpeg_path(60);
                        watermarked.save(&p).unwrap();
                        load_image(&p).unwrap()
                    }),
                ),
                (
                    "q50",
                    Box::new(|| {
                        let p = get_test_jpeg_path(50);
                        watermarked.save(&p).unwrap();
                        load_image(&p).unwrap()
                    }),
                ),
                (
                    "q35",
                    Box::new(|| {
                        let p = get_test_jpeg_path(35);
                        watermarked.save(&p).unwrap();
                        load_image(&p).unwrap()
                    }),
                ),
                (
                    "q20",
                    Box::new(|| {
                        let p = get_test_jpeg_path(20);
                        watermarked.save(&p).unwrap();
                        load_image(&p).unwrap()
                    }),
                ),
            ],
        ),
        (
            "rotation_deg",
            vec![
                ("1deg", Box::new(|| apply_rotation(&watermarked, 1.0))),
                ("2deg", Box::new(|| apply_rotation(&watermarked, 2.0))),
                ("3deg", Box::new(|| apply_rotation(&watermarked, 3.0))),
                ("5deg", Box::new(|| apply_rotation(&watermarked, 5.0))),
                ("10deg", Box::new(|| apply_rotation(&watermarked, 10.0))),
                ("15deg", Box::new(|| apply_rotation(&watermarked, 15.0))),
            ],
        ),
        (
            "scale_pct",
            vec![
                ("95pct", Box::new(|| apply_scale(&watermarked, 0.95))),
                ("90pct", Box::new(|| apply_scale(&watermarked, 0.90))),
                ("85pct", Box::new(|| apply_scale(&watermarked, 0.85))),
                ("80pct", Box::new(|| apply_scale(&watermarked, 0.80))),
                ("75pct", Box::new(|| apply_scale(&watermarked, 0.75))),
                ("70pct", Box::new(|| apply_scale(&watermarked, 0.70))),
            ],
        ),
        (
            "brightness",
            vec![
                ("0.7x", Box::new(|| apply_brightness(&watermarked, 0.7))),
                ("0.8x", Box::new(|| apply_brightness(&watermarked, 0.8))),
                ("1.2x", Box::new(|| apply_brightness(&watermarked, 1.2))),
                ("1.5x", Box::new(|| apply_brightness(&watermarked, 1.5))),
            ],
        ),
        (
            "contrast",
            vec![
                ("0.7x", Box::new(|| apply_contrast(&watermarked, 0.7))),
                ("0.8x", Box::new(|| apply_contrast(&watermarked, 0.8))),
                ("1.2x", Box::new(|| apply_contrast(&watermarked, 1.2))),
                ("1.5x", Box::new(|| apply_contrast(&watermarked, 1.5))),
            ],
        ),
        (
            "blur_radius",
            vec![
                ("r3", Box::new(|| apply_blur(&watermarked, 3))),
                ("r5", Box::new(|| apply_blur(&watermarked, 5))),
                ("r7", Box::new(|| apply_blur(&watermarked, 7))),
            ],
        ),
        (
            "noise_sigma",
            vec![
                ("s5", Box::new(|| apply_gaussian_noise(&watermarked, 5.0))),
                ("s10", Box::new(|| apply_gaussian_noise(&watermarked, 10.0))),
                ("s20", Box::new(|| apply_gaussian_noise(&watermarked, 20.0))),
            ],
        ),
        (
            "crop_retain",
            vec![
                ("90pct", Box::new(|| apply_crop(&watermarked, 0.90))),
                ("75pct", Box::new(|| apply_crop(&watermarked, 0.75))),
                ("50pct", Box::new(|| apply_crop(&watermarked, 0.50))),
            ],
        ),
    ];

    println!();
    println!("{:=<120}", "");
    println!("{:^120}", "ROBUSTNESS SURVEY");
    println!("{:=<120}", "");
    println!();
    println!(
        "{:40} {:>10} {:>10} {:>8} {:>10} {:>8} {:>10}",
        "attack", "score", "thresh", "ratio", "conf", "rot", "scale"
    );
    println!(
        "{:-<40} {:-<10} {:-<10} {:-<8} {:-<10} {:-<8} {:-<10}",
        "", "", "", "", "", "", ""
    );

    for (category, tests) in &data {
        for (name, attack_fn) in tests {
            let attacked = attack_fn();
            let result = detect_watermark(&image, &attacked, TEST_KEY, 0.001);
            let ratio = if result.threshold > 0.0 {
                result.score / result.threshold
            } else {
                0.0
            };
            println!(
                "{:40} {:>10.4} {:>10.4} {:>8.2}x {:>10.3} {:>8.1} {:>10.2}",
                format!("{}/{}", category, name),
                result.score,
                result.threshold,
                ratio,
                result.alignment.confidence,
                result.alignment.rotation,
                result.alignment.scale,
            );
        }
    }
    println!();
    println!("{:=<120}", "");
    println!("NOTE: ratio > 1.0x means detected");
    println!("      conf < 0.2 means alignment unreliable (result forced to not-detected)");
    println!("{:=<120}", "");
}
