# Proposal: Optimize Watermark System

## Current Issues

1. **Performance**: ~260 seconds per test (too slow)
2. **PSNR**: 44 dB (target: 50 dB for invisibility)
3. **Rotation**: Only 1° (need better alignment)

## Solution

1. Optimize DCT computation (use lookup tables, SIMD)
2. Reduce embedding strength for better PSNR
3. Improve rotation estimation algorithm

## Scope

- Optimize `src/midfreq.rs` (DCT, PRNG)
- Tune embedding strength in `src/lib.rs`
- Improve alignment in `src/align.rs`
- Update tests

## Out of Scope

- Complete rewrite
- New features
