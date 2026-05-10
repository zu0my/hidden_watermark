## Task 1: Optimize DCT computation

**File**: `src/midfreq.rs`

- [x] Optimize DCT with direct 1D computation (O(N²) for small blocks)
- [x] Keep separable row-column approach
- [x] Test DCT roundtrip accuracy
- [x] Measure speedup

**Note**: FFT-based DCT via rustfft was attempted but had correctness issues for small block sizes (index out of bounds). Reverted to direct computation which is simpler and sufficient for 16×16 blocks.

**Estimated effort**: 2 hours

---

## Task 2: Optimize fine_align with coarse-to-fine search

**File**: `src/align.rs`

- [x] Implement two-stage template matching (coarse 2px, fine 1px)
- [x] Reduce total search positions from 441 to 146
- [x] Test alignment accuracy
- [x] Benchmark vs current fine_align

**Estimated effort**: 1 hour

---

## Task 3: Optimize rgb_to_y_channel using integer math

**File**: `src/lib.rs`

- [x] Replace float-based Y channel conversion with integer arithmetic
- [x] Use fixed-point: `(299*r + 587*g + 114*b + 500) / 1000`
- [x] Test PSNR still acceptable (>40 dB)
- [x] Benchmark speedup

**Estimated effort**: 30 minutes

---

## Task 4: Remove unused code and clean up warnings

**File**: `src/align.rs`, `src/lib.rs`

- [x] Remove unused functions
- [x] Remove unused imports and variables
- [x] Fix clippy warnings
- [x] Run `cargo clippy -- -D warnings`

**Estimated effort**: 30 minutes

---

## Task 5: Run tests and validate

- [x] Run all 7 unit tests
- [x] Run image tests with all 6 images
- [x] Measure total test time
- [x] Document performance improvement

**Results**:
- 7/7 unit tests pass
- Test time: **138s** (was 365s)
- Speedup: **2.6x**
- All image transforms still pass

**Estimated effort**: 1 hour
