# Tasks: Polish and Optimize

## Task 1: Optimize DCT with butterfly algorithm

**File**: `src/midfreq.rs`

- [x] Implement butterfly DCT (Lee algorithm)
- [x] Keep precomputed cosine tables
- [x] Test roundtrip accuracy
- [ ] Measure speedup vs current

**Estimated effort**: 2 hours

---

## Task 2: Clean up Python prototype

- [x] Move `prototype/` to `archive/prototype/`
- [x] Keep `scripts/test_robustness.sh` in place
- [x] Note: `scripts/test_robustness.sh` still uses old CLI (`encode`/`decode`) — will update as part of README update

**Estimated effort**: 15 minutes

---

## Task 3: Update README.md

- [x] Update CLI commands (embed/detect/detect-batch)
- [x] Update architecture description (non-blind)
- [x] Add test results table
- [x] Remove blind-detection references
- [x] Update dependencies

**Estimated effort**: 30 minutes

---

## Task 4: Run tests and validate

- [x] Run unit tests
- [x] Run image tests with all 6 images
- [x] Verify performance improvement

**Results:**
- 7/7 tests pass
- DCT roundtrip accuracy verified
- Performance: ~341s (minor improvement from DCT optimization)
- Python prototype archived to `archive/prototype/`

**Estimated effort**: 30 minutes

---

## Dependency order

```
Task 1 (DCT) ──→ Task 4 (test)
Task 2 (cleanup) ── independent
Task 3 (README) ── independent
```
