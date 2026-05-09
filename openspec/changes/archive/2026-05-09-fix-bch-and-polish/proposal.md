# Fix BCH Codec & Polish

## Problem

The `robust-screen-cam` change left 4 incomplete tasks:

1. **BCH(127,64) encoding bug** — The generator polynomial coefficient ordering (ascending degree) conflicts with the bit-level polynomial division logic. The Berlekamp-Massey and Chien search implementations are correct, but encoding produces invalid codewords (syndromes non-zero).

2. **Preprocessing script untested** — `scripts/preprocess.py` has not been tested with real screen-captured images.

3. **Scripts not packaged** — The `scripts/` directory is not included in release builds.

4. **BCH unit tests missing** — No tests for encode/decode roundtrip, error correction, or uncorrectable rejection.

## Solution

1. Fix the BCH polynomial division by reversing the generator polynomial to descending degree order (index 0 = highest degree), matching the bit-level division expectation.

2. Test the preprocessing script with the existing asset images (simulated attacks via ImageMagick).

3. Add a build script or CI step to include `scripts/` in release artifacts.

4. Add comprehensive BCH unit tests after the fix.

## Scope

- Fix `src/bch.rs` (polynomial ordering)
- Test `scripts/preprocess.py` with real images
- Add packaging for `scripts/`
- Add BCH unit tests
