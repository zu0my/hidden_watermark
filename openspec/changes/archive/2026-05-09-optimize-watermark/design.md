# Design: Optimize Watermark System

## Performance Optimization

### Current Bottleneck
DCT computation is O(N²) for each block. With 32×32 blocks and thousands of blocks per image, this is slow.

### Optimization Strategy

1. **Precompute cosine tables**
   - cos(π * u * (2x + 1) / 2N) can be precomputed
   - Store in lookup tables

2. **Use separable DCT**
   - 2D DCT = 1D DCT on rows + 1D DCT on columns
   - More cache-friendly

3. **Parallel processing**
   - Use rayon for parallel block processing
   - Each block is independent

4. **Reduce block size**
   - 16×16 blocks instead of 32×32
   - 4x more blocks but each is 4x faster
   - Net: similar total time but better parallelism

### Expected Speedup
- Precomputed tables: 2-3x
- Parallel processing: 4-8x (depending on cores)
- Total: 8-24x speedup

## PSNR Improvement

### Current Issue
PSNR = 44 dB with strength = 0.5

### Solution
- Reduce default strength to 0.3
- Make strength configurable
- Test different values to find optimal

### Expected Result
- PSNR > 50 dB with strength = 0.3
- Detection still works (signal is weaker but still detectable)

## Rotation Improvement

### Current Issue
Alignment fails for rotations > 1°

### Solution
1. **Use gradient orientation histogram**
   - More robust than template matching
   - Invariant to translation

2. **Multi-scale approach**
   - Coarse search at low resolution
   - Fine search at high resolution

3. **Phase correlation on gradient magnitude**
   - Gradient magnitude is more rotation-invariant

### Expected Result
- Support rotations up to 5-10°
