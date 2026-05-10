"""Detect watermark by comparing suspect image against original (non-blind detection)."""

import argparse
import os
import sys

import cv2
import numpy as np

from align import align_images, normalize_histogram
from utils import (
    BLOCK_SIZE,
    apply_dct,
    generate_prng_sequence,
    get_mid_freq_positions,
    load_image,
    split_into_blocks,
)

IMAGE_EXTENSIONS = {'.jpg', '.jpeg', '.png', '.bmp', '.tiff', '.webp'}


def detect_watermark(original: np.ndarray, suspect: np.ndarray, key: str,
                     fpr: float = 0.001, quiet: bool = False) -> tuple[bool, float, float]:
    """Detect watermark by comparing suspect to original.

    Args:
        original: Original image (BGR, float64)
        suspect: Suspect image (BGR, float64)
        key: Secret key used for embedding
        fpr: Target false positive rate (default 0.1%)
        quiet: Suppress alignment output

    Returns:
        (detected, score, threshold)
    """
    # Step 1: Align suspect to original
    aligned, params = align_images(original, suspect)
    if not quiet:
        print(f"  Alignment: rotation={params['rotation']:.1f}°, "
              f"shift=({params['shift_x']:.1f}, {params['shift_y']:.1f}), "
              f"score={params['alignment_score']:.3f}")

    # Step 2: Normalize histogram
    aligned = normalize_histogram(original.astype(np.uint8), aligned.astype(np.uint8))
    aligned = aligned.astype(np.float64)

    # Step 3: Convert to YCrCb, extract Y channel
    y_orig = cv2.cvtColor(original.astype(np.uint8), cv2.COLOR_BGR2YCrCb)[:, :, 0].astype(np.float64)
    y_suspect = cv2.cvtColor(aligned.astype(np.uint8), cv2.COLOR_BGR2YCrCb)[:, :, 0].astype(np.float64)

    # Step 4: Split into blocks
    blocks_orig, rows, cols = split_into_blocks(y_orig)
    blocks_suspect, _, _ = split_into_blocks(y_suspect)

    # Step 5: Compute correlation scores for each block
    mid_freq = get_mid_freq_positions()
    total_coeffs = len(blocks_orig) * len(mid_freq)
    prn_sequence = generate_prng_sequence(key, total_coeffs)

    block_scores = []
    coeff_idx = 0

    for i in range(len(blocks_orig)):
        # Apply DCT
        dct_orig = apply_dct(blocks_orig[i])
        dct_suspect = apply_dct(blocks_suspect[i])

        # Compute difference in mid-frequency coefficients
        block_score = 0.0
        for r, c in mid_freq:
            diff = dct_suspect[r, c] - dct_orig[r, c]
            block_score += diff * prn_sequence[coeff_idx]
            coeff_idx += 1

        block_score /= len(mid_freq)
        block_scores.append(block_score)

    # Step 6: Compute overall score
    block_scores = np.array(block_scores)
    score = np.mean(block_scores)

    # Step 7: Estimate threshold for target FPR
    # The threshold should be based on the expected noise level, not the score variance.
    # Under H0 (no watermark), the difference between suspect and original is just noise.
    # We estimate noise from the DCT coefficient differences.
    
    # Compute noise level from mid-frequency coefficient differences
    noise_levels = []
    coeff_idx = 0
    for i in range(len(blocks_orig)):
        dct_orig = apply_dct(blocks_orig[i])
        dct_suspect = apply_dct(blocks_suspect[i])
        for r, c in mid_freq:
            diff = dct_suspect[r, c] - dct_orig[r, c]
            noise_levels.append(abs(diff))
            coeff_idx += 1
    
    noise_levels = np.array(noise_levels)
    sigma_noise = np.median(noise_levels) * 1.4826  # MAD-based robust estimate
    
    # Threshold: for 0.1% FPR, threshold = 3.09 * sigma_noise / sqrt(N_blocks)
    # This is the expected standard deviation of the mean under H0
    from scipy.stats import norm
    n_blocks = len(blocks_orig)
    threshold = norm.ppf(1 - fpr) * sigma_noise / np.sqrt(n_blocks)

    # Decision
    detected = score > threshold

    return detected, score, threshold


