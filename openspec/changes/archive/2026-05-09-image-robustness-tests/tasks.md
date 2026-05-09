# Tasks: Image Robustness Tests

## Task 1: Create test script

**File**: `scripts/test_robustness.sh`

- [x] Bash script with proper error handling
- [x] Auto-detect images in `assets/images/`
- [x] Support `--backend` flag (default: both)
- [x] Support `--transforms` flag (default: all)
- [x] Encode watermark with random ID per image
- [x] Apply each transform via ImageMagick
- [x] Decode and compare
- [x] Output summary table
- [x] Clean up temp files on exit

**Estimated effort**: 1 hour

---

## Task 2: Run tests and document results

- [x] Run script on all 6 images with both backends
- [x] Document which transforms pass/fail
- [x] Identify size thresholds for robust decoding
- [x] Note any unexpected failures

**Results summary:**
- Total: 156 tests, 111 pass, 45 fail (71% pass rate)
- frequency_v2: 78 tests, 62 pass (79%)
- jpeg_dct: 78 tests, 49 pass (63%)

**Key findings:**
- Images ≥2560×1440 (goku, Forza) are robust with frequency_v2
- Images ≤1170×1550 (oMNKE, webp) fail many transforms
- jpeg_dct fails on screen-to-cam for ALL images
- Aggressive transform fails universally
- Small images (1080px) fail even blur1 with frequency_v2

**Estimated effort**: 30 minutes

---

## Dependency order

```
Task 1 (create script) ── Task 2 (run and document)
```
