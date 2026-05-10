# Tasks: Non-Blind Watermark Prototype

## Task 1: Setup and utilities

**Files**: `prototype/utils.py`, `prototype/requirements.txt`

- [x] Create `prototype/` directory
- [x] Implement DCT block operations (32×32 blocks)
- [x] Implement PRNG (ChaCha20-based, seeded from key)
- [x] Implement perceptual weighting (texture complexity)
- [x] Implement zigzag coefficient selection (mid-freq 8-24)
- [x] Create requirements.txt (numpy, opencv-python, scipy)

**Estimated effort**: 1 hour

---

## Task 2: Embedding implementation

**File**: `prototype/embed.py`

- [x] Read image, convert to YCrCb
- [x] Divide Y channel into 32×32 blocks
- [x] Apply DCT to each block
- [x] Embed watermark signal in mid-frequency coefficients
- [x] Inverse DCT, convert back to RGB
- [x] Save watermarked image
- [x] Calculate and report PSNR
- [x] Command-line interface: `python embed.py --input img.jpg --output wm.jpg --key secret`

**Estimated effort**: 1.5 hours

---

## Task 3: Image alignment

**File**: `prototype/align.py`

- [x] Implement coarse alignment (phase correlation)
- [x] Implement rotation estimation (try multiple angles)
- [x] Implement fine alignment (template matching)
- [x] Handle resize (scale normalization)
- [x] Handle crop (find matching region)
- [x] Return aligned image + transform parameters

**Estimated effort**: 2 hours

---

## Task 4: Detection implementation

**File**: `prototype/detect.py`

- [x] Load original and suspect images
- [x] Preprocess (resize, color normalization)
- [x] Align suspect to original
- [x] Extract DCT coefficients from both images
- [x] Compute correlation score
- [x] Apply threshold (3.09σ for 0.1% FPR)
- [x] Output detection result + confidence score
- [x] Command-line interface: `python detect.py --original img.jpg --suspect stolen.jpg --key secret`

**Estimated effort**: 2 hours

---

## Task 5: Validation tests

**File**: `prototype/test.py`

- [x] Test invisibility (PSNR > 50dB)
- [x] Test detection on clean watermarked image (should detect)
- [x] Test robustness: blur, JPEG q40-q75, resize, brightness
- [x] Test robustness: rotation (±5°, ±10°)
- [x] Test robustness: crop (25%, 50%, 75%)
- [x] Test false positives on non-watermarked images
- [x] Document results

**Estimated effort**: 2 hours

---

## Task 6: Batch processing

**File**: `prototype/detect.py` (extend)

- [x] Support `--original-dir` and `--suspect-dir` flags
- [x] Match files by name or hash
- [x] Output table of results (image pairs, scores, decisions)
- [x] Summary statistics (detection rate, false positives)

**Estimated effort**: 1 hour

---

## Dependency order

```
Task 1 (utils) ──→ Task 2 (embed) ──→ Task 5 (test)
                ──→ Task 3 (align) ──→ Task 4 (detect) ──→ Task 5
                                                              ──→ Task 6 (batch)
```
