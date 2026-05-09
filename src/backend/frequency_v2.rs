use image::{ImageBuffer, RgbImage};
use rand::SeedableRng;
use rand::seq::SliceRandom;
use rand_chacha::ChaCha20Rng;
use sha2::{Digest, Sha256};

use crate::backend::WatermarkBackend;
use crate::common::{
    DCT_BLOCK, DecodedBits, FrequencyBlock, FrequencySubband, RobustWatermarkOptions, TileAttempt,
    WatermarkPreset, accumulate_luma_tile, block_variance, cross_band_pairs, cross_band_weights,
    dct_2d, dct_index, haar_forward, haar_inverse, idct_2d, normalize_tile, perceptual_mask,
    read_block8, read_luma_tile, rgb_to_ycbcr, write_block8, ycbcr_to_rgb,
};
use crate::frame::{
    self, HEADER_BYTES, HeaderStatus, ParseFrameResult, bits_to_bytes, parse_frame,
    parse_frame_header,
};

use crate::common::DecodeStatus;

const FREQUENCY_ALGORITHM: &str = "frequency_v2";

pub(crate) struct FrequencyV2Backend;

impl WatermarkBackend for FrequencyV2Backend {
    fn name(&self) -> &'static str {
        FREQUENCY_ALGORITHM
    }

    fn capacity_bits(&self, tile_size: u32, band_count: usize) -> usize {
        frequency_tile_capacity_bits(tile_size, band_count)
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

        let mut y_accum = vec![0.0_f32; y_channel.len()];
        let mut weights = vec![0_u16; y_channel.len()];
        let block_plan = frequency_block_plan(key, bits.len(), options.tile_size, band_count);
        let margin = frequency_margin(options);
        let pairs = cross_band_pairs(band_count);
        let bw = cross_band_weights(band_count);

        for &(origin_x, origin_y) in origins {
            let mut tile = read_luma_tile(&y_channel, width, origin_x, origin_y, options.tile_size);
            embed_frequency_tile(&mut tile, &block_plan, bits, margin, pairs, bw, band_count);
            accumulate_luma_tile(
                &mut y_accum,
                &mut weights,
                width,
                origin_x,
                origin_y,
                options.tile_size,
                &tile,
            );
        }

        let mut output = ImageBuffer::new(width, height);
        for y in 0..height {
            for x in 0..width {
                let idx = (y * width + x) as usize;
                let new_y = if weights[idx] == 0 {
                    y_channel[idx]
                } else {
                    y_accum[idx] / f32::from(weights[idx])
                };
                output.put_pixel(x, y, ycbcr_to_rgb(new_y, cb_channel[idx], cr_channel[idx]));
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
            let header_bits = decode_frequency_bits(
                gray,
                width,
                origin_x,
                origin_y,
                base_tile_size,
                key,
                header_bits_len,
                try_bc,
            );
            let header_bytes = bits_to_bytes(&header_bits.bits, HEADER_BYTES);
            let HeaderStatus::Valid(id_len) = parse_frame_header(&header_bytes) else {
                continue;
            };

            let raw_bytes = HEADER_BYTES + id_len + frame::CRC_BYTES;
            let full_bit_len = raw_bytes * 8;
            let decoded_bits = decode_frequency_bits(
                gray,
                width,
                origin_x,
                origin_y,
                base_tile_size,
                key,
                full_bit_len,
                try_bc,
            );
            let frame_data = bits_to_bytes(&decoded_bits.bits, raw_bytes);
            match parse_frame(&frame_data) {
                ParseFrameResult::Decoded(id) => {
                    return TileAttempt {
                        id: Some(id),
                        status: DecodeStatus::Decoded,
                        confidence: decoded_bits.confidence,
                        corrected_bytes: 0,
                        bits: decoded_bits.bits,
                        raw_bytes,
                    };
                }
                ParseFrameResult::CrcMismatch => {
                    return TileAttempt {
                        status: DecodeStatus::CrcMismatch,
                        confidence: decoded_bits.confidence,
                        corrected_bytes: 0,
                        bits: decoded_bits.bits,
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

fn frequency_tile_capacity_bits(tile_size: u32, band_count: usize) -> usize {
    if tile_size < 64 || band_count == 0 {
        return 0;
    }
    let sub_size = tile_size / 2;
    let blocks_per_subband = (sub_size / DCT_BLOCK) * (sub_size / DCT_BLOCK);
    blocks_per_subband as usize / band_count
}

#[allow(clippy::too_many_arguments)]
fn decode_frequency_bits(
    gray: &[f32],
    width: u32,
    origin_x: u32,
    origin_y: u32,
    tile_size: u32,
    key: &str,
    bit_len: usize,
    band_count: usize,
) -> DecodedBits {
    if bit_len > frequency_tile_capacity_bits(tile_size, band_count) {
        return DecodedBits {
            bits: Vec::new(),
            confidence: 0.0,
        };
    }
    let tile = read_luma_tile(gray, width, origin_x, origin_y, tile_size);
    let wavelet = haar_forward(&tile, tile_size);
    let sub_size = tile_size / 2;
    let block_plan = frequency_block_plan(key, bit_len, tile_size, band_count);
    let pairs = cross_band_pairs(band_count);
    let bw = cross_band_weights(band_count);
    let mut bits = Vec::with_capacity(bit_len);
    let mut total_margin = 0.0_f32;

    for bit_index in 0..bit_len {
        let mut votes = [0i32; 2];
        let mut band_margins = 0.0f32;

        for band in 0..band_count {
            let plan_idx = bit_index * band_count + band;
            let Some(block) = block_plan.get(plan_idx) else {
                break;
            };
            let subband = match block.subband {
                FrequencySubband::Ll => &wavelet.ll,
            };
            let (a_u, a_v, b_u, b_v) = pairs[band];
            let mut blk = read_block8(subband, sub_size, block.x, block.y);
            dct_2d(&mut blk);
            let diff = blk[dct_index(a_u, a_v)] - blk[dct_index(b_u, b_v)];
            if diff > 0.0 {
                votes[1] += 1;
            } else {
                votes[0] += 1;
            }
            band_margins += diff.abs() * bw[band];
        }

        let bit = votes[1] > votes[0];
        bits.push(bit);
        total_margin += band_margins / band_count as f32;
    }

    let confidence = if bit_len == 0 {
        0.0
    } else {
        (total_margin / bit_len as f32 / 24.0).clamp(0.0, 1.0)
    };

    DecodedBits { bits, confidence }
}

fn embed_frequency_tile(
    tile: &mut [f32],
    block_plan: &[FrequencyBlock],
    bits: &[bool],
    margin: f32,
    pairs: &[(usize, usize, usize, usize)],
    bw: &[f32],
    band_count: usize,
) {
    let tile_size = (tile.len() as f32).sqrt() as u32;
    let sub_size = tile_size / 2;
    let mut wavelet = haar_forward(tile, tile_size);

    for (bit_index, bit) in bits.iter().copied().enumerate() {
        for band in 0..band_count {
            let plan_idx = bit_index * band_count + band;
            let Some(block) = block_plan.get(plan_idx) else {
                break;
            };
            let subband = match block.subband {
                FrequencySubband::Ll => &mut wavelet.ll,
            };
            let variance = block_variance(subband, sub_size, block.x, block.y);
            let mask = perceptual_mask(variance);
            if mask <= 0.0 {
                continue;
            }
            let band_margin = margin * bw[band];
            embed_dct_bit_band(
                subband,
                sub_size,
                block.x,
                block.y,
                bit,
                band_margin * mask,
                pairs[band],
            );
        }
    }

    let reconstructed = haar_inverse(&wavelet, tile_size);
    tile.copy_from_slice(&reconstructed);
}

fn frequency_block_plan(
    key: &str,
    bit_len: usize,
    tile_size: u32,
    band_count: usize,
) -> Vec<FrequencyBlock> {
    let sub_size = tile_size / 2;
    let grid = sub_size / DCT_BLOCK;
    let mut blocks = Vec::with_capacity((grid * grid) as usize);
    for subband in [FrequencySubband::Ll] {
        for y in 0..grid {
            for x in 0..grid {
                blocks.push(FrequencyBlock {
                    subband,
                    x: x * DCT_BLOCK,
                    y: y * DCT_BLOCK,
                });
            }
        }
    }
    let mut rng = rng_for_frequency_plan(key);
    blocks.shuffle(&mut rng);
    let needed = bit_len * band_count;
    blocks.truncate(needed.min(blocks.len()));
    blocks
}

fn rng_for_frequency_plan(key: &str) -> ChaCha20Rng {
    let mut hasher = Sha256::new();
    hasher.update(b"hidden-watermark-frequency-v2-plan");
    hasher.update(key.as_bytes());
    let digest = hasher.finalize();
    let mut seed = [0_u8; 32];
    seed.copy_from_slice(&digest);
    ChaCha20Rng::from_seed(seed)
}

fn frequency_margin(options: &RobustWatermarkOptions) -> f32 {
    let base = match options.preset {
        WatermarkPreset::Invisible => 240.0,
        WatermarkPreset::Balanced => 280.0,
        WatermarkPreset::Robust => 360.0,
    };
    (base * options.strength).clamp(2.0, 64.0)
}

fn embed_dct_bit_band(
    data: &mut [f32],
    width: u32,
    block_x: u32,
    block_y: u32,
    bit: bool,
    margin: f32,
    (a_u, a_v, b_u, b_v): (usize, usize, usize, usize),
) {
    let mut block = read_block8(data, width, block_x, block_y);
    dct_2d(&mut block);
    let a_idx = dct_index(a_u, a_v);
    let b_idx = dct_index(b_u, b_v);
    let diff = block[a_idx] - block[b_idx];
    let target = if bit { margin } else { -margin };
    let needs_adjustment = (bit && diff < target) || (!bit && diff > target);
    if needs_adjustment {
        let adjustment = (target - diff) * 0.5;
        block[a_idx] += adjustment;
        block[b_idx] -= adjustment;
    }
    idct_2d(&mut block);
    write_block8(data, width, block_x, block_y, &block);
}
