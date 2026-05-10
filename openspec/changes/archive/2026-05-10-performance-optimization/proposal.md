# Proposal: Performance Optimization

## Problem

Current test suite takes ~360 seconds. DCT computation is pure Rust loops with no SIMD. The `fine_align` template matching is O(w×h×search²) which is very slow for large images.

## Solution

1. **Optimize DCT**: Use the `rustfft` crate for fast FFT-based DCT computation. Achieves O(N log N) instead of O(N²).

2. **Optimize fine_align**: Reduce the search space by using a coarse-to-fine approach. First search at 2px steps, then refine at 1px steps.

3. **Optimize rgb_to_y_channel**: Use SIMD-friendly loop structure.

## Expected Speedup

- DCT: 5-10x faster (FFT-based vs naive)
- fine_align: 4x faster (coarse-to-fine search)
- Overall test time: ~360s → ~100s

## Scope

- Modify: `src/midfreq.rs` (DCT)
- Modify: `src/align.rs` (fine_align)
- Modify: `src/lib.rs` (rgb_to_y_channel, block extraction)
- Add: `rustfft` dependency

## Out of Scope

- algorithmic changes
- new features
