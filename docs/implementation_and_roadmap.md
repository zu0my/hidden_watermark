# Hidden Watermark Implementation And Roadmap

## Current Implementation

This project is a Rust library plus CLI for adding and decoding robust image watermarks. The current implementation targets PNG/JPEG images and blind decoding, meaning decoding does not require the original unwatermarked image.

The implementation supports two watermarking backends, selected automatically based on output format:

**`frequency_v2`** (PNG/WebP lossless): Each image is converted to YCbCr, only the Y channel is modified, each tile is transformed with a 1-level Haar DWT, and payload bits are embedded into 8x8 DCT mid-frequency coefficient pairs in the DWT LL subband. Prioritizes invisibility.

**`jpeg_dct`** (JPEG/WebP lossy): Direct 8x8 DCT embedding on the Y channel without DWT decomposition. Uses JPEG-aligned blocks, content masking, and key-derived block selection. Produces fewer visible block artifacts when the output will be JPEG-recompressed.

Both backends use the same payload frame format:

- Magic bytes: `HWM2`
- Version: `2`
- Payload length
- UTF-8 ID bytes
- CRC32 checksum

Both backends use **cross-band redundancy**: each payload bit is encoded using three DCT coefficient pairs across different frequency bands — (1,0)-(0,1), (2,1)-(1,2), and (3,2)-(2,3). Decoding uses majority vote across the three bands, making the watermark more resilient to attacks that target specific frequency ranges (e.g., blur, noise, compression).

Decoding is blind and uses the same key-derived block plan to read coefficient-pair signs. CRC32 is used to reject unreliable results. Auto-decode tries both backends when `--backend auto` is used.

## Public Rust API

The library exposes these main types:

- `RobustWatermarkOptions`
- `EncodeOptions`
- `DecodeOptions`
- `EncodeReport`
- `DecodeReport`
- `CapacityReport`
- `BackendChoice`

Main entry points:

- `encode_image(input, output, id, options)`
- `decode_image(input, options)`
- `estimate_capacity(input, options)`

`RobustWatermarkOptions` includes:

- `key`: optional secret key. Empty or missing key uses a fixed public default.
- `strength`: controls watermark visibility and robustness.
- `preset`: `invisible`, `balanced`, or `robust`; default is `invisible`.
- `tile_size`: default is `512`.
- `overlap`: controls tile overlap; default is `0.0` because overlap currently weakens blind tile decoding.

`BackendChoice` controls which watermarking algorithm to use:

- `Auto` (default): selects `jpeg_dct` for JPEG output, `frequency_v2` for PNG/other.
- `FrequencyV2`: force the Haar DWT + DCT backend.
- `JpegDct`: force the direct 8x8 DCT backend.

`EncodeOptions` and `DecodeOptions` both include a `backend: BackendChoice` field.

## CLI

The CLI supports four commands and a `--backend` flag:

```powershell
hidden_watermark encode --input in.jpg --output out.jpg --id "asset-123" --key secret --preset invisible --strength 0.25 --tile-size 256 --jpeg-quality 92 --backend auto
```

```powershell
hidden_watermark decode --input out.jpg --key secret --backend auto
```

```powershell
hidden_watermark capacity --input in.jpg --tile-size 160
```

```powershell
hidden_watermark diagnose --input out.jpg --key secret
```

The `--backend` flag accepts `auto` (default), `frequency-v2`, or `jpeg-dct`. When `auto`, the backend is selected from the output file extension: `.jpg`/`.jpeg` uses `jpeg_dct`, `.png` and others use `frequency_v2`. For `decode` with `auto`, the tool tries the format-detected backend first, then falls back to the other.

`decode` prints the decoded ID and summary confidence in text mode. `diagnose` emits richer information, including attempts, confidence, tile hits, estimated rotation, estimated scale, and per-tile diagnostics when JSON output is requested.

## Verified Behavior

The current automated tests verify (16 tests total):

**frequency_v2 backend:**

- Frame encoding/decoding and CRC rejection.
- Same key produces a repeatable frequency block plan.
- Haar DWT and 8x8 DCT round-trip stability.
- Wrong key does not decode successfully.
- PNG image can be encoded and decoded.
- JPEG re-encoding can still decode.
- Cardinal rotation can still decode.
- A crop retaining about 25% of the image can still decode when it contains a complete embedded tile.
- Default invisible preset keeps PSNR above 40 dB in automated tests.
- Images smaller than the configured tile size are rejected by capacity estimation.

**jpeg_dct backend:**

