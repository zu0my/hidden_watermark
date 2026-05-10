# Proposal: Non-Blind Watermark Prototype

## Problem

Current watermark system uses blind decoding (no access to original image), requiring high signal strength, complex error correction, and large tile sizes. This limits robustness against cropping, rotation, and compression.

## Solution

Restructure to **non-blind detection**: embed a very weak watermark signal in mid-frequency DCT coefficients, then detect by comparing suspect image against the original.

### Key Advantages
- **Invisible**: PSNR > 50dB (completely imperceptible)
- **Robust**: Survives cropping, rotation, JPEG compression, color adjustments
- **Simple**: No BCH error correction, no frame format, no complex tile management
- **Anti-theft focused**: Designed for proving ownership, not extracting payload

## Use Cases

1. **Direct comparison**: Have suspect's uploaded file → compare with original
2. **Downloaded image**: Download suspect's image → align + compare
3. **Screenshot**: Screenshot of suspect's page → align + compare (lower priority)

## Scope

- Python prototype for algorithm validation
- Embed: mid-frequency DCT spread spectrum
- Detect: image alignment + correlation-based detection
- Batch processing support
- 0.1% false positive rate target

## Out of Scope

- Rust implementation (future change)
- Legal evidence output (internal judgment only)
- Real-time processing
