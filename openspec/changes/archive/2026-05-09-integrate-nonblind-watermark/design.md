# Design: Integrate Non-Blind Watermark into Rust

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│                    CLI (main.rs)                         │
│  embed / detect                                         │
├─────────────────────────────────────────────────────────┤
│                    Core (lib.rs)                         │
│  embed_watermark() / detect_watermark()                 │
├─────────────────────────────────────────────────────────┤
│                    Backend                               │
│  midfreq.rs: DCT, PRNG, perceptual weighting            │
├─────────────────────────────────────────────────────────┤
│                    Alignment (align.rs)                  │
│  rotation estimation, template matching, phase corr     │
└─────────────────────────────────────────────────────────┘
```

## Files to Delete

- `src/bch.rs` — BCH error correction (not needed)
- `src/frame.rs` — Frame format (not needed)
- `src/backend/jpeg_dct.rs` — jpeg_dct backend (not useful)
- `src/backend/mod.rs` — Backend trait (simplify)
- `src/common.rs` — Most of it (simplify)

## Files to Create/Rewrite

- `src/midfreq.rs` — Mid-frequency DCT operations, PRNG, embedding
- `src/align.rs` — Image alignment (rotation, translation, template matching)
- `src/lib.rs` — embed_watermark(), detect_watermark()
- `src/main.rs` — CLI: embed/detect commands
- `tests/robust.rs` — Robustness tests

## Algorithm (from Python prototype)

### Embedding
1. Convert to YCrCb, extract Y channel
2. Split into 32×32 blocks
3. DCT each block
4. Add PRN signal to mid-frequency coefficients (zigzag 8-24)
5. Inverse DCT, convert back to RGB

### Detection
1. Align suspect to original (rotation + translation)
2. Normalize histogram
3. Convert to YCrCb, extract Y channel
4. Split into 32×32 blocks
5. DCT each block
6. Compute correlation with PRN sequence
7. Threshold decision (3.09σ for 0.1% FPR)
