# Hidden Watermark

Robust image watermark encoder and blind decoder. Designed for screen-to-cam scenarios where watermarked images are displayed on screens and captured by phone cameras.

## Features

- **Cross-band redundancy** — Embeds watermark across multiple frequency bands for robustness
- **BCH error correction** — Corrects up to 3 bit errors per 127-bit codeword
- **Scale detection** — Decodes watermarks from resized images (50%-200%)
- **Rotation support** — Cardinal rotations (0°, 90°, 180°, 270°) + arbitrary rotation via preprocessing
- **Two backends**:
  - `frequency_v2` — Wavelet + DCT based, robust for screen-to-cam (default)
  - `jpeg_dct` — JPEG-domain DCT, higher capacity but less robust

## Installation

```bash
cargo build --release
```

The binary is at `target/release/hidden_watermark`.

## Usage

### Encode

```bash
hidden_watermark encode \
  --input image.png \
  --output watermarked.png \
  --id "MyWatermarkID" \
  --key "secret-key"
```

### Decode

```bash
hidden_watermark decode \
  --input watermarked.png \
  --key "secret-key"
```

### Options

| Option | Default | Description |
|--------|---------|-------------|
| `--key` | (none) | Encryption key (required for security) |
| `--strength` | 0.25 | Watermark strength (0.0-1.0) |
| `--tile-size` | 512 | Tile size in pixels |
| `--preset` | invisible | `invisible`, `balanced`, `robust` |
| `--backend` | auto | `auto`, `frequency-v2`, `jpeg-dct` |
| `--cross-band` | 3 | Number of frequency bands (1-3) |

## Robustness

Tested with ImageMagick transforms on images ≥2560×1440:

| Transform | Result |
|-----------|--------|
| Clean | ✓ Always works |
| Blur 1-2px | ✓ Survives |
| JPEG q40-q75 | ✓ Survives |
| Resize 50-75% | ✓ Survives |
| Brightness ±20% | ✓ Survives |
| Contrast ±20% | ✓ Survives |
| Noise (σ=0.02) | ✓ Survives |
| Screen-to-cam simulation | ✓ Survives |
| Rotation ±5° | ✓ With preprocessing (requires OpenCV) |
| Aggressive (blur3+JPEG30) | ✗ Fails |

**Minimum recommended image size:** 2560×1440

## Preprocessing (Optional)

For screen-captured images, install Python + OpenCV for preprocessing:

```bash
pip install opencv-python
```

Preprocessing includes:
- Screen detection and perspective correction
- Rotation correction (Hough transform)
- Moiré pattern suppression
- Color normalization
- Denoising

The preprocessing script is embedded in the binary. If OpenCV is unavailable, decoding proceeds without preprocessing (graceful degradation).

## Testing

### Unit & Integration Tests

```bash
cargo test
```

### Robustness Tests

```bash
scripts/test_robustness.sh
```

Options:
- `--backend frequency-v2` — Test only one backend
- `--transforms clean,blur1,jpeg75` — Test specific transforms
- `--image path/to/image.png` — Test one image

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│                    CLI (main.rs)                         │
├─────────────────────────────────────────────────────────┤
│                    Core (lib.rs)                         │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐              │
│  │ Encode   │  │ Decode   │  │ Preproc  │              │
│  └────┬─────┘  └────┬─────┘  └────┬─────┘              │
│       │              │              │                    │
├───────┴──────────────┴──────────────┴───────────────────┤
│                 Backends (backend/)                      │
│  ┌──────────────┐  ┌──────────────┐                    │
│  │ frequency_v2 │  │   jpeg_dct   │                    │
│  └──────────────┘  └──────────────┘                    │
├─────────────────────────────────────────────────────────┤
│  Frame (frame.rs)  │  BCH (bch.rs)  │  Common (common.rs)│
└─────────────────────────────────────────────────────────┘
```

## License

MIT
