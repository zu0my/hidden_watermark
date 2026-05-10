# Proposal: Polish and Optimize

## Work Items

1. **Performance**: ~360s test time. Optimize DCT with precomputed butterfly tables. Consider using `image` crate's built-in resize for alignment.
2. **Cleanup Python prototype**: Remove `prototype/` directory or archive it. All functionality is now in Rust.
3. **Update README**: Reflect new CLI, architecture, and test results.

## Scope

- `src/midfreq.rs` — DCT optimization
- `src/align.rs` — Use `image` crate resize
- `prototype/` — Cleanup
- `docs/`, `README.md` — Documentation updates
- `tests/robust.rs` — Update test

## Out of Scope

- New features
- Architectural changes
