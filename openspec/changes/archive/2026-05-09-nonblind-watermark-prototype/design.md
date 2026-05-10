# Design: Non-Blind Watermark Prototype

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│                    Python Prototype                      │
├─────────────────────────────────────────────────────────┤
│                                                         │
│  embed.py    — Embed watermark into images              │
│  detect.py   — Detect watermark by comparing images     │
│  align.py    — Image alignment (rotation, crop, scale)  │
│  utils.py    — Shared utilities (DCT, PRNG, etc.)       │
│                                                         │
└─────────────────────────────────────────────────────────┘
```

## Embedding Algorithm

### DCT Block Size: 32×32

- Good frequency resolution for mid-band selection
- Survives JPEG compression (which uses 8×8 blocks)
- Sufficient spatial localization for cropping

### Embedding Process

```
Original Image (RGB)
    ↓
Convert to YCrCb
    ↓
Extract Y channel (luminance)
    ↓
Divide into 32×32 blocks
    ↓
For each block:
    DCT → Select mid-frequency coefficients (zigzag 8-24)
    c_new = c_old + α × p_i
    α = base_strength × local_texture_weight
    ↓
Inverse DCT → Inverse YCrCb → Output
```

### Parameters

| Parameter | Value | Rationale |
|-----------|-------|-----------|
| Block size | 32×32 | Balance frequency resolution vs cropping tolerance |
| Mid-freq range | zigzag 8-24 | Robust to compression, invisible to eye |
| Base strength | Adaptive | Target PSNR > 50dB |
| Texture weight | 0.5-2.0 | Higher in textured regions |
| PRNG | ChaCha20 | Deterministic from key |

## Detection Algorithm

### Pipeline

```
Original Image ─────┐
                    ├──→ Preprocess ──→ Align ──→ Detect ──→ Result
Suspect Image ──────┘
```

### Step 1: Preprocessing

- Resize suspect to match original dimensions
- Convert both to YCrCb, extract Y channel
- Normalize histogram (compensate brightness/contrast changes)

### Step 2: Alignment

**Coarse alignment:**
- Downsample both images (e.g., 256×256)
- Phase correlation to estimate translation
- Try multiple rotation angles (±15° in 1° steps)

**Fine alignment:**
- Template matching on full-resolution images
- Sub-pixel precision

### Step 3: Detection

```
For each 32×32 block:
    DCT(original) → mid-freq coefficients
    DCT(suspect)  → mid-freq coefficients
    
    difference = suspect_coeffs - original_coeffs
    
    block_score = Σ(difference_i × p_i) / N_coefficients

Overall score = mean(block_scores)
```

### Step 4: Threshold Decision

```
H₀: No watermark (score ~ N(0, σ²))
H₁: Watermark present (score > threshold)

For 0.1% false positive rate:
threshold = 3.09 × σ

σ estimated from block score distribution
```

## File Structure

```
prototype/
├── embed.py          # Embedding command-line tool
├── detect.py         # Detection command-line tool
├── align.py          # Image alignment functions
├── utils.py          # DCT, PRNG, perceptual weighting
├── requirements.txt  # Dependencies (numpy, opencv, scipy)
└── test.py           # Validation tests
```

## Dependencies

- **numpy**: Array operations, DCT
- **opencv-python**: Image I/O, alignment, resizing
- **scipy**: DCT implementation, signal processing

## Validation Plan

1. **Invisibility test**: Embed in test images, verify PSNR > 50dB
2. **Robustness test**: Apply transforms (blur, JPEG, crop, rotate), verify detection
3. **False positive test**: Test on non-watermarked images, verify < 0.1% false detection
4. **Batch test**: Process multiple images, verify consistency
