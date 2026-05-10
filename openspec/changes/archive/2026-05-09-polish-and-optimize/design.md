# Design: Polish and Optimize

## Performance Optimization

### Current DCT Implementation
- Precomputed cosine tables
- Separable 1D DCT (row + column)
- O(N³) loops: for each row, for each coefficient, for each pixel

### Optimization Strategy
1. **Butterfly DCT**: Use the Lee algorithm (recursive decomposition) for O(N² log N) instead of O(N³)
2. **Cache-friendly access**: Process rows/columns with contiguous memory access
3. **Reduce function calls**: Inline helper functions

### Expected Speedup
- DCT: 2-5x faster
- Overall: 20-30% faster

## Python Prototype Cleanup

### Current State
- `prototype/` with 6 files (~600 lines)
- All functionality now in Rust
- No longer needed for development

### Cleanup Plan
1. Move `prototype/` to `archive/` as reference
2. Keep `scripts/test_robustness.sh` (uses image magick for real testing)

## README Update

### Changes Needed
1. CLI commands: `embed` / `detect` / `detect-batch` (not `encode`/`decode`)
2. Architecture: new non-blind detection approach
3. Test results: 7/7 tests pass, 6 real images
4. Dependencies: current Cargo.toml
5. Remove references to old blind-detection system
