use image::imageops;
use image::{GrayImage, Luma, RgbImage};
use imageproc::binary_descriptors::brief::{BriefDescriptor, brief};
use imageproc::binary_descriptors::match_binary_descriptors;
use imageproc::corners::{OrientedFastCorner, oriented_fast};
use imageproc::geometric_transformations::{Interpolation, Projection, warp};
use imageproc::point::Point;

/// Alignment result
#[derive(Debug, Clone)]
pub struct AlignmentResult {
    pub rotation: f64,
    pub scale: f64,
    pub shift_x: f64,
    pub shift_y: f64,
    pub score: f64,
    /// 0.0 = completely unreliable, 1.0 = perfect alignment
    pub confidence: f64,
    /// Row-major 3x3 projective transform matrix (ORB result), None when using fallback
    pub transform: Option<[f32; 9]>,
}

/// Align suspect image to original using ORB feature matching,
/// falling back to template matching when features are insufficient.
pub fn align_images(original: &RgbImage, suspect: &RgbImage) -> (RgbImage, AlignmentResult) {
    let (orb_aligned, orb_result) = align_with_orb(original, suspect);

    if orb_result.confidence >= 0.3 {
        return (orb_aligned, orb_result);
    }

    let (w, _h) = original.dimensions();
    let (w_sus, h_sus) = suspect.dimensions();
    let suspect_resized = if (w_sus, h_sus) != (w, _h) {
        resize_rgb(suspect, w, _h)
    } else {
        suspect.clone()
    };

    let aligned = fine_align(original, &suspect_resized);

    let score = compute_alignment_score(&rgb_to_gray(original), &rgb_to_gray(&aligned));
    let confidence = if score > 0.5 { 0.4 } else { 0.1 };

    let result = AlignmentResult {
        rotation: 0.0,
        scale: 1.0,
        shift_x: 0.0,
        shift_y: 0.0,
        score,
        confidence,
        transform: None,
    };

    (aligned, result)
}

fn align_with_orb(original: &RgbImage, suspect: &RgbImage) -> (RgbImage, AlignmentResult) {
    let (w, h) = original.dimensions();

    let gray_orig = rgb_to_gray(original);
    let gray_suspect = rgb_to_gray(suspect);

    let mut orig_corners: Vec<OrientedFastCorner> =
        oriented_fast(&gray_orig, Some(20), 200, 17, Some(42));
    let mut suspect_corners: Vec<OrientedFastCorner> =
        oriented_fast(&gray_suspect, Some(20), 200, 17, Some(42));

    // Sort by score descending, keep top N
    orig_corners.sort_by(|a, b| b.corner.score.partial_cmp(&a.corner.score).unwrap());
    suspect_corners.sort_by(|a, b| b.corner.score.partial_cmp(&a.corner.score).unwrap());
    let max_corners = 100;
    orig_corners.truncate(max_corners);
    suspect_corners.truncate(max_corners);

    if orig_corners.len() < 8 || suspect_corners.len() < 8 {
        return fallback_result(suspect, w, h);
    }

    let orig_points: Vec<Point<u32>> = orig_corners
        .iter()
        .map(|c| Point::new(c.corner.x, c.corner.y))
        .collect();
    let suspect_points: Vec<Point<u32>> = suspect_corners
        .iter()
        .map(|c| Point::new(c.corner.x, c.corner.y))
        .collect();

    let desc_length = 256;
    let (orig_descs, test_pairs) = match brief(&gray_orig, &orig_points, desc_length, None) {
        Ok(d) => d,
        Err(_) => return fallback_result(suspect, w, h),
    };
    let (suspect_descs, _) = match brief(
        &gray_suspect,
        &suspect_points,
        desc_length,
        Some(&test_pairs),
    ) {
        Ok(d) => d,
        Err(_) => return fallback_result(suspect, w, h),
    };

    let matches = match_binary_descriptors(&orig_descs, &suspect_descs, 48, Some(42));

    if matches.len() < 4 {
        return fallback_result(suspect, w, h);
    }

    let mut src_pts: Vec<(f32, f32)> = Vec::new();
    let mut dst_pts: Vec<(f32, f32)> = Vec::new();

    for (orig_desc, suspect_desc) in &matches {
        let oi = find_corner_idx(orig_desc, &orig_descs);
        let si = find_corner_idx(suspect_desc, &suspect_descs);
        if let (Some(oi), Some(si)) = (oi, si) {
            src_pts.push((orig_points[oi].x as f32, orig_points[oi].y as f32));
            dst_pts.push((suspect_points[si].x as f32, suspect_points[si].y as f32));
        }
    }

    if src_pts.len() < 4 {
        return fallback_result(suspect, w, h);
    }

    let (best_transform, inlier_count) = ransac_homography(&src_pts, &dst_pts, 200, 5.0);

    let inlier_ratio = inlier_count as f64 / src_pts.len() as f64;

    if inlier_ratio < 0.25 || best_transform.is_none() {
        return fallback_result(suspect, w, h);
    }

    let transform = best_transform.unwrap();

    // Invert: we want suspect -> original mapping
    // The transform from control points maps suspect -> original, so use directly
    let aligned = warp(
        suspect,
        &transform,
        Interpolation::Bilinear,
        image::Rgb([0u8, 0u8, 0u8]),
    );

    // Resize to original dimensions if needed
    let (aw, ah) = aligned.dimensions();
    let aligned = if (aw, ah) != (w, h) {
        resize_rgb(&aligned, w, h)
    } else {
        aligned
    };

    let rotation = estimate_rotation(&transform);
    let scale = estimate_scale(&transform);
    let confidence = (inlier_ratio * 0.7 + (matches.len() as f64 / 50.0).min(0.3)).min(1.0);
    let score = compute_alignment_score(&gray_orig, &rgb_to_gray(&aligned));

    (
        aligned,
        AlignmentResult {
            rotation,
            scale,
            shift_x: 0.0,
            shift_y: 0.0,
            score,
            confidence,
            transform: Some(get_matrix(&transform)),
        },
    )
}

