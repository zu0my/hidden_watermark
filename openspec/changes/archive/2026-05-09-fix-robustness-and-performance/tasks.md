# Tasks: Fix Robustness and Performance

## Task 1: Increase embedding strength

**File**: `src/main.rs`

- [x] Change default strength from 0.3 to 0.5
- [x] Verify blur test passes
- [x] Verify PSNR still acceptable (>40 dB)

**Estimated effort**: 15 minutes

---

## Task 2: Improve rotation alignment

**File**: `src/align.rs`

- [x] Use phase correlation on gradient magnitude
- [x] Implement multi-scale search (64×64 + 256×256)
- [x] Increase search range to ±20°
- [x] Test with rotations 1°, 3°, 5°, 10°

**Estimated effort**: 2 hours

---

## Task 3: Optimize alignment performance

**File**: `src/align.rs`

- [x] Reduce downsample size to 128×128
- [x] Reduce search angles (coarse: 5° steps)
- [x] Add early termination for poor matches
- [x] Measure speedup

**Estimated effort**: 1 hour

---

## Task 4: Optimize DCT performance

**File**: `src/midfreq.rs`

- [x] Profile DCT computation
- [x] Optimize inner loops
- [x] Consider SIMD (future)
- [x] Measure speedup

**Estimated effort**: 1 hour

---

## Task 5: Run tests and validate

- [x] Run all tests
- [x] Verify blur test passes
- [x] Verify rotation improvement
- [x] Verify performance improvement
- [x] Document results

**Results:**
- 7/7 unit tests pass
- Performance: ~77s (3x faster than original ~250s)
- All 6 real images tested with CLI:
  - ✓ Clean: all 6 images pass (13x-57x confidence)
  - ✓ Blur: all 6 images pass (9x-39x confidence)
  - ✓ JPEG q60: all 6 images pass (13x-40x confidence)
  - ✓ Resize 75%: 4/6 pass (oMNKE 1.2x, webp2 2.6x)
- Fixed: blur detection (was box blur, now Gaussian)
- Fixed: non-16-multiple image sizes (crop_to_multiple)
- Fixed: image format detection (Reader + guessed_format)
- Pending: [ ] Run clippy and fix warnings (minor)

---

## Dependency order

```
Task 1 (strength) ──→ Task 5 (test)
Task 2 (rotation) ──→ Task 5
Task 3 (alignment perf) ──→ Task 5
Task 4 (DCT perf) ──→ Task 5
```
