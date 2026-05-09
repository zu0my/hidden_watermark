# Design: Robust Screen-to-Cam Watermarking

## Architecture Overview

```
Encoding pipeline:
  ID → HWM2 frame → BCH encode → cross-band modulation → DWT/DCT embed

Decoding pipeline:
  Image → OpenCV preprocess → DWT/DCT read → cross-band vote → BCH decode → HWM2 parse → ID
```

## Frame Format: HWM2

```
Offset  Size   Field
──────  ────   ─────
0       4      Magic: "HWM2" (ASCII)
4       1      Version: 2
5       1      ID length (bytes), max 255
6       N      UTF-8 ID bytes
6+N     4      CRC32 checksum over bytes 0..6+N
```

Total overhead: 10 + id_len bytes. Compared to HWM1, the only change is the magic bytes. This is intentional — the format itself is simple and doesn't need structural changes. The robustness comes from BCH encoding applied to the bit stream before embedding.

## BCH Error Correction

### Parameters

Use systematic BCH codes over GF(2^m). Recommended configuration:

- **BCH(127, 64)**: 127-bit codeword, 64-bit payload, can correct up to 10 bit errors
- Code rate: ~0.5
- For the full payload: split into 64-bit blocks, encode each independently

### Implementation

New file: `src/bch.rs`

```
struct BchEncoder {
    // Precomputed generator polynomial
    gen_poly: Vec<u64>,
    m: usize,       // GF(2^m)
    t: usize,       // error correction capability
}

impl BchEncoder {
    fn encode(&self, data: &[bool]) -> Vec<bool>;      // data → codeword
    fn decode(&self, codeword: &[bool]) -> Option<Vec<bool>>;  // codeword → corrected data (None if uncorrectable)
}
```

Since Rust crates for BCH are limited, implement a focused BCH(127,64) encoder/decoder directly. The math is well-defined:

1. Generator polynomial for BCH(127,64) with t=10
2. Encoding: systematic form (data bits + parity bits)
3. Decoding: syndrome calculation → Berlekamp-Massey → Chien search → error correction

### Usage in the pipeline

```
Encode: payload_bits → bch.encode() → coded_bits → embed
Decode: read_bits → bch.decode() → payload_bits → parse_frame
```

## Cross-band Redundancy

### Frequency band pairs

Each bit is encoded using three DCT coefficient pairs:

| Band    | Coefficient pair | Frequency | Robustness characteristic |
|---------|-----------------|-----------|---------------------------|
| Low     | (1,0) vs (0,1)  | Low       | Survives blur, resampling |
| Mid     | (2,1) vs (1,2)  | Medium    | Balanced                  |
| High    | (3,2) vs (2,3)  | Medium-high | Current approach        |

### Embedding (encode)

For each payload bit at index `i`:

1. Select three DCT blocks from the block plan (blocks `3i`, `3i+1`, `3i+2`)
2. In block `3i`: embed bit using pair (1,0)-(0,1) with margin × 1.2
3. In block `3i+1`: embed bit using pair (2,1)-(1,2) with margin × 1.0
4. In block `3i+2`: embed bit using pair (3,2)-(2,3) with margin × 0.8

The low-frequency pair gets a higher margin because:
- Low-frequency coefficients have larger magnitudes naturally
- The JND constraint is tighter (more visible), but the absolute margin needs to be larger to survive attacks
- Perceptual masking still applies — skip blocks with very low variance

### Decoding (decode)

For each payload bit at index `i`:

1. Read diff from block `3i` pair (1,0)-(0,1) → `v_low`
2. Read diff from block `3i+1` pair (2,1)-(1,2) → `v_mid`
3. Read diff from block `3i+2` pair (3,2)-(2,3) → `v_high`
4. `bit = majority_vote(v_low > 0, v_mid > 0, v_high > 0)`
5. `confidence = weighted_sum(abs(v_low), abs(v_mid), abs(v_high))`

