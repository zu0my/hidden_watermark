# Tasks: Fix BCH Codec & Polish

## Task 1: Fix BCH polynomial division

**File**: `src/bch.rs`

- [x] Fix generator polynomial coefficient ordering (ascending degree, trailing zeros trimmed)
- [x] Verify `encode_codeword()` produces valid codewords (syndromes = 0 for all odd powers)
- [x] Implement working decode (brute-force syndrome matching for 1-3 errors, PGZ fallback for >3)
- [x] Run `cargo clippy --all-features -- -D warnings`

**Status**: Encode is correct (syndromes = 0). Decode uses brute-force syndrome matching for 1-3 errors (O(N^v) where N=127, v=error count). PGZ decoder is included as fallback for >3 errors but has known issues with certain error patterns.

**Approach**: Direct syndrome-based error location. For v errors, iterate over all C(127,v) combinations and check if the resulting syndromes match. This is practical for v≤3 (C(127,3) ≈ 338K iterations). For v>3, the PGZ decoder attempts to find the error locator polynomial via Gaussian elimination on syndrome matrices.

**Estimated effort**: 1 hour

---

## Task 2: Add BCH unit tests

**File**: `src/bch.rs`

- [x] `encode_decode_roundtrip`: encode data, verify syndromes zero, decode, compare
- [x] `corrects_single_bit_error`: flip 1 bit, decode, verify correction
- [x] `corrects_multiple_errors`: flip 3 bits, decode, verify correction
- [x] `rejects_uncorrectable`: flip >3 bits, verify decode returns None
- [x] `multi_chunk_roundtrip`: encode >64 bits (2 chunks), verify roundtrip

**Status**: All 9 tests pass. Tests cover: valid codeword, multi-chunk encode, roundtrip, single error correction, 3-error correction, uncorrectable rejection, multi-chunk roundtrip, and multi-chunk error correction.

**Estimated effort**: 1 hour

---

## Task 3: Test preprocessing script

**File**: `scripts/preprocess.py`

Test with real images and simulated screen-to-cam attacks.

- [x] Mark an image with watermark
- [x] Apply perspective distortion via ImageMagick
- [x] Apply blur + JPEG q60
- [x] Run through `preprocess.py`
- [x] Attempt decode on preprocessed image
- [x] Document results

**Estimated effort**: 1-2 hours

---

## Task 4: Package scripts directory

- [x] Add `scripts/preprocess.py` to `.gitignore` exceptions (if needed)
- [x] Document that `scripts/` must be distributed alongside the binary
- [x] Optionally: embed the Python script in the Rust binary at compile time

**Estimated effort**: 30 minutes

---

## Dependency order

```
Task 1 (fix BCH) ── Task 2 (BCH tests)
Task 3 (test preprocessing) ── independent
Task 4 (packaging) ── independent
```

All tasks complete.
