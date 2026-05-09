# Proposal: Rotation Detection & Preprocessing Polish

## Problem

1. Arbitrary rotation (±5°, common in screen-to-cam) breaks decode
2. Preprocessing script not bundled with binary
3. BCH decoder for >3 errors is buggy (deferred — cross-band redundancy covers it)

## Solution

1. Add Hough transform rotation detection to `preprocess.py`
2. Embed `preprocess.py` at compile time, extract at runtime
3. Document that Python + OpenCV are optional but recommended

## Scope

- Modify: `scripts/preprocess.py` (add rotation detection)
- Modify: `src/lib.rs` (embed script, extract at runtime)
- Document: Python/OpenCV dependency in README

## Out of scope

- BCH PGZ decoder fix (cross-band redundancy is sufficient)
- Pure Rust preprocessing (too complex, Python is pragmatic)
