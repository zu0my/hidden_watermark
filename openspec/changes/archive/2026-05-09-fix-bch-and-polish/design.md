# Design: Fix BCH Codec & Polish

## BCH Fix

### Root Cause

The `generator_polynomial()` function uses `poly_mul()` which stores polynomials in ascending degree order (index 0 = constant term). But the polynomial division in `encode_codeword()` processes bits from highest degree down, expecting index 0 = highest degree.

### Fix

Reverse the generator polynomial in `BchEncoder::new()` so that `gen_poly[0]` = leading coefficient (degree 63) and `gen_poly[63]` = constant term. This matches the polynomial division's expectation.

```rust
pub(crate) fn new() -> Self {
    let gf = Gf2m::new();
    let mut gen_poly = generator_polynomial(&gf);
    gen_poly.reverse(); // index 0 = highest degree
    BchEncoder { gf, gen_poly }
}
```

Also need to update `encode_codeword()` to use the reversed polynomial correctly in the division loop.

### Alternative: Rewrite encode with coefficient array

Instead of bit manipulation, use a coefficient array (index 0 = degree 0) throughout, and rewrite the polynomial division to work with this ordering. This is cleaner but requires more changes.

**Decision:** Reverse the generator polynomial (minimal change).

## Preprocessing Testing

Test `scripts/preprocess.py` with ImageMagick-simulated attacks:

1. Take a marked image
2. Apply perspective distortion: `magick img -distort Perspective "..." `
3. Apply blur + JPEG compression
4. Run through `preprocess.py`
5. Attempt decode

## Packaging

Add `scripts/preprocess.py` to the project's build artifacts. For Rust, this means including it in the release binary directory or documenting the installation step.
