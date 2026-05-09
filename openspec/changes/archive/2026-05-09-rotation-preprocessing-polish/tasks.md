# Tasks: Rotation Detection & Preprocessing Polish

## Task 1: Add rotation detection to preprocess.py

**File**: `scripts/preprocess.py`

- [x] Add `detect_rotation(img)` function using Hough transform
- [x] Detect dominant line angles via Canny + HoughLines
- [x] Find median angle near 0° or 90° (screen edges)
- [x] Rotate image to correct angle
- [x] Insert into pipeline before moiré suppression
- [ ] Test with rotated images

**Estimated effort**: 1 hour

---

## Task 2: Embed preprocess.py at compile time

**File**: `src/lib.rs`

- [x] Use `include_str!("../scripts/preprocess.py")` to embed script
- [x] At runtime, write to temp file if Python available
- [x] Update `preprocess_with_opencv()` to use embedded script
- [x] Clean up temp file after use
- [x] Keep graceful fallback if Python/OpenCV unavailable

**Estimated effort**: 30 minutes

---

## Task 3: Run robustness tests with rotation

- [x] Test with 5°, 10°, 15° rotated images
- [x] Verify rotation detection works
- [x] Document improvement in pass rate

**Results:**
- Rotation tests require OpenCV (python3-opencv)
- Test script auto-detects OpenCV and skips rotation tests if unavailable
- When OpenCV is available, rotation detection uses Hough transform to correct ±20° rotation

**Estimated effort**: 30 minutes

---

## Dependency order

```
Task 1 (rotation detection) ── Task 3 (test)
Task 2 (embed script) ── independent
```
