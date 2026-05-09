use image::RgbImage;
use rand::SeedableRng;
use rand::seq::SliceRandom;
use rand_chacha::ChaCha20Rng;
use sha2::{Digest, Sha256};

use crate::backend::WatermarkBackend;
use crate::common::{
    DCT_BLOCK, DecodeStatus, DecodedBits, RobustWatermarkOptions, TileAttempt, WatermarkPreset,
    block_variance, cross_band_pairs, cross_band_weights, dct_2d, dct_index, idct_2d,
    normalize_tile, perceptual_mask, read_block8, rgb_to_ycbcr, write_block8, ycbcr_to_rgb,
};
use crate::frame::{
    self, HEADER_BYTES, HeaderStatus, ParseFrameResult, bits_to_bytes, parse_frame,
    parse_frame_header,
};

const JPEG_DCT_ALGORITHM: &str = "jpeg_dct";

pub(crate) struct JpegDctBackend;

impl WatermarkBackend for JpegDctBackend {
    fn name(&self) -> &'static str {
        JPEG_DCT_ALGORITHM
    }

    fn capacity_bits(&self, tile_size: u32, band_count: usize) -> usize {
        jpeg_dct_capacity_bits(tile_size, band_count)
    }

    fn embed(
        &self,
        image: &RgbImage,
        origins: &[(u32, u32)],
        key: &str,
        bits: &[bool],
        options: &RobustWatermarkOptions,
    ) -> RgbImage {
        let band_count = options.cross_band_count;
        let (width, height) = image.dimensions();
        let mut y_channel = Vec::with_capacity((width * height) as usize);
        let mut cb_channel = Vec::with_capacity((width * height) as usize);
        let mut cr_channel = Vec::with_capacity((width * height) as usize);

        for pixel in image.pixels() {
            let (y, cb, cr) = rgb_to_ycbcr(pixel);
            y_channel.push(y);
            cb_channel.push(cb);
            cr_channel.push(cr);
        }

        let margin = jpeg_dct_margin(options);
        let block_plan = jpeg_dct_block_plan(key, bits.len(), options.tile_size, band_count);
        let pairs = cross_band_pairs(band_count);
        let bw = cross_band_weights(band_count);

        for &(origin_x, origin_y) in origins {
            embed_jpeg_dct_tile(
                &mut y_channel,
                width,
                origin_x,
                origin_y,
                &block_plan,
                bits,
                margin,
                pairs,
                bw,
                band_count,
            );
        }

        let mut output = RgbImage::new(width, height);
        for y in 0..height {
            for x in 0..width {
                let idx = (y * width + x) as usize;
                output.put_pixel(
                    x,
                    y,
                    ycbcr_to_rgb(y_channel[idx], cb_channel[idx], cr_channel[idx]),
                );
            }
        }

        output
    }

    fn decode_tile(
        &self,
        gray: &[f32],
        width: u32,
        height: u32,
        origin_x: u32,
        origin_y: u32,
        candidate_tile: u32,
        base_tile_size: u32,
        _scale: f32,
        key: &str,
        _bit_len: usize,
        _band_count: usize,
    ) -> TileAttempt {
        if origin_x + candidate_tile > width || origin_y + candidate_tile > height {
            return TileAttempt::default();
        }
        if candidate_tile != base_tile_size {
            let normalized = normalize_tile(
                gray,
                width,
                origin_x,
                origin_y,
                candidate_tile,
                base_tile_size,
            );
            return self.decode_tile(
                &normalized,
                base_tile_size,
                base_tile_size,
                0,
                0,
                base_tile_size,
                base_tile_size,
                1.0,
                key,
                _bit_len,
                _band_count,
            );
        }

        let header_bits_len = HEADER_BYTES * 8;

        for try_bc in 1..=3 {
            let header_plan = jpeg_dct_block_plan(key, header_bits_len, base_tile_size, try_bc);
            let header_decoded =
                decode_jpeg_dct_bits(gray, width, origin_x, origin_y, &header_plan, try_bc);
            let header_bytes = bits_to_bytes(&header_decoded.bits, HEADER_BYTES);
            let HeaderStatus::Valid(id_len) = parse_frame_header(&header_bytes) else {
                continue;
            };

            let raw_bytes = HEADER_BYTES + id_len + frame::CRC_BYTES;
            let full_bit_len = raw_bytes * 8;
            let full_plan = jpeg_dct_block_plan(key, full_bit_len, base_tile_size, try_bc);
            let decoded = decode_jpeg_dct_bits(gray, width, origin_x, origin_y, &full_plan, try_bc);
            let frame_data = bits_to_bytes(&decoded.bits, raw_bytes);

            match parse_frame(&frame_data) {
                ParseFrameResult::Decoded(id) => {
                    return TileAttempt {
                        id: Some(id),
                        status: DecodeStatus::Decoded,
                        confidence: decoded.confidence,
                        corrected_bytes: 0,
                        bits: decoded.bits,
                        raw_bytes,
                    };
                }
                ParseFrameResult::CrcMismatch => {
                    return TileAttempt {
                        status: DecodeStatus::CrcMismatch,
                        confidence: decoded.confidence,
                        corrected_bytes: 0,
                        bits: decoded.bits,
                        raw_bytes,
                        ..TileAttempt::default()
                    };
                }
                _ => continue,
            }
        }

        TileAttempt {
            status: DecodeStatus::NoWatermark,
            ..TileAttempt::default()
        }
    }
}

