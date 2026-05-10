# Tasks: Integrate Non-Blind Watermark into Rust

## Task 1: Delete unused code

- [x] Delete `src/bch.rs`
- [x] Delete `src/frame.rs`
- [x] Delete `src/backend/jpeg_dct.rs`
- [x] Delete `src/backend/mod.rs`
- [x] Delete `src/backend/frequency_v2.rs`
- [x] Simplify `src/common.rs` (remove tile/瓦片 logic)
- [x] Remove unused imports and dependencies

**Estimated effort**: 30 minutes

---

## Task 2: Implement midfreq.rs

- [x] DCT block operations (32×32)
- [x] PRNG (ChaCha20-based, seeded from key)
- [x] Zigzag coefficient selection (mid-freq 8-24)
- [x] Perceptual weighting (texture complexity)
- [x] Embed signal into DCT coefficients
- [x] Extract signal from DCT coefficients

**Estimated effort**: 1.5 hours

---

## Task 3: Implement align.rs

- [x] Rotation estimation (template matching, three-stage search)
- [x] Translation estimation (phase correlation)
- [x] Fine alignment (template matching)
- [x] Histogram normalization
- [x] Handle resize (scale normalization)

**Estimated effort**: 2 hours

---

## Task 4: Implement lib.rs

- [x] embed_watermark() function
- [x] detect_watermark() function
- [x] Image I/O (load, save)
- [x] PSNR calculation
- [x] Threshold calculation (3.09σ for 0.1% FPR)

**Estimated effort**: 1.5 hours

---

## Task 5: Implement main.rs (CLI)

- [x] embed command: `hidden_watermark embed --input img.jpg --output wm.jpg --key secret`
- [x] detect command: `hidden_watermark detect --original img.jpg --suspect stolen.jpg --key secret`
- [x] Batch mode: `--original-dir` and `--suspect-dir` flags
- [x] Output format (text)

**Estimated effort**: 1 hour

---

## Task 6: Implement tests

- [x] Test invisibility (PSNR > 50dB)
- [x] Test detection on clean watermarked image
- [x] Test robustness: blur, JPEG, resize, brightness, contrast, noise
- [x] Test robustness: rotation (1°)
- [x] Test false positives
- [x] Test batch mode

**Estimated effort**: 1.5 hours

---

## Task 7: Update documentation

- [x] Update README.md with new architecture
- [x] Update CLI usage examples
- [x] Document known limitations (rotation > 1°)

**Estimated effort**: 30 minutes

---

## Dependency order

```
Task 1 (delete) ──→ Task 2 (midfreq) ──→ Task 4 (lib) ──→ Task 5 (CLI) ──→ Task 6 (tests)
                ──→ Task 3 (align)   ──→ Task 4
                                                              Task 7 (docs)
```
