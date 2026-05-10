"""Embed watermark into images using spread spectrum in mid-frequency DCT coefficients."""

import argparse
import sys

import cv2
import numpy as np

from utils import (
    BLOCK_SIZE,
    apply_dct,
    apply_idct,
    calculate_psnr,
    compute_texture_weight,
    generate_prng_sequence,
    get_mid_freq_positions,
    load_image,
    save_image,
    split_into_blocks,
    reconstruct_from_blocks,
)


def embed_watermark(image: np.ndarray, key: str, strength: float = 0.5) -> tuple[np.ndarray, float]:
    """Embed watermark signal into image.

    Args:
        image: Input image (BGR, float64)
        key: Secret key for PRNG
        strength: Base embedding strength

    Returns:
        (watermarked_image, psnr)
    """
    # Convert to YCrCb
    ycrcb = cv2.cvtColor(image.astype(np.uint8), cv2.COLOR_BGR2YCrCb).astype(np.float64)
    y_channel = ycrcb[:, :, 0]

    # Split into blocks
    blocks, rows, cols = split_into_blocks(y_channel)
    mid_freq = get_mid_freq_positions()

    # Generate PRN sequence for all mid-frequency coefficients across all blocks
    total_coeffs = len(blocks) * len(mid_freq)
    prn_sequence = generate_prng_sequence(key, total_coeffs)

    # Embed
    modified_blocks = []
    coeff_idx = 0

    for block in blocks:
        # Apply DCT
        dct_block = apply_dct(block)

        # Compute texture weight
        weight = compute_texture_weight(block)

        # Embed in mid-frequency coefficients
        for r, c in mid_freq:
            alpha = strength * weight
            dct_block[r, c] += alpha * prn_sequence[coeff_idx]
            coeff_idx += 1

        # Inverse DCT
        modified_block = apply_idct(dct_block)
        modified_blocks.append(modified_block)

    # Reconstruct Y channel
    y_modified = reconstruct_from_blocks(modified_blocks, rows, cols)

    # Replace Y channel
    ycrcb_modified = ycrcb.copy()
    ycrcb[:, :, 0] = y_modified

    # Convert back to BGR
    result = cv2.cvtColor(np.clip(ycrcb, 0, 255).astype(np.uint8), cv2.COLOR_YCrCb2BGR).astype(np.float64)

    # Calculate PSNR
    psnr = calculate_psnr(image, result)

    return result, psnr


def main():
    parser = argparse.ArgumentParser(description="Embed watermark into image")
    parser.add_argument("--input", required=True, help="Input image path")
    parser.add_argument("--output", required=True, help="Output image path")
    parser.add_argument("--key", required=True, help="Secret key")
    parser.add_argument("--strength", type=float, default=0.5, help="Embedding strength (default: 0.5)")
    args = parser.parse_args()

    # Load image
    image = load_image(args.input)
    print(f"Image size: {image.shape[1]}x{image.shape[0]}")

    # Embed watermark
    watermarked, psnr = embed_watermark(image, args.key, args.strength)

    # Save result
    save_image(args.output, watermarked)
    print(f"Watermark embedded. PSNR: {psnr:.2f} dB")

    if psnr < 50:
        print(f"WARNING: PSNR {psnr:.2f} dB < 50 dB. Consider reducing strength.")
    else:
        print("PSNR > 50 dB: Watermark is invisible.")


if __name__ == "__main__":
    main()