### Block plan generation

The block plan (which DCT blocks to use) is still key-derived via SHA256 + ChaCha20Rng shuffle. The difference is that 3× more blocks are needed per bit.

```
Current:  bit_count blocks total (1 block per bit)
New:      bit_count × 3 blocks total (3 blocks per bit)
```

## tile_size Default

Change default from 256 to 512.

Capacity with tile_size=512:

| Backend      | Total 8×8 blocks | After 3× redundancy | After BCH (~0.5 rate) | Max ID bytes |
|-------------|-------------------|---------------------|----------------------|--------------|
| jpeg_dct    | 4096              | 1365                | ~639 bits = 79 bytes | 79           |
| frequency_v2| 1024              | 341                 | ~130 bits = 16 bytes | 16           |

The recommended ID length remains ≤32 bytes for jpeg_dct and ≤12 bytes for frequency_v2.

## OpenCV Preprocessing Pipeline

### Integration

Rust CLI calls Python subprocess:

```
rust: encode_image()  → no change (pure Rust)
rust: decode_image()  → call python preprocess.py → read result → decode
```

Subprocess invocation:

```rust
Command::new("python")
    .arg("scripts/preprocess.py")
    .arg("--input").arg(input_path)
    .arg("--output").arg(temp_path)
    .output()?;
```

The preprocessed image is written to a temp file, then loaded and decoded normally.

### Python script: `scripts/preprocess.py`

```python
import cv2
import numpy as np
import argparse
import sys

def preprocess(input_path: str, output_path: str):
    img = cv2.imread(input_path)
    if img is None:
        sys.exit(1)

    # Step 1: Screen boundary detection
    img = detect_and_warp_screen(img)

    # Step 2: Moiré suppression
    img = suppress_moire(img)

    # Step 3: Color normalization
    img = normalize_color(img)

    # Step 4: Denoising
    img = cv2.fastNlMeansDenoisingColored(img, None, 10, 10, 7, 21)

    cv2.imwrite(output_path, img)
```

### Step details

**① Screen boundary detection**

```python
def detect_and_warp_screen(img):
    gray = cv2.cvtColor(img, cv2.COLOR_BGR2GRAY)
    edges = cv2.Canny(gray, 50, 150)
    contours, _ = cv2.findContours(edges, cv2.RETR_EXTERNAL, cv2.CHAIN_APPROX_SIMPLE)

    # Find largest quadrilateral
    for contour in sorted(contours, key=cv2.contourArea, reverse=True):
        peri = cv2.arcLength(contour, True)
        approx = cv2.approxPolyDP(contour, 0.02 * peri, True)
        if len(approx) == 4:
            pts = approx.reshape(4, 2)
            # Order: top-left, top-right, bottom-right, bottom-left
            rect = order_points(pts)
            # Warp to rectangle
            width, height = estimate_output_size(rect)
            dst = np.array([[0,0],[width,0],[width,height],[0,height]], dtype=np.float32)
            M = cv2.getPerspectiveTransform(rect.astype(np.float32), dst)
            return cv2.warpPerspective(img, M, (width, height))

    return img  # No screen detected, return as-is
```

**② Moiré suppression**

