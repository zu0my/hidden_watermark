# Design: Image Robustness Tests

## Architecture

Single bash script `scripts/test_robustness.sh` that orchestrates the test pipeline.

```
┌─────────────────────────────────────────────────────────┐
│                    test_robustness.sh                    │
├─────────────────────────────────────────────────────────┤
│                                                         │
│  For each image in assets/images/:                      │
│    For each backend (frequency_v2, jpeg_dct):           │
│      1. Encode watermark with random ID                 │
│      2. For each transform:                             │
│           a. Apply transform via ImageMagick            │
│           b. Decode watermark                           │
│           c. Compare decoded ID to original             │
│           d. Record pass/fail + confidence              │
│      3. Clean up temp files                             │
│                                                         │
│  Output: summary table                                  │
│                                                         │
└─────────────────────────────────────────────────────────┘
```

## Transform Matrix

| # | Transform | ImageMagick Command | Why |
|---|-----------|---------------------|-----|
| 1 | Clean | (none) | Baseline |
| 2 | Blur 1px | `-blur 0x1` | Camera focus |
| 3 | Blur 2px | `-blur 0x2` | Defocus |
| 4 | JPEG q75 | `-quality 75` | Phone camera |
| 5 | JPEG q60 | `-quality 60` | Low quality |
| 6 | JPEG q40 | `-quality 40` | Aggressive |
| 7 | Resize 75% | `-resize 75%` | Zoom |
| 8 | Resize 50% | `-resize 50%` | Far away |
| 9 | Brightness +20% | `-modulate 120` | Screen brightness |
| 10 | Contrast +20% | `-brightness-contrast 20x20` | Ambient light |
| 11 | Noise | `-attenuate 0.02 +noise Gaussian` | Sensor noise |
| 12 | Screen-to-cam | blur1+JPEG60+noise+modulate110 | Realistic |
| 13 | Aggressive | blur3+JPEG30+noise+resize75% | Stress test |

## Pass Criteria

- **Pass**: Decoded ID matches encoded ID exactly
- **Fail**: Decode fails or ID mismatch
- **Confidence**: Recorded but not used for pass/fail (informational)

## Output Format

```
┌────────────────────┬──────────┬──────────┬──────────┬──────────┐
│ Image              │ Backend  │ Clean    │ Screen   │ Aggress  │
├────────────────────┼──────────┼──────────┼──────────┼──────────┤
│ goku.png           │ freq_v2  │ ✓ 1.00   │ ✓ 1.00   │ ✗        │
│ goku.png           │ jpeg_dct │ ✓ 1.00   │ ✗        │ ✗        │
│ Forza.png          │ freq_v2  │ ✓ 1.00   │ ✓ 1.00   │ ?        │
│ ...                │          │          │          │          │
└────────────────────┴──────────┴──────────┴──────────┴──────────┘
```
