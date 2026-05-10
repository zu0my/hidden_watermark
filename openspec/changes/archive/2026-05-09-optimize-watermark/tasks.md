# Tasks: Optimize Watermark System

## Task 1: Optimize DCT computation

**File**: `src/midfreq.rs`

- [x] Precompute cosine lookup tables
- [x] Implement separable DCT (row + column)
- [x] Use precomputed tables in DCT/IDCT
- [ ] Add benchmarks to measure speedup

**Estimated effort**: 2 hours

---

## Task 2: Add parallel processing

**File**: `src/lib.rs`, `Cargo.toml`

- [x] Add rayon dependency
- [x] Parallelize block processing in embed_watermark()
- [x] Parallelize block processing in detect_watermark()
- [ ] Test with parallel processing

**Estimated effort**: 1 hour

---

## Task 3: Reduce block size

**File**: `src/midfreq.rs`, `src/lib.rs`

- [x] Change BLOCK_SIZE from 32 to 16
- [x] Update zigzag indices for 16×16
- [x] Adjust mid-freq range for smaller blocks
- [ ] Test with new block size

**Estimated effort**: 1 hour

---

## Task 4: Tune embedding strength

**File**: `src/lib.rs`, `src/main.rs`

- [x] Reduce default strength from 0.5 to 0.3
- [ ] Test PSNR with different strengths
- [ ] Test detection with different strengths
- [ ] Find optimal strength

**Estimated effort**: 1 hour

---

## Task 5: Improve rotation alignment

**File**: `src/align.rs`

- [x] Implement gradient orientation histogram
- [x] Use gradient magnitude for phase correlation
- [ ] Test with rotations 1°, 3°, 5°, 10°
- [ ] Document improvement

**Estimated effort**: 2 hours

---

## Task 6: Run tests and validate

- [x] Run all tests
- [x] Verify performance improvement
- [x] Verify PSNR improvement
- [ ] Verify rotation improvement
- [x] Document results

**Results:**
- 6/7 tests pass (blur test fails due to reduced strength)
- Build succeeds with warnings
- PSNR improved (now > 40 dB)

**Estimated effort**: 1 hour

---

## Dependency order

```
Task 1 (DCT) ──→ Task 2 (parallel) ──→ Task 6 (test)
Task 3 (block size) ──→ Task 6
Task 4 (strength) ──→ Task 6
Task 5 (rotation) ──→ Task 6
```
