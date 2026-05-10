use sha2::{Digest, Sha256};

pub const BLOCK_SIZE: usize = 16;
pub const MID_FREQ_START: usize = 8;
pub const MID_FREQ_END: usize = 24;

/// Get mid-frequency coefficient positions (zigzag indices 8-24)
pub fn get_mid_freq_positions() -> Vec<(usize, usize)> {
    let zigzag = get_zigzag_indices(BLOCK_SIZE);
    zigzag[MID_FREQ_START..MID_FREQ_END].to_vec()
}

/// Generate PRN sequence of +1.0/-1.0 values from key
pub fn generate_prng_sequence(key: &str, length: usize) -> Vec<f64> {
    let seed = Sha256::digest(key.as_bytes());
    let mut result = vec![0.0f64; length];
    let mut block_index = 0u32;
    let mut generated = 0;

    while generated < length {
        let mut hasher = Sha256::new();
        hasher.update(seed);
        hasher.update(block_index.to_le_bytes());
        let block_hash = hasher.finalize();

        for chunk in block_hash.chunks(2) {
            if generated >= length {
                break;
            }
            let val = if chunk.len() >= 2 {
                chunk[0] ^ chunk[1]
            } else {
                chunk[0]
            };
            result[generated] = if val >= 128 { 1.0 } else { -1.0 };
            generated += 1;
        }

        block_index += 1;
    }

    result
}

/// Get zigzag scan order for a block
pub fn get_zigzag_indices(block_size: usize) -> Vec<(usize, usize)> {
    let mut indices = Vec::new();
    for sum_val in 0..(2 * block_size - 1) {
        if sum_val % 2 == 0 {
            // Go up-right
            let start = sum_val.min(block_size - 1);
            let end = (sum_val as isize - block_size as isize + 1).max(0) as usize;
            for row in (end..=start).rev() {
                let col = sum_val - row;
                if col < block_size {
                    indices.push((row, col));
                }
            }
        } else {
            // Go down-left
            let start = (sum_val as isize - block_size as isize + 1).max(0) as usize;
            let end = sum_val.min(block_size - 1);
            for row in start..=end {
                let col = sum_val - row;
                if col < block_size {
                    indices.push((row, col));
                }
            }
        }
    }
    indices
}

/// Compute cross-shaped PRN combined value (own + 4 neighbors)
pub fn combined_prn(
    prn: &[f64],
    blocks_x: usize,
    blocks_y: usize,
    band_len: usize,
    by: usize,
    bx: usize,
    i: usize,
) -> f64 {
    let base = by * blocks_x * band_len + bx * band_len + i;
    let own = prn[base];
    let left = if bx > 0 {
        prn[by * blocks_x * band_len + (bx - 1) * band_len + i]
    } else {
        0.0
    };
    let top = if by > 0 {
        prn[(by - 1) * blocks_x * band_len + bx * band_len + i]
    } else {
        0.0
    };
    let right = if bx + 1 < blocks_x {
        prn[by * blocks_x * band_len + (bx + 1) * band_len + i]
    } else {
        0.0
    };
    let bottom = if by + 1 < blocks_y {
        prn[(by + 1) * blocks_x * band_len + bx * band_len + i]
    } else {
        0.0
    };
    own + left + top + right + bottom
}

/// Compute perceptual weight based on block texture complexity
pub fn compute_texture_weight(block: &[f64]) -> f64 {
    let mean = block.iter().sum::<f64>() / block.len() as f64;
    let variance = block.iter().map(|&x| (x - mean).powi(2)).sum::<f64>() / block.len() as f64;
    // Low variance = flat region (low weight), high variance = textured (high weight)
    // Clamp to [0.5, 2.0]
    (0.5 + (variance / 1000.0).min(1.5)).clamp(0.5, 2.0)
}

/// Apply 1D Type-II DCT using direct computation (for small block sizes)
#[allow(clippy::manual_memcpy, clippy::needless_range_loop)]
fn dct_1d(data: &[f64], size: usize) -> Vec<f64> {
    // O(N²) direct DCT: for small block sizes (16), this is competitive with FFT
    let n = size;
    let mut result = vec![0.0f64; n];
    for k in 0..n {
        let mut sum = 0.0;
        for x in 0..n {
            sum += data[x]
                * (std::f64::consts::PI * k as f64 * (2 * x + 1) as f64 / (2 * n) as f64).cos();
        }
        let scale = if k == 0 {
            (1.0 / n as f64).sqrt()
        } else {
            (2.0 / n as f64).sqrt()
        };
        result[k] = sum * scale;
    }
    result
}

