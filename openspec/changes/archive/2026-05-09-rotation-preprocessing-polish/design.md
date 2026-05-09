# Design: Rotation Detection & Preprocessing Polish

## Rotation Detection

Add `detect_rotation(img)` to `preprocess.py`:

```
┌─────────────────────────────────────────────────────────┐
│                 detect_rotation(img)                     │
├─────────────────────────────────────────────────────────┤
│                                                         │
│  1. Convert to grayscale                                │
│  2. Edge detection (Canny)                              │
│  3. Hough line detection                                │
│  4. Collect line angles (0-180°)                        │
│  5. Find dominant angle (histogram peak)                │
│  6. Compute rotation correction                         │
│  7. Rotate image to correct                             │
│                                                         │
│  Angle convention:                                      │
│  - Lines at 0° or 90° → no rotation                     │
│  - Lines at 45° → could be 45° or -45°                  │
│  - Use median of angles near 0° or 90°                  │
│                                                         │
└─────────────────────────────────────────────────────────┘
```

Updated pipeline:
```
detect_and_warp_screen → detect_rotation → suppress_moire → normalize_color → denoise
```

## Script Embedding

```
┌─────────────────────────────────────────────────────────┐
│                  Compile time                            │
│  include_str!("../scripts/preprocess.py")               │
│  → embedded string in binary                            │
├─────────────────────────────────────────────────────────┤
│                  Runtime                                 │
│  1. Check if Python + cv2 available                     │
│  2. Write script to temp file                           │
│  3. Run: python temp_script.py --input ... --output ... │
│  4. Clean up temp file                                  │
│  5. If unavailable, warn and skip                       │
└─────────────────────────────────────────────────────────┘
```

## BCH (Deferred)

Keep current 1-3 error brute-force. PGZ for >3 errors stays as-is (buggy but harmless — falls through to "uncorrectable"). Cross-band redundancy handles most real-world cases.
