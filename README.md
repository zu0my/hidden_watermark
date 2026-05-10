# Hidden Watermark

鲁棒图像水印编码器和非盲检测器，用于防盗用。

Robust image watermark encoder and non-blind detector for anti-theft protection.

## Features

- **Invisible watermark**: PSNR > 40 dB (imperceptible)
- **Robust detection**: Survives blur, JPEG compression, resize, brightness/contrast
- **Non-blind detection**: Compare suspect image against original
- **Batch processing**: Process multiple image pairs at once
- **0.1% false positive rate**: Configurable detection threshold

## Installation

```bash
cargo build --release
```

## Usage

### Embed watermark

```bash
hidden_watermark embed \
  --input photo.jpg \
  --output watermarked.png \
  --key "my-secret-key"
```

### Detect watermark (single)

```bash
hidden_watermark detect \
  --original photo.jpg \
  --suspect stolen.jpg \
  --key "my-secret-key"
```

### Detect watermark (batch)

```bash
hidden_watermark detect-batch \
  --original-dir ./my_photos/ \
  --suspect-dir ./stolen_photos/ \
  --key "my-secret-key"
```

## How It Works

### Embedding
1. Convert image to YCrCb, extract luminance (Y) channel
2. Split Y channel into 16×16 blocks
3. Apply DCT to each block
4. Add pseudo-random signal to mid-frequency coefficients (zigzag 8-24)
5. Inverse DCT and convert back to RGB

### Detection
1. Align suspect to original (multi-scale rotation + template matching)
2. Normalize histogram (compensate brightness/contrast)
3. Extract DCT coefficients from both images
4. Compute correlation with pseudo-random sequence
5. Apply threshold for 0.1% false positive rate

## Robustness

Verified via automated tests on real images:

| Attack | Result |
|--------|--------|
| Clean (no attack) | ✓ Detected |
| JPEG q90 | ✓ Detected |
| JPEG q75 | ✓ Detected |
| JPEG q50 | ✓ Detected |
| Rotation 2° | ✓ Detected |
| Rotation 5° | ✓ Detected |
| Scale 90% | ✓ Detected |
| Brightness +20% | ✓ Detected |
| Contrast +20% | ✓ Detected |

## Architecture

```
src/
├── midfreq.rs    DCT, PRNG, zigzag selection, perceptual weighting
├── align.rs      Image alignment (ORB feature matching + template matching)
├── lib.rs        Core: embed_watermark(), detect_watermark()
└── main.rs       CLI: embed / detect / detect-batch
```

## Known Limitations

- **Non-blind**: Requires original image for detection
- **Alignment confidence**: Results are flagged as unreliable when alignment fails (< 0.2 confidence)
- **Blur**: Strong Gaussian blur can degrade watermark signal below detection threshold

## Testing

```bash
# All tests (run sequentially to avoid CPU contention)
cargo test -- --test-threads=1
```

## License

MIT
