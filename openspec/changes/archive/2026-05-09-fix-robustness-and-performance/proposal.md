# Proposal: Fix Robustness and Performance

## Current Issues

1. **Blur detection fails** — Embedding strength reduced from 0.5 to 0.3, making watermark too weak for blur
2. **Rotation limited to 1°** — Gradient orientation histogram not accurate enough
3. **Performance slow** — ~250 seconds per test (target: <30 seconds)

## Solution

1. Increase embedding strength back to 0.5 (blur robustness more important than PSNR)
2. Improve rotation alignment with multi-scale approach
3. Optimize DCT with SIMD or lookup table improvements

## Scope

- Adjust embedding strength in `src/main.rs`
- Improve alignment in `src/align.rs`
- Optimize DCT in `src/midfreq.rs`
- Update tests

## Out of Scope

- New features
- Python prototype changes
