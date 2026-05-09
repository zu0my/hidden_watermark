# Robust Screen-to-Cam Watermarking

## Problem

The current watermarking system works well for digital-domain attacks (JPEG re-encode, cardinal rotation, crop) but fails when images are captured via screen-to-cam (手机拍屏). Screen photography introduces:

- **Moiré patterns** from screen pixel grid vs camera sensor grid interference
- **Perspective distortion** from non-perpendicular viewing angles
- **Lens distortion** (barrel/pincushion)
- **Auto-exposure and white balance shifts**
- **Focus/defocus blur**
- **Sensor noise**
- **Camera JPEG compression**

These attacks systematically destroy the DCT coefficient-pair sign relationships that the current encoding relies on. The single-frequency-band embedding (3,2)-(2,3) is particularly vulnerable because a narrowband attack can flip bit values across the entire payload.

## Solution

Three complementary improvements:

### 1. Cross-band redundancy

Instead of encoding each bit with a single DCT coefficient pair, use three pairs across different frequency bands:

- Low-frequency pair (1,0)-(0,1): survives blur and resampling
- Mid-frequency pair (2,1)-(1,2): balanced robustness
- High-frequency pair (3,2)-(2,3): current approach, good for fine detail

Each bit is encoded in all three bands. Decoding uses majority vote across bands. An attacker must破坏所有三个频带才能翻转一个 bit.

### 2. BCH error correction

Replace CRC32 (detection only) with BCH codes that can both detect and correct bit errors. This allows successful decoding even when some coefficient-pair readings are wrong due to noise or distortion.

### 3. OpenCV preprocessing pipeline

A Python subprocess performs image cleanup before watermark decoding:

- Screen boundary detection (quadrilateral detection)
- Perspective correction (warp to frontal view)
- Moiré suppression (frequency-domain notch filtering)
- Color normalization (histogram equalization in LAB space)
- Denoising (non-local means)

## Scope

### In scope

- New frame format (HWM2) — no backward compatibility with HWM1
- Cross-band redundancy in both `frequency_v2` and `jpeg_dct` backends
- BCH error correction module (new Rust file)
- Default tile_size increased to 512
- Python preprocessing script with OpenCV
- CLI integration (Rust calls Python subprocess)
- Updated tests

### Out of scope

- Learned/neural network watermarks
- Sync pattern embedding (geometric synchronization via embedded patterns)
- Real-time video watermarking
- Adversarial attack resistance
- Print-cam scenarios (photographing printed material)

## Impact

- **Capacity**: With tile_size=512, jpeg_dct supports ~32 byte IDs; frequency_v2 supports ~12 byte IDs
- **Latency**: OpenCV preprocessing adds ~1-3 seconds per decode
- **Dependencies**: Python 3 + opencv-python required for decode preprocessing
- **Compatibility**: HWM2 watermarks are not compatible with HWM1 decoders
