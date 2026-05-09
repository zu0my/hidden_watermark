# Proposal: Image Robustness Tests

## Problem

No automated test suite exists for verifying watermark robustness across real images and ImageMagick transforms. Testing is manual and ad-hoc.

## Solution

Create a shell script `scripts/test_robustness.sh` that:
1. Encodes watermarks in all images under `assets/images/`
2. Applies a fixed set of ImageMagick transforms
3. Decodes and verifies each result
4. Outputs a pass/fail table

## Scope

- Test all images in `assets/images/` (6 images, sizes 1080×1066 to 5608×3078)
- Test both `frequency_v2` and `jpeg_dct` backends
- 13 transforms: clean, blur, JPEG, resize, brightness, contrast, noise, screen-to-cam, aggressive
- Pass criteria: decode success with matching message

## Out of scope

- Removing jpeg_dct backend (deferred)
- CI integration (future)
- Performance benchmarking (future)
