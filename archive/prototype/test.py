"""Validation tests for non-blind watermark prototype."""

import os
import sys
import tempfile

import cv2
import numpy as np

# Add prototype directory to path
sys.path.insert(0, os.path.dirname(__file__))

from embed import embed_watermark
from detect import detect_watermark
from utils import load_image, save_image, calculate_psnr


TEST_KEY = "test_secret_key_123"
TEST_IMAGE = None  # Will be set from command line


def create_test_image(width=1920, height=1080):
    """Create a synthetic test image with various textures."""
    img = np.zeros((height, width, 3), dtype=np.uint8)

    # Gradient background
    for y in range(height):
        for x in range(width):
            img[y, x] = [
                int(128 + 50 * np.sin(x / 50)),
                int(128 + 50 * np.cos(y / 50)),
                int(128 + 30 * np.sin((x + y) / 70))
            ]

    # Add some rectangles (different textures)
    cv2.rectangle(img, (100, 100), (400, 400), (255, 0, 0), -1)
    cv2.rectangle(img, (500, 200), (800, 500), (0, 255, 0), -1)
    cv2.rectangle(img, (1000, 300), (1400, 700), (0, 0, 255), -1)

    # Add some text
    cv2.putText(img, "Test Image", (width // 2 - 100, height // 2),
                cv2.FONT_HERSHEY_SIMPLEX, 2, (255, 255, 255), 3)

    return img.astype(np.float64)


def test_invisibility(image_path: str):
    """Test that watermark is invisible (PSNR > 50dB)."""
    print("=" * 60)
    print("TEST: Invisibility (PSNR > 50dB)")
    print("=" * 60)

    image = load_image(image_path)
    watermarked, psnr = embed_watermark(image, TEST_KEY, strength=0.5)

    print(f"  PSNR: {psnr:.2f} dB")
    if psnr > 50:
        print("  ✓ PASS: Watermark is invisible")
    else:
        print("  ✗ FAIL: Watermark may be visible")
        # Try lower strength
        for strength in [0.3, 0.2, 0.1]:
            watermarked, psnr = embed_watermark(image, TEST_KEY, strength=strength)
            print(f"  Trying strength={strength}: PSNR={psnr:.2f} dB")
            if psnr > 50:
                print(f"  ✓ PASS with strength={strength}")
                return strength

    return 0.5


def test_detection_clean(image_path: str, strength: float = 0.5):
    """Test detection on clean watermarked image."""
    print()
    print("=" * 60)
    print("TEST: Detection on clean watermarked image")
    print("=" * 60)

    image = load_image(image_path)
    watermarked, _ = embed_watermark(image, TEST_KEY, strength)

    detected, score, threshold = detect_watermark(image, watermarked, TEST_KEY)

    print(f"  Score: {score:.4f}, Threshold: {threshold:.4f}")
    if detected:
        print("  ✓ PASS: Watermark detected in clean image")
    else:
        print("  ✗ FAIL: Watermark not detected in clean image")

    return detected


def test_robustness(image_path: str, strength: float = 0.5):
    """Test robustness against various transforms."""
    print()
    print("=" * 60)
    print("TEST: Robustness against transforms")
    print("=" * 60)

    image = load_image(image_path)
    watermarked, _ = embed_watermark(image, TEST_KEY, strength)

    transforms = {
        'blur_1px': lambda img: cv2.GaussianBlur(img, (5, 5), 0),
        'blur_2px': lambda img: cv2.GaussianBlur(img, (9, 9), 0),
        'brightness+20%': lambda img: np.clip(img * 1.2, 0, 255),
        'brightness-20%': lambda img: np.clip(img * 0.8, 0, 255),
        'contrast+20%': lambda img: np.clip((img - 128) * 1.2 + 128, 0, 255),
    }

    results = {}
    for name, transform in transforms.items():
        transformed = transform(watermarked.copy())
        detected, score, threshold = detect_watermark(image, transformed, TEST_KEY)
        results[name] = (detected, score, threshold)
        status = "✓ PASS" if detected else "✗ FAIL"
        print(f"  {name}: {status} (score={score:.4f}, threshold={threshold:.4f})")

    return results


def test_jpeg_robustness(image_path: str, strength: float = 0.5):
    """Test robustness against JPEG compression."""
    print()
    print("=" * 60)
    print("TEST: JPEG compression robustness")
    print("=" * 60)

    image = load_image(image_path)
    watermarked, _ = embed_watermark(image, TEST_KEY, strength)

    results = {}
    for quality in [90, 75, 60, 40]:
        # Save as JPEG and reload
        with tempfile.NamedTemporaryFile(suffix='.jpg', delete=False) as f:
            temp_path = f.name
            cv2.imwrite(temp_path, np.clip(watermarked, 0, 255).astype(np.uint8),
                       [cv2.IMWRITE_JPEG_QUALITY, quality])
            jpeg_image = load_image(temp_path)
            os.unlink(temp_path)

        detected, score, threshold = detect_watermark(image, jpeg_image, TEST_KEY)
        results[f'jpeg_q{quality}'] = (detected, score, threshold)
        status = "✓ PASS" if detected else "✗ FAIL"
        print(f"  JPEG q{quality}: {status} (score={score:.4f}, threshold={threshold:.4f})")

    return results


def test_resize_robustness(image_path: str, strength: float = 0.5):
    """Test robustness against resizing."""
    print()
    print("=" * 60)
    print("TEST: Resize robustness")
    print("=" * 60)

    image = load_image(image_path)
    watermarked, _ = embed_watermark(image, TEST_KEY, strength)

    h, w = watermarked.shape[:2]
    results = {}

    for scale in [0.75, 0.5]:
        new_w = int(w * scale)
        new_h = int(h * scale)
        resized = cv2.resize(watermarked.astype(np.uint8), (new_w, new_h))
        # Resize back to original
        resized_back = cv2.resize(resized, (w, h)).astype(np.float64)

        detected, score, threshold = detect_watermark(image, resized_back, TEST_KEY)
        results[f'resize_{int(scale*100)}%'] = (detected, score, threshold)
        status = "✓ PASS" if detected else "✗ FAIL"
        print(f"  Resize {int(scale*100)}%: {status} (score={score:.4f}, threshold={threshold:.4f})")

    return results


def test_false_positives(image_path: str, num_tests: int = 10):
    """Test false positive rate on non-watermarked images."""
    print()
    print("=" * 60)
    print("TEST: False positives on non-watermarked images")
    print("=" * 60)

    image = load_image(image_path)
    h, w = image.shape[:2]

    false_positives = 0
    for i in range(num_tests):
        # Create random "suspect" image
        random_image = np.random.randint(0, 256, (h, w, 3), dtype=np.uint8).astype(np.float64)

        # Add some structure (not pure noise)
        random_image = cv2.GaussianBlur(random_image, (21, 21), 0)

        detected, score, threshold = detect_watermark(image, random_image, TEST_KEY)
        if detected:
            false_positives += 1
            print(f"  Test {i+1}: FALSE POSITIVE (score={score:.4f})")
        else:
            print(f"  Test {i+1}: Correct rejection (score={score:.4f})")

    fpr = false_positives / num_tests
    print(f"\n  False positive rate: {fpr:.1%} ({false_positives}/{num_tests})")

    if fpr <= 0.001:
        print("  ✓ PASS: FPR ≤ 0.1%")
    else:
        print("  ✗ FAIL: FPR > 0.1%")

    return fpr


def main():
    import argparse
    parser = argparse.ArgumentParser(description="Run validation tests")
    parser.add_argument("--image", required=True, help="Test image path")
    parser.add_argument("--quick", action="store_true", help="Run quick tests only")
    args = parser.parse_args()

    print("Non-Blind Watermark Prototype - Validation Tests")
    print("=" * 60)

    # Test 1: Invisibility
    strength = test_invisibility(args.image)

    # Test 2: Clean detection
    test_detection_clean(args.image, strength)

    # Test 3: Transform robustness
    test_robustness(args.image, strength)

    # Test 4: JPEG robustness
    test_jpeg_robustness(args.image, strength)

    if not args.quick:
        # Test 5: Resize robustness
        test_resize_robustness(args.image, strength)

        # Test 6: False positives
        test_false_positives(args.image, num_tests=5)

    print()
    print("=" * 60)
    print("Tests complete!")


if __name__ == "__main__":
    main()
