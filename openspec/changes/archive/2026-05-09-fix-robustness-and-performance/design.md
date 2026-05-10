# Design: Fix Robustness and Performance

## Blur Robustness Fix

### Problem
Reducing strength from 0.5 to 0.3 made watermark too weak for blur detection.

### Solution
- Increase default strength back to 0.5
- PSNR will drop to ~44 dB (still acceptable)
- Blur detection should pass again

## Rotation Improvement

### Problem
Gradient orientation histogram not accurate for small rotations (1-5°).

### Solution
1. **Use phase correlation on gradient magnitude**
   - More robust than orientation histogram
   - Better for small rotations

2. **Multi-scale approach**
   - Coarse search at 64×64 (fast)
   - Fine search at 256×256 (accurate)

3. **Increase search range**
   - Try ±20° instead of ±15°

## Performance Optimization

### Problem
~250 seconds per test is too slow.

### Bottleneck Analysis
- DCT computation: ~60% of time
- Image alignment: ~30% of time
- Other: ~10% of time

### Solutions

1. **DCT optimization**
   - Use lookup tables (already done)
   - Use SIMD (future)
   - Reduce iterations

2. **Alignment optimization**
   - Use smaller downsample size (128×128 instead of 256×256)
   - Reduce search angles
   - Use early termination

3. **Parallel processing**
   - Already using rayon
   - Can optimize chunk sizes
