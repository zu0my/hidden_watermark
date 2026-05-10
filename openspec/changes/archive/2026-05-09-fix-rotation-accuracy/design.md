# Design: Fix Rotation Accuracy

## Current Algorithm

```
Coarse: 64×64, 5° steps, ±20°
  ↓ (find best angle ± 5°)
Fine: 256×256, 0.5° steps, ±0.5° from best
  ↓ (find best angle ± 0.1°)
Super-fine: 256×256, 0.1° steps, ±0.5° from best
```

**Problem**: 64×64 is too small to distinguish 5° rotation differences reliably.

## Improved Algorithm

```
Coarse: 128×128, 2° steps, ±20°
  ↓ (find best angle ± 4°)
Fine: 256×256, 0.2° steps, ±4° from best
```

### Why this works better

- 128×64 has 4× the pixels of 64×64 → better angle discrimination
- 2° coarse steps vs 5° → initial guess is much closer
- 0.2° fine steps with ±4° range → wide enough to catch any coarse error
- Removing the super-fine step saves time while maintaining accuracy

### Additional Fix

The `fine_align` function performs template matching on the full-resolution image. For rotated images, the border pixels (filled with BORDER_REFLECT) can cause the template matching to find a false offset. Solution: skip fine alignment when rotation is detected, since rotation correction already aligns the images well enough.

```
If |best_angle| > 0.5°:
  skip fine_alignment (rotation already corrected)
else:
  run fine_alignment (small translation only)
```