fn fallback_result(image: &RgbImage, w: u32, h: u32) -> (RgbImage, AlignmentResult) {
    (
        resize_rgb(image, w, h),
        AlignmentResult {
            rotation: 0.0,
            scale: 1.0,
            shift_x: 0.0,
            shift_y: 0.0,
            score: 0.0,
            confidence: 0.0,
            transform: None,
        },
    )
}

fn ransac_homography(
    src: &[(f32, f32)],
    dst: &[(f32, f32)],
    iterations: usize,
    threshold: f32,
) -> (Option<Projection>, usize) {
    if src.len() < 4 {
        return (None, 0);
    }

    let mut best_inliers = 0usize;
    let mut best_transform: Option<Projection> = None;
    let n = src.len();

    for _ in 0..iterations {
        let mut indices: Vec<usize> = (0..n).collect();
        for i in 0..4.min(n) {
            let j = i + fastrand::usize(0..n - i);
            indices.swap(i, j);
        }
        let (i0, i1, i2, i3) = (indices[0], indices[1], indices[2], indices[3]);

        let from_pts = [src[i0], src[i1], src[i2], src[i3]];
        let to_pts = [dst[i0], dst[i1], dst[i2], dst[i3]];

        if let Some(proj) = Projection::from_control_points(from_pts, to_pts) {
            let mut inliers = 0;
            for k in 0..n {
                let p = proj * src[k];
                let dx = p.0 - dst[k].0;
                let dy = p.1 - dst[k].1;
                if dx * dx + dy * dy < threshold * threshold {
                    inliers += 1;
                }
            }
            if inliers > best_inliers {
                best_inliers = inliers;
                best_transform = Some(proj);
            }
        }
    }

    (best_transform, best_inliers)
}

fn estimate_rotation(proj: &Projection) -> f64 {
    let m = get_matrix(proj);
    let a = m[0];
    let b = m[1];
    if a.abs() < 1e-10 && b.abs() < 1e-10 {
        return 0.0;
    }
    (-b).atan2(a).to_degrees() as f64
}

fn estimate_scale(proj: &Projection) -> f64 {
    let m = get_matrix(proj);
    let a = m[0] as f64;
    let b = m[1] as f64;
    let c = m[3] as f64;
    let d = m[4] as f64;
    let sx = (a * a + b * b).sqrt();
    let sy = (c * c + d * d).sqrt();
    if sx < 1e-10 || sy < 1e-10 {
        return 1.0;
    }
    (sx + sy) / 2.0
}