```python
def suppress_moire(img):
    # Convert to frequency domain
    gray = cv2.cvtColor(img, cv2.COLOR_BGR2GRAY)
    dft = cv2.dft(np.float32(gray), flags=cv2.DFT_COMPLEX_OUTPUT)
    dft_shift = np.fft.fftshift(dft, axes=[0,1])

    # Create notch filter to suppress moiré frequencies
    # Moiré appears as periodic peaks in the frequency domain
    rows, cols = gray.shape
    mask = np.ones((rows, cols, 2), np.float32)

    # Suppress high-frequency periodic patterns (screen grid artifacts)
    # Use adaptive thresholding on magnitude spectrum
    magnitude = cv2.magnitude(dft_shift[:,:,0], dft_shift[:,:,1])
    magnitude_log = np.log1p(magnitude)
    threshold = np.mean(magnitude_log) + 3 * np.std(magnitude_log)

    # Create circular notch masks at detected peaks
    peaks = np.where(magnitude_log > threshold)
    for y, x in zip(peaks[0], peaks[1]):
        # Skip DC component
        if abs(y - rows//2) < 10 and abs(x - cols//2) < 10:
            continue
        cv2.circle(mask, (x, y), 5, 0, -1)

    # Apply filter
    filtered = dft_shift * mask
    img_back = cv2.idft(np.fft.ifftshift(filtered, axes=[0,1]), flags=cv2.DFT_SCALE | cv2.DFT_REAL_OUTPUT)
    return cv2.cvtColor(np.uint8(np.clip(img_back, 0, 255)), cv2.COLOR_GRAY2BGR)
```

**③ Color normalization**

```python
def normalize_color(img):
    lab = cv2.cvtColor(img, cv2.COLOR_BGR2LAB)
    l, a, b = cv2.split(lab)
    # CLAHE on L channel (adaptive histogram equalization)
    clahe = cv2.createCLAHE(clipLimit=2.0, tileGridSize=(8,8))
    l = clahe.apply(l)
    lab = cv2.merge([l, a, b])
    return cv2.cvtColor(lab, cv2.COLOR_LAB2BGR)
```

## Changes to Existing Code

### `src/frame.rs`

- Change magic from `HWM1` to `HWM2`
- Update `HEADER_BYTES` if needed
- Update version byte to `2`

### `src/common.rs`

- Change `DEFAULT_TILE_SIZE` concept: update default in `RobustWatermarkOptions::default()` from 256 to 512
- Update `perceptual_mask` to be more nuanced (optional, can be done later)

### `src/backend/frequency_v2.rs`

- `frequency_block_plan`: generate 3× blocks (one set per frequency band)
- `embed_frequency_tile`: embed each bit using 3 different coefficient pairs
- `decode_frequency_bits`: read from 3 pairs, majority vote
- `frequency_tile_capacity_bits`: divide by 3

### `src/backend/jpeg_dct.rs`

- `jpeg_dct_block_plan`: generate 3× blocks
- `embed_jpeg_dct_tile`: embed each bit using 3 different coefficient pairs
- `decode_jpeg_dct_bits`: read from 3 pairs, majority vote
- `jpeg_dct_capacity_bits`: divide by 3

### `src/backend/mod.rs`

- No changes to trait definition

### `src/lib.rs`

- In `decode_image`: call OpenCV preprocessing before decode
- Add `preprocess_with_opencv()` function that invokes Python subprocess
- Use temp file for preprocessed image

### `src/main.rs`

- Add `--preprocess` flag to decode command (default: auto-detect if Python available)
- Add `--no-preprocess` flag to skip preprocessing

### New file: `src/bch.rs`

- BCH(127,64) encoder/decoder
- Generator polynomial computation
- Syndrome calculation
- Berlekamp-Massey algorithm
- Chien search for error locations

### New file: `scripts/preprocess.py`

- OpenCV preprocessing pipeline
- Screen detection, perspective correction, moiré suppression, color normalization, denoising

## Decision Log

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Frame format | HWM2 (no backward compat) | Clean break, simplify implementation |
| BCH parameters | BCH(127,64), t=10 | Good balance of correction capability and overhead |
| Redundancy scheme | 3 pairs per bit, cross-band | More resilient than same-band multi-block |
| Default tile_size | 512 | Needed for capacity after redundancy + BCH |
| OpenCV integration | Python subprocess | Flexibility for tuning, mature OpenCV Python API |
| Frequency pairs | (1,0)-(0,1), (2,1)-(1,2), (3,2)-(2,3) | Low/mid/high coverage |