- JPEG encode+decode with explicit `--backend jpeg-dct`.
- Wrong key does not decode with jpeg-dct.
- PSNR >= 35 dB for jpeg-dct invisible preset on JPEG output.
- Cardinal rotation with jpeg-dct backend.
- Auto backend selects `jpeg_dct` for `.jpg` output.
- Auto backend selects `frequency_v2` for `.png` output.
- Auto decode finds jpeg-dct watermark in JPEG files.

Verification commands used:

```powershell
cargo fmt --check
cargo clippy --all-features -- -D warnings
cargo test -- --test-threads=1
```

Tests are run with `--test-threads=1` because image-search integration tests are CPU-heavy on this Windows environment and parallel test execution can produce slow or noisy behavior.

## Current Limitations

The current implementation is a practical first version, not a full production-grade forensic watermarking system.

Known limitations:

- Short IDs are supported; arbitrary long text is intentionally not the target.
- The watermark is designed to be visually subtle; `encode` reports PSNR and changed-pixel ratio.
- Cropping is robust when the retained crop includes at least one complete embedded tile; arbitrary crop offsets are not fully solved.
- Scaling search and tile normalization exist in the code, but interpolated resize recovery is not yet stable enough to claim as verified.
- Phone re-photography, perspective distortion, blur, glare, moire, and exposure changes are not yet verified.
- The `opencv` feature exists as a build-time placeholder, but a real OpenCV preprocessing backend has not been implemented.
- The algorithm is not designed to resist malicious watermark removal.
- Capacity is lower than the spatial prototype because both backends prioritize invisibility; use short IDs.
- The `jpeg_dct` backend does not yet have verified robustness against JPEG q75 re-encoding (only direct decode is tested).
- Auto-decode fallback adds latency since it may try both backends sequentially.

## Roadmap

### Phase 1: Stabilize Geometric Sync

- [x] Introduce `jpeg_dct` backend with format-aware auto selection.
- [x] Add `--backend` CLI flag with `auto`, `frequency-v2`, and `jpeg-dct` options.
- [x] Refactor into modular architecture (`frame`, `common`, `backend/` modules).
- [x] Cross-band redundancy: encode each bit using 3 frequency pairs for robustness.
- [x] HWM2 frame format (not backward compatible with HWM1).
- [x] Default tile_size increased to 512.
- [ ] Add robust tile synchronization for arbitrary crop offsets.
- [ ] Improve scale recovery by normalizing candidate regions more accurately.
- [ ] Add small-angle rotation search beyond 0/90/180/270 degrees.
- [ ] Add confidence thresholds that distinguish weak candidate hits from reliable decodes.
- [ ] Optimize the search path so failed decodes do not require expensive exhaustive scans.

### Phase 2: Improve Robustness

- [x] BCH(127,64) encoder/decoder module (implemented but not yet wired into pipeline).
- [ ] Fix BCH encoding bug (syndromes non-zero for valid codewords).
- [ ] Integrate BCH into encode/decode pipeline.
- [ ] Add optional LH/HL secondary layers for robust preset only.
- [ ] Add multi-scale watermark layers so both screenshots and resized images have independent recovery paths.
- [ ] Add more realistic degradation tests: blur, brightness/contrast shifts, JPEG quality variation, screenshot-like resampling, and partial crops with random offsets.

### Phase 3: Optional OpenCV Backend

- [x] Python preprocessing script (`scripts/preprocess.py`) with screen detection, perspective correction, moiré suppression, color normalization, and denoising.
- [x] CLI integration: `--no-preprocess` flag, graceful fallback when Python unavailable.
- [ ] Add automatic crop and rotation normalization.
- [ ] Add diagnostics that report detected quadrilateral, perspective confidence, and preprocessing steps.

### Phase 4: Production Hardening

- Define stable JSON output schemas for CLI automation.
- Add benchmark tests for decode speed.
- Add corpus-based image tests with photographic samples.
- Add README examples and release packaging.
- Decide whether to expose advanced tuning parameters or keep them internal.

## Practical Guidance

For current use, prefer:

- Short IDs, ideally under 16-22 bytes with the default `tile-size 512`.
- Large source images.
- PNG output: uses `frequency_v2` automatically (best invisibility).
- JPEG output: uses `jpeg_dct` automatically (fewer block artifacts).
- `--preset invisible --strength 0.25 --tile-size 256` for normal invisible use.
- `--preset balanced` or higher `--strength` when the image may be re-encoded heavily.
- Keeping at least one complete `tile_size x tile_size` region after cropping.
- Use `--backend auto` (the default) to let the tool pick the best backend for the output format.

Do not currently rely on the tool as the sole mechanism for adversarial proof, legal evidence, or guaranteed recovery after phone re-photography.