/// Extract the 3x3 matrix from a Projection by testing on basis vectors.
pub(crate) fn get_matrix(proj: &Projection) -> [f32; 9] {
    let p = *proj;
    let e00 = p * (0.0f32, 0.0f32);
    let e10 = p * (1.0f32, 0.0f32);
    let e01 = p * (0.0f32, 1.0f32);

    // For an affine/projective transform: p' = M * p
    // M = [a b tx; c d ty; 0 0 1] for points (x, y, 1)
    // p' = (a*x + b*y + tx, c*x + d*y + ty) / (g*x + h*y + 1)
    // For simplicity, we assume affine (g=h=0)
    let tx = e00.0;
    let ty = e00.1;
    let a = e10.0 - tx;
    let c = e10.1 - ty;
    let b = e01.0 - tx;
    let d = e01.1 - ty;

    [a, b, tx, c, d, ty, 0.0, 0.0, 1.0]
}

fn find_corner_idx(desc: &BriefDescriptor, descs: &[BriefDescriptor]) -> Option<usize> {
    descs.iter().position(|d| std::ptr::eq(d, desc))
}

fn compute_alignment_score(gray1: &GrayImage, gray2: &GrayImage) -> f64 {
    let (w, h) = gray1.dimensions();
    let margin = w.min(h) / 8;
    if margin >= w / 2 || margin >= h / 2 {
        return 0.0;
    }

    let mut sum1 = 0.0f64;
    let mut sum2 = 0.0f64;
    let mut sum11 = 0.0f64;
    let mut sum22 = 0.0f64;
    let mut sum12 = 0.0f64;
    let mut count = 0u64;

    for y in margin..(h - margin) {
        for x in margin..(w - margin) {
            let v1 = gray1.get_pixel(x, y)[0] as f64;
            let v2 = gray2.get_pixel(x, y)[0] as f64;
            sum1 += v1;
            sum2 += v2;
            sum11 += v1 * v1;
            sum22 += v2 * v2;
            sum12 += v1 * v2;
            count += 1;
        }
    }

    let n = count as f64;
    if n < 1.0 {
        return 0.0;
    }
    let mean1 = sum1 / n;
    let mean2 = sum2 / n;
    let var1 = (sum11 / n - mean1 * mean1).sqrt().max(1e-10);
    let var2 = (sum22 / n - mean2 * mean2).sqrt().max(1e-10);
    let cov = sum12 / n - mean1 * mean2;

    cov / (var1 * var2)
}

/// Normalize histogram of suspect to match original
pub fn normalize_histogram(original: &RgbImage, suspect: &RgbImage) -> RgbImage {
    let mut result = suspect.clone();

    for c in 0..3 {
        let orig_channel: Vec<f64> = original.pixels().map(|p| p[c] as f64).collect();
        let suspect_channel: Vec<f64> = suspect.pixels().map(|p| p[c] as f64).collect();

        let orig_mean = orig_channel.iter().sum::<f64>() / orig_channel.len() as f64;
        let orig_std = (orig_channel
            .iter()
            .map(|&x| (x - orig_mean).powi(2))
            .sum::<f64>()
            / orig_channel.len() as f64)
            .sqrt();
        let suspect_mean = suspect_channel.iter().sum::<f64>() / suspect_channel.len() as f64;
        let suspect_std = (suspect_channel
            .iter()
            .map(|&x| (x - suspect_mean).powi(2))
            .sum::<f64>()
            / suspect_channel.len() as f64)
            .sqrt();

        if suspect_std > 1e-10 {
            for (i, pixel) in result.pixels_mut().enumerate() {
                let normalized =
                    (suspect_channel[i] - suspect_mean) * (orig_std / suspect_std) + orig_mean;
                let mut p = *pixel;
                p[c] = normalized.clamp(0.0, 255.0) as u8;
                *pixel = p;
            }
        }
    }

    result
}

fn rgb_to_gray(image: &RgbImage) -> GrayImage {
    let (w, h) = image.dimensions();
    let mut gray = GrayImage::new(w, h);
    for y in 0..h {
        for x in 0..w {
            let p = image.get_pixel(x, y);
            let v = (0.299 * p[0] as f32 + 0.587 * p[1] as f32 + 0.114 * p[2] as f32) as u8;
            gray.put_pixel(x, y, Luma([v]));
        }
    }
    gray
}

fn resize_rgb(image: &RgbImage, new_w: u32, new_h: u32) -> RgbImage {
    imageops::resize(image, new_w, new_h, imageops::FilterType::Lanczos3)
}

fn to_gray_f32(image: &RgbImage) -> Vec<f32> {
    image
        .pixels()
        .map(|p| 0.299 * p[0] as f32 + 0.587 * p[1] as f32 + 0.114 * p[2] as f32)
        .collect()
}

