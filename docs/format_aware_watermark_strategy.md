# Format-Aware Watermark Strategy

## Goal

Different image formats preserve and damage watermark signals in different ways. The tool should not force one algorithm to serve every output format. Instead, `encode` should choose a watermark backend according to the output format, while keeping the CLI simple.

The default user experience should be:

```powershell
hidden_watermark encode --input input.jpg --output marked.jpg --id "asset-123"
hidden_watermark decode --input marked.jpg
```

The tool should decide the right backend automatically from `--output`.

## Format Characteristics

### PNG

PNG is lossless. Pixel values are preserved exactly after saving.

Recommended approach:

- Prioritize invisibility.
- Use low-strength frequency-domain embedding.
- Avoid visible spatial patterns.
- Do not over-optimize for JPEG quantization.

Expected behavior:

- Very high visual quality.
- Stable decode after PNG save.
- Some tolerance to crop/rotation if a complete tile remains.
- Not guaranteed after converting to low-quality JPEG unless encoded with a stronger preset.

### Lossless WebP

Lossless WebP behaves closer to PNG than JPEG.

Recommended approach:

- Use the PNG-style invisible backend.
- Keep strength low.
- Treat it as a lossless output format when format metadata confirms lossless mode.

Expected behavior:

- Similar to PNG.
- Good invisibility.
- Stable decode after save.

### Lossy WebP

Lossy WebP is transform-based and will remove or quantize subtle high-frequency changes.

Recommended approach:

- Use a lossy-format backend, closer to JPEG strategy.
- Avoid tiny spatial/frequency changes that the encoder will discard.
- Increase redundancy more than PNG.

Expected behavior:

- Slightly lower visual quality than PNG mode.
- Better decode survival after lossy re-encoding.

### JPEG

JPEG is lossy and based on 8x8 DCT quantization. It can turn watermark energy into visible block noise, small dots, or a mosaic-like texture if the algorithm fights the encoder.

Recommended approach:

- Use a JPEG-specific backend.
- Embed in JPEG-friendly DCT coefficient pairs.
- Avoid smooth regions where artifacts are highly visible.
- Use content masking: stronger in textured areas, weaker in flat areas and faces/skin-like smooth tones.
- Tune strength based on requested JPEG quality.
- Prefer spreading payload across more blocks with smaller per-block changes instead of forcing large changes into fewer blocks.

Expected behavior:

- Lower visible noise than using the PNG-oriented frequency backend.
- Better JPEG survival than very weak invisible embedding.
- Still a tradeoff: stronger robustness can create visible artifacts.

## Backend Selection

Default backend mode should be `auto`.

```text
--backend auto
--backend frequency-v2
--backend jpeg-dct
```

Selection rule:

- Output `.png`: use `frequency-v2` invisible strategy.
- Output `.jpg` / `.jpeg`: use `jpeg-dct`.
- Output lossless `.webp`: use `frequency-v2`.
- Output lossy `.webp`: use lossy-format strategy, initially `jpeg-dct` or a shared lossy backend.
- Unknown output extension: require explicit `--backend`.

Important distinction:

- The output format matters more than the input format.
- JPEG input with PNG output should use PNG-style strategy.
- PNG input with JPEG output should use JPEG-style strategy.

## Presets

Presets should remain format-aware.

### `invisible`

Purpose:

- Best visual quality.
- Default for normal use.

Behavior:

- PNG: very low strength.
- JPEG: avoid smooth areas aggressively; use high JPEG quality by default.
- Decode robustness is good for direct save and light processing, not maximum attack resistance.

### `balanced`

Purpose:

- Reasonable visual quality with better survival after recompression.

Behavior:

- PNG: moderate frequency strength.
- JPEG: stronger DCT margin and more redundancy.
- Useful when images may be reposted or compressed again.

### `robust`

Purpose:

- Maximize decode survival.

Behavior:

- Accepts more quality loss.
- Uses more tiles and stronger coefficient margins.
- Should clearly warn that artifacts may become visible, especially for JPEG.

## JPEG-Specific Design Direction

The JPEG backend should not simply reuse `frequency_v2` unchanged.

Core design:

- Work on the Y channel.
- Split image into 8x8 blocks aligned with JPEG structure.
- Use mid-frequency coefficient pairs, not DC or very high frequency.
- Skip blocks with very low variance.
- Skip or weaken blocks near saturation or very smooth gradients.
- Use key-derived pseudo-random block selection.
- Encode the same short payload repeatedly across many eligible blocks.
- Decode by majority vote across block groups and validate with CRC.

Artifact reduction:

- Prefer smaller coefficient deltas over many blocks.
- Avoid changing neighboring blocks in a way that forms visible grids.
- Use a per-block just-noticeable-change estimate.
- If output quality is below a threshold, increase redundancy before increasing strength.

## PNG/Frequency Design Direction

The PNG-style backend can keep the current `frequency_v2` direction:

- YCbCr conversion.
- Modify only Y.
- 1-level Haar DWT.
- Embed in DCT coefficient pairs in the LL or selected detail subbands.
- Keep PSNR target high.

Improvements:

- Add a stricter visual mask for flat areas.
- Keep `PSNR >= 42 dB` for `invisible`.
- Keep `PSNR >= 40 dB` for `balanced`.
- Report PSNR and changed-pixel ratio on encode.

## CLI Defaults

Recommended defaults:

```text
--backend auto
--preset invisible
--strength auto
--tile-size auto
--overlap auto
```

For compatibility, explicit numeric values can still override auto behavior.

Suggested visible CLI:

```powershell
hidden_watermark encode --input input.png --output marked.png --id "asset-123"
hidden_watermark encode --input input.jpg --output marked.jpg --id "asset-123"
```

Advanced:

```powershell
hidden_watermark encode --input input.jpg --output marked.jpg --id "asset-123" --backend jpeg-dct --preset balanced --jpeg-quality 95
```

## Decode Strategy

Decode should try likely backends automatically:

1. If the image format is JPEG, try `jpeg-dct` first.
2. Try `frequency-v2`.
3. If diagnostics are enabled, report each backend attempt and confidence.

The decoded payload must still pass CRC. Low-confidence candidates without CRC success should not be reported as successful.

## Quality Metrics

Encode reports should include:

- `algorithm`
- `backend`
- `preset`
- `psnr`
- `changed_pixels_ratio`
- `tile_count`
- `id_bytes`

Manual regression should track:

- Decode success on original output.
- Decode success after JPEG q75.
- Decode success after 90 degree rotation.
- Decode success after 50%x50% crop.
- Decode success after 75% resize.
- PSNR against original.
- Visual artifact notes for JPEG.

## Acceptance Criteria

PNG invisible mode:

- PSNR >= 42 dB on the local image set.
- No obvious visible texture under normal viewing.
- Direct decode succeeds.

JPEG invisible mode:

- Clearly fewer dots/block artifacts than `frequency_v2` JPEG output.
- Direct decode succeeds.
- JPEG q75 decode succeeds for most normal photos.
- PSNR >= 38-40 dB, with visual inspection taking priority over PSNR alone.

Balanced mode:

- Direct decode and JPEG q75 decode should be more reliable than invisible mode.
- Some minor visual degradation is acceptable.

Robust mode:

- Visible artifacts are acceptable if documented.
- Decode survival is prioritized.

## Current Decision

The next implementation step should introduce `--backend auto` and a new `jpeg-dct` backend, while keeping `frequency_v2` for PNG/lossless-style output.

The output format should drive backend selection by default.