/// Apply 1D Type-III IDCT using direct computation
#[allow(clippy::needless_range_loop)]
fn idct_1d(data: &[f64], size: usize) -> Vec<f64> {
    let n = size;
    let mut result = vec![0.0f64; n];
    for x in 0..n {
        let mut sum = 0.0;
        for k in 0..n {
            let scale = if k == 0 {
                (1.0 / n as f64).sqrt()
            } else {
                (2.0 / n as f64).sqrt()
            };
            sum += scale
                * data[k]
                * (std::f64::consts::PI * k as f64 * (2 * x + 1) as f64 / (2 * n) as f64).cos();
        }
        result[x] = sum;
    }
    result
}

/// Apply 2D DCT using separable FFT-based DCT
pub fn dct_2d(block: &[f64], size: usize) -> Vec<f64> {
    let mut temp = vec![0.0f64; size * size];

    // Row-wise DCT
    for row in 0..size {
        let row_offset = row * size;
        let dct_row = dct_1d(&block[row_offset..row_offset + size], size);
        temp[row_offset..(size + row_offset)].copy_from_slice(&dct_row[..size]);
    }

    // Column-wise DCT
    let mut result = vec![0.0f64; size * size];
    let mut col_data = vec![0.0f64; size];
    for col in 0..size {
        for row in 0..size {
            col_data[row] = temp[row * size + col];
        }
        let dct_col = dct_1d(&col_data, size);
        for row in 0..size {
            result[row * size + col] = dct_col[row];
        }
    }

    result
}

/// Apply 2D IDCT using separable FFT-based IDCT
pub fn idct_2d(block: &[f64], size: usize) -> Vec<f64> {
    let mut temp = vec![0.0f64; size * size];

    // Column-wise IDCT
    let mut col_data = vec![0.0f64; size];
    for col in 0..size {
        for row in 0..size {
            col_data[row] = block[row * size + col];
        }
        let idct_col = idct_1d(&col_data, size);
        for row in 0..size {
            temp[row * size + col] = idct_col[row];
        }
    }

    // Row-wise IDCT
    let mut result = vec![0.0f64; size * size];
    for row in 0..size {
        let row_offset = row * size;
        let idct_row = idct_1d(&temp[row_offset..row_offset + size], size);
        result[row_offset..(size + row_offset)].copy_from_slice(&idct_row[..size]);
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prng_deterministic() {
        let seq1 = generate_prng_sequence("test_key", 100);
        let seq2 = generate_prng_sequence("test_key", 100);
        assert_eq!(seq1, seq2);
    }

    #[test]
    fn test_prng_values() {
        let seq = generate_prng_sequence("test123", 20);
        println!("PRN for key test123: {:?}", &seq[..16]);
        // Manually check first few values
        let seed = Sha256::digest(b"test_key");
        println!("Seed: {:x?}", seed);
        let mut hasher = Sha256::new();
        hasher.update(&seed);
        hasher.update(&0u32.to_le_bytes());
        let block_hash = hasher.finalize();
        println!("Block 0 hash: {:x?}", block_hash);
        for (i, chunk) in block_hash.chunks(2).enumerate() {
            if i >= 5 {
                break;
            }
            let val = chunk[0] ^ chunk[1];
            println!(
                "  xor[{}]={} -> {}",
                i,
                val,
                if val >= 128 { 1.0 } else { -1.0 }
            );
        }
        for &v in &seq {
            assert!(v == 1.0 || v == -1.0);
        }
    }

    #[test]
    fn test_zigzag_indices() {
        let indices = get_zigzag_indices(8);
        assert_eq!(indices.len(), 64);
        // First few should be (0,0), (0,1), (1,0), (2,0), (1,1), (0,2), ...
        assert_eq!(indices[0], (0, 0));
        assert_eq!(indices[1], (0, 1));
        assert_eq!(indices[2], (1, 0));
    }

    #[test]
    fn test_mid_freq_positions() {
        let positions = get_mid_freq_positions();
        assert_eq!(positions.len(), MID_FREQ_END - MID_FREQ_START);
    }

    #[test]
    fn test_dct_values() {
        let block = vec![
            1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 11.0, 12.0, 13.0, 14.0, 15.0, 16.0,
        ];
        let size = 4;
        let dct = dct_2d(&block, size);
        println!("FFT DCT: {:?}", &dct[..8]);
        // Expected from scipy: [34.0, -4.46, 0.0, -0.32, -17.84, 0.0, 0.0, 0.0]
    }

    #[test]
    fn test_dct_idct_roundtrip() {
        let block = vec![
            1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 11.0, 12.0, 13.0, 14.0, 15.0, 16.0,
        ];
        let size = 4;
        let dct = dct_2d(&block, size);
        let idct = idct_2d(&dct, size);
        for (a, b) in block.iter().zip(idct.iter()) {
            assert!((a - b).abs() < 1e-8);
        }
    }
}