fn template_match_score(template: &[f32], image: &[f32]) -> f64 {
    let n = template.len() as f64;
    let mean_t = template.iter().sum::<f32>() as f64 / n;
    let mean_i = image.iter().sum::<f32>() as f64 / n;

    let std_t = (template
        .iter()
        .map(|&x| (x as f64 - mean_t).powi(2))
        .sum::<f64>()
        / n)
        .sqrt();
    let std_i = (image
        .iter()
        .map(|&x| (x as f64 - mean_i).powi(2))
        .sum::<f64>()
        / n)
        .sqrt();

    if std_t < 1e-10 || std_i < 1e-10 {
        return 0.0;
    }

    let correlation: f64 = template
        .iter()
        .zip(image.iter())
        .map(|(&t, &i)| (t as f64 - mean_t) * (i as f64 - mean_i))
        .sum::<f64>()
        / (n * std_t * std_i);

    correlation
}

fn fine_align(original: &RgbImage, suspect: &RgbImage) -> RgbImage {
    let (w, h) = original.dimensions();
    let gray_orig = to_gray_f32(original);
    let gray_suspect = to_gray_f32(suspect);

    let margin = 40;
    let template_w = w - 2 * margin;
    let template_h = h - 2 * margin;

    if template_w == 0 || template_h == 0 {
        return suspect.clone();
    }

    let mut template = Vec::new();
    for y in margin..(h - margin) {
        for x in margin..(w - margin) {
            template.push(gray_orig[(y * w + x) as usize]);
        }
    }

    let search_range = 10i32;
    let mut best_x = 0i32;
    let mut best_y = 0i32;
    let mut best_score = -1.0f64;

    let mut dy = -search_range;
    while dy <= search_range {
        let mut dx = -search_range;
        while dx <= search_range {
            let mut image_patch = Vec::new();
            let mut valid = true;

            for y in 0..template_h {
                for x in 0..template_w {
                    let src_x = (margin as i32 + dx + x as i32) as u32;
                    let src_y = (margin as i32 + dy + y as i32) as u32;

                    if src_x >= w || src_y >= h {
                        valid = false;
                        break;
                    }
                    image_patch.push(gray_suspect[(src_y * w + src_x) as usize]);
                }
                if !valid {
                    break;
                }
            }

            if valid {
                let score = template_match_score(&template, &image_patch);
                if score > best_score {
                    best_score = score;
                    best_x = dx;
                    best_y = dy;
                }
            }
            dx += 2;
        }
        dy += 2;
    }

    for dy in (best_y - 2)..=(best_y + 2) {
        for dx in (best_x - 2)..=(best_x + 2) {
            if dy < -search_range || dy > search_range || dx < -search_range || dx > search_range {
                continue;
            }
            let mut image_patch = Vec::new();
            let mut valid = true;

            for y in 0..template_h {
                for x in 0..template_w {
                    let src_x = (margin as i32 + dx + x as i32) as u32;
                    let src_y = (margin as i32 + dy + y as i32) as u32;

                    if src_x >= w || src_y >= h {
                        valid = false;
                        break;
                    }
                    image_patch.push(gray_suspect[(src_y * w + src_x) as usize]);
                }
                if !valid {
                    break;
                }
            }

            if valid {
                let score = template_match_score(&template, &image_patch);
                if score > best_score {
                    best_score = score;
                    best_x = dx;
                    best_y = dy;
                }
            }
        }
    }

    let mut result = RgbImage::new(w, h);
    for y in 0..h {
        for x in 0..w {
            let src_x = (x as i32 - best_x).clamp(0, w as i32 - 1) as u32;
            let src_y = (y as i32 - best_y).clamp(0, h as i32 - 1) as u32;
            result.put_pixel(x, y, *suspect.get_pixel(src_x, src_y));
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::RgbImage;

    fn create_test_image(w: u32, h: u32) -> RgbImage {
        let mut img = RgbImage::new(w, h);
        for y in 0..h {
            for x in 0..w {
                let v = ((x * 7 + y * 13) % 256) as u8;
                img.put_pixel(x, y, image::Rgb([v, v, v]));
            }
        }
        img
    }

    #[test]
    fn test_orb_align_same_image() {
        let img = create_test_image(256, 256);
        let (_, _result) = align_with_orb(&img, &img);
        // Synthetic images may have few features; this just ensures no crash
    }
}