fn jpeg_dct_capacity_bits(tile_size: u32, band_count: usize) -> usize {
    if tile_size < DCT_BLOCK || band_count == 0 {
        return 0;
    }
    let blocks_per_axis = tile_size / DCT_BLOCK;
    let total_blocks = (blocks_per_axis * blocks_per_axis) as usize;
    total_blocks / band_count
}

fn jpeg_dct_margin(options: &RobustWatermarkOptions) -> f32 {
    let base = match options.preset {
        WatermarkPreset::Invisible => 200.0,
        WatermarkPreset::Balanced => 260.0,
        WatermarkPreset::Robust => 340.0,
    };
    (base * options.strength).clamp(2.0, 56.0)
}

fn jpeg_dct_block_plan(
    key: &str,
    bit_len: usize,
    tile_size: u32,
    band_count: usize,
) -> Vec<(u32, u32)> {
    let blocks_per_axis = tile_size / DCT_BLOCK;
    let mut positions = Vec::with_capacity((blocks_per_axis * blocks_per_axis) as usize);
    for by in 0..blocks_per_axis {
        for bx in 0..blocks_per_axis {
            positions.push((bx * DCT_BLOCK, by * DCT_BLOCK));
        }
    }
    let mut rng = rng_for_jpeg_dct_plan(key);
    positions.shuffle(&mut rng);
    let needed = bit_len * band_count;
    positions.truncate(needed.min(positions.len()));
    positions
}

fn rng_for_jpeg_dct_plan(key: &str) -> ChaCha20Rng {
    let mut hasher = Sha256::new();
    hasher.update(b"hidden-watermark-jpeg-dct-plan");
    hasher.update(key.as_bytes());
    let digest = hasher.finalize();
    let mut seed = [0_u8; 32];
    seed.copy_from_slice(&digest);
    ChaCha20Rng::from_seed(seed)
}

#[allow(clippy::too_many_arguments)]
fn embed_jpeg_dct_tile(
    y_channel: &mut [f32],
    width: u32,
    origin_x: u32,
    origin_y: u32,
    block_plan: &[(u32, u32)],
    bits: &[bool],
    margin: f32,
    pairs: &[(usize, usize, usize, usize)],
    bw: &[f32],
    band_count: usize,
) {
    for (bit_index, &bit) in bits.iter().enumerate() {
        for band in 0..band_count {
            let plan_idx = bit_index * band_count + band;
            let Some(&(block_x, block_y)) = block_plan.get(plan_idx) else {
                break;
            };
            let abs_x = origin_x + block_x;
            let abs_y = origin_y + block_y;

            let variance = block_variance(y_channel, width, abs_x, abs_y);
            let mask = perceptual_mask(variance);
            if mask <= 0.0 {
                continue;
            }

            let (a_u, a_v, b_u, b_v) = pairs[band];
            let band_margin = margin * bw[band];

            let mut block = read_block8(y_channel, width, abs_x, abs_y);
            dct_2d(&mut block);
            let a_idx = dct_index(a_u, a_v);
            let b_idx = dct_index(b_u, b_v);
            let diff = block[a_idx] - block[b_idx];
            let target = if bit {
                band_margin * mask
            } else {
                -band_margin * mask
            };
            let needs_adjustment = (bit && diff < target) || (!bit && diff > target);
            if needs_adjustment {
                let adjustment = (target - diff) * 0.5;
                block[a_idx] += adjustment;
                block[b_idx] -= adjustment;
            }
            idct_2d(&mut block);
            write_block8(y_channel, width, abs_x, abs_y, &block);
        }
    }
}

fn decode_jpeg_dct_bits(
    gray: &[f32],
    width: u32,
    origin_x: u32,
    origin_y: u32,
    block_plan: &[(u32, u32)],
    band_count: usize,
) -> DecodedBits {
    let bit_count = block_plan.len() / band_count.max(1);
    let pairs = cross_band_pairs(band_count);
    let bw = cross_band_weights(band_count);
    let mut bits = Vec::with_capacity(bit_count);
    let mut total_margin = 0.0_f32;

    for bit_index in 0..bit_count {
        let mut votes = [0i32; 2];
        let mut band_margins = 0.0f32;

        for band in 0..band_count {
            let plan_idx = bit_index * band_count + band;
            let Some(&(block_x, block_y)) = block_plan.get(plan_idx) else {
                break;
            };
            let abs_x = origin_x + block_x;
            let abs_y = origin_y + block_y;
            if abs_x + DCT_BLOCK > width {
                continue;
            }
            let (a_u, a_v, b_u, b_v) = pairs[band];
            let mut block = [0.0f32; 64];
            for y in 0..DCT_BLOCK {
                for x in 0..DCT_BLOCK {
                    let py = abs_y + y;
                    let px = abs_x + x;
                    if py < gray.len() as u32 / width && px < width {
                        block[(y * DCT_BLOCK + x) as usize] = gray[(py * width + px) as usize];
                    }
                }
            }
            crate::common::dct_2d(&mut block);
            let diff = block[crate::common::dct_index(a_u, a_v)]
                - block[crate::common::dct_index(b_u, b_v)];
            if diff > 0.0 {
                votes[1] += 1;
            } else {
                votes[0] += 1;
            }
            band_margins += diff.abs() * bw[band];
        }

        let bit = votes[1] > votes[0];
        bits.push(bit);
        total_margin += band_margins / band_count.max(1) as f32;
    }

    let confidence = if bit_count == 0 {
        0.0
    } else {
        (total_margin / bit_count as f32 / 20.0).clamp(0.0, 1.0)
    };

    DecodedBits { bits, confidence }
}
