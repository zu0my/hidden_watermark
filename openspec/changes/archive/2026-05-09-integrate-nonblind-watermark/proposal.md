# Proposal: Integrate Non-Blind Watermark into Rust

## Problem

Python prototype works well but is slow. Rust code has unused blind-detection system.

## Solution

1. Clean up Rust code (remove BCH, frame format, jpeg_dct)
2. Port Python prototype to Rust
3. Keep Python prototype for validation
4. Update CLI and tests

## Scope

- Delete: `src/bch.rs`, `src/frame.rs`, `src/backend/jpeg_dct.rs`
- Rewrite: `src/backend/frequency_v2.rs` → mid-frequency spread spectrum
- Rewrite: `src/lib.rs` → non-blind detection pipeline
- Rewrite: `src/main.rs` → new CLI (embed/detect)
- Rewrite: `tests/robust.rs` → new test suite
- Update: `README.md`

## Out of Scope

- Performance optimization (future)
- GUI (future)