def find_images_in_dir(directory: str) -> dict[str, str]:
    """Find all images in directory, return {basename: path}."""
    images = {}
    for filename in os.listdir(directory):
        name, ext = os.path.splitext(filename)
        if ext.lower() in IMAGE_EXTENSIONS:
            path = os.path.join(directory, filename)
            images[name] = path
    return images


def match_images(original_dir: str, suspect_dir: str) -> list[tuple[str, str, str]]:
    """Match images between directories by filename.

    Returns list of (name, original_path, suspect_path)
    """
    originals = find_images_in_dir(original_dir)
    suspects = find_images_in_dir(suspect_dir)

    matches = []
    for name in originals:
        if name in suspects:
            matches.append((name, originals[name], suspects[name]))

    return matches


def batch_detect(original_dir: str, suspect_dir: str, key: str, fpr: float = 0.001):
    """Run batch detection on matched image pairs."""
    matches = match_images(original_dir, suspect_dir)

    if not matches:
        print("No matching images found between directories.")
        return

    print(f"Found {len(matches)} matching image pairs")
    print()

    results = []
    detected_count = 0

    for name, orig_path, suspect_path in matches:
        print(f"Processing: {name}")

        try:
            original = load_image(orig_path)
            suspect = load_image(suspect_path)

            detected, score, threshold = detect_watermark(original, suspect, key, fpr, quiet=True)

            status = "DETECTED" if detected else "NOT_DETECTED"
            confidence = score / threshold if threshold > 0 else 0.0

            results.append({
                'name': name,
                'original': orig_path,
                'suspect': suspect_path,
                'detected': detected,
                'score': score,
                'threshold': threshold,
                'confidence': confidence,
            })

            if detected:
                detected_count += 1
                print(f"  → {status} (score={score:.4f}, confidence={confidence:.2f}x)")
            else:
                print(f"  → {status} (score={score:.4f})")

        except Exception as e:
            print(f"  → ERROR: {e}")
            results.append({
                'name': name,
                'original': orig_path,
                'suspect': suspect_path,
                'detected': False,
                'score': 0,
                'threshold': 0,
                'confidence': 0,
                'error': str(e),
            })

    # Summary
    print()
    print("=" * 60)
    print("SUMMARY")
    print("=" * 60)
    print(f"Total pairs:     {len(matches)}")
    print(f"Detected:        {detected_count}")
    print(f"Not detected:    {len(matches) - detected_count}")
    print(f"Detection rate:  {detected_count / len(matches):.1%}")

    return results


def main():
    parser = argparse.ArgumentParser(description="Detect watermark (non-blind)")
    parser.add_argument("--original", help="Original image path")
    parser.add_argument("--suspect", help="Suspect image path")
    parser.add_argument("--original-dir", help="Directory with original images")
    parser.add_argument("--suspect-dir", help="Directory with suspect images")
    parser.add_argument("--key", required=True, help="Secret key")
    parser.add_argument("--fpr", type=float, default=0.001, help="False positive rate (default: 0.001)")
    args = parser.parse_args()

    # Validate arguments
    if args.original_dir and args.suspect_dir:
        # Batch mode
        batch_detect(args.original_dir, args.suspect_dir, args.key, args.fpr)
    elif args.original and args.suspect:
        # Single image mode
        original = load_image(args.original)
        suspect = load_image(args.suspect)

        print(f"Original: {args.original} ({original.shape[1]}x{original.shape[0]})")
        print(f"Suspect:  {args.suspect} ({suspect.shape[1]}x{suspect.shape[0]})")
        print()

        detected, score, threshold = detect_watermark(original, suspect, args.key, args.fpr)

        print()
        print(f"Score:     {score:.4f}")
        print(f"Threshold: {threshold:.4f}")
        print()

        if detected:
            print(f"RESULT: WATERMARK DETECTED (confidence: {score / threshold:.2f}x threshold)")
        else:
            print("RESULT: No watermark detected")
    else:
        parser.error("Must provide either --original/--suspect or --original-dir/--suspect-dir")


if __name__ == "__main__":
    main()
