# Design: Performance Optimization

## DCT Optimization

### Current Implementation
```
dct_2d():  O(N³) = N × N × N = 4096 operations per 16×16 block
  - Precomputed cosine table
  - Three nested loops (row/coeff/pixel)
```

### New Implementation (FFT-based)
```
dct_2d(): O(N² log N) 
  - Use Type-II DCT = FFT of length 2N with rearrangement
  - rustfft crate for FFT computation
  - 16×16 block → 32-point FFT per row → rearrange → 32-point FFT per column
```

### Why FFT is faster
- For 16×16: naive = 16×16×16×2 = 8192 multiply-adds per 2D DCT
- FFT = 16×32×log₂(32)×2 = 2560 multiply-adds per 2D DCT
- Speedup: ~3x for DCT alone
- For 2560×1440 image: 14400 blocks × 2560 ops = 37M ops vs 118M ops

## fine_align Optimization

### Current Implementation
```
Template matching at search_range=10:
  (21×21) × (template_w × template_h) = 441 × ~2.5M = ~1.1B pixel comparisons
```

### New Implementation (coarse-to-fine)
```
Stage 1: search_range=10, step=2  → 121 positions
Stage 2: search_range=2, step=1   → 25 positions around best from stage 1
Total: 146 positions (vs 441)
Speedup: ~3x
```

## rgb_to_y_channel Optimization

### Current
```rust
y = 0.299 * r + 0.587 * g + 0.114 * b
```
Each pixel computed individually with float multiplications.

### New
Use integer arithmetic for the Y channel conversion:
```rust
y = (299 * r + 587 * g + 114 * b + 500) / 1000
```
Avoids float conversions for ~4M pixels.

## Expected Overall Speedup

| Component | Before | After | Speedup |
|-----------|--------|-------|---------|
| DCT | ~200s | ~50s | 4x |
| fine_align | ~80s | ~25s | 3x |
| rgb_to_y | ~10s | ~3s | 3x |
| Other overhead | ~70s | ~50s | 1.4x |
| **Total** | **~360s** | **~128s** | **~2.8x** |
