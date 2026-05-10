# Proposal: Fix Rotation Accuracy

## Problem

Current rotation alignment only works for ≤1°. The multi-scale search (64×64 + 256×256) with 5° coarse steps is insufficiently precise, leading to alignment errors that break detection for rotations >1°.

## Solution

Improve the rotation alignment algorithm:

1. **Larger coarse resolution**: 128×128 instead of 64×64 for better angle discrimination
2. **Finer coarse steps**: 2° instead of 5° for better initial angle guess
3. **Wider fine search**: ±4° instead of ±0.5° at 0.2° steps
4. **Skip fine alignment for rotated images**: large templates with rotation cause mismatch

## Scope

- Modify: `src/align.rs`
- Test: rotations 1°, 3°, 5°, 10°, 15°

## Out of Scope

- CLI changes
- Performance optimization
