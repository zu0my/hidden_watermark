import argparse
import sys

import cv2
import numpy as np


def detect_rotation(img):
    """Detect and correct small rotation using Hough line detection."""
    gray = cv2.cvtColor(img, cv2.COLOR_BGR2GRAY)
    edges = cv2.Canny(gray, 50, 150)

    # Detect lines: rho=1, theta=1°, threshold=100, minLineLength=100, maxLineGap=10
    lines = cv2.HoughLinesP(edges, 1, np.pi / 180, threshold=100,
                            minLineLength=100, maxLineGap=10)
    if lines is None or len(lines) < 5:
        return img

    # Collect angles, normalize to [-45°, 45°]
    angles = []
    for line in lines:
        x1, y1, x2, y2 = line[0]
        angle = np.degrees(np.arctan2(y2 - y1, x2 - x1))
        # Normalize to [-90, 90)
        angle = angle % 180
        if angle > 90:
            angle -= 180
        # We care about lines near horizontal (0°) or vertical (±90°)
        # For horizontal lines, angle should be near 0°
        # For vertical lines, angle should be near ±90°
        # Map vertical angles to horizontal equivalent
        if abs(angle) > 45:
            angle = angle - 90 if angle > 0 else angle + 90
        angles.append(angle)

    if not angles:
        return img

    # Use median angle as the rotation estimate
    angles = np.array(angles)
    rotation_angle = np.median(angles)

    # Only correct if rotation is significant (>0.5°) and small (<20°)
    if abs(rotation_angle) < 0.5 or abs(rotation_angle) > 20:
        return img

    h, w = img.shape[:2]
    center = (w // 2, h // 2)
    M = cv2.getRotationMatrix2D(center, rotation_angle, 1.0)
    return cv2.warpAffine(img, M, (w, h), flags=cv2.INTER_LINEAR,
                          borderMode=cv2.BORDER_REPLICATE)


def detect_and_warp_screen(img):
    gray = cv2.cvtColor(img, cv2.COLOR_BGR2GRAY)
    edges = cv2.Canny(gray, 50, 150)
    contours, _ = cv2.findContours(edges, cv2.RETR_EXTERNAL, cv2.CHAIN_APPROX_SIMPLE)

    if not contours:
        return img

    for contour in sorted(contours, key=cv2.contourArea, reverse=True)[:5]:
        peri = cv2.arcLength(contour, True)
        approx = cv2.approxPolyDP(contour, 0.02 * peri, True)
        if len(approx) == 4:
            pts = approx.reshape(4, 2).astype(np.float32)
            rect = order_points(pts)
            width = int(max(
                np.linalg.norm(rect[0] - rect[1]),
                np.linalg.norm(rect[2] - rect[3]),
            ))
            height = int(max(
                np.linalg.norm(rect[0] - rect[3]),
                np.linalg.norm(rect[1] - rect[2]),
            ))
            if width < 100 or height < 100:
                continue
            dst = np.array(
                [[0, 0], [width, 0], [width, height], [0, height]], dtype=np.float32
            )
            M = cv2.getPerspectiveTransform(rect, dst)
            return cv2.warpPerspective(img, M, (width, height))

    return img


def order_points(pts):
    rect = np.zeros((4, 2), dtype=np.float32)
    s = pts.sum(axis=1)
    rect[0] = pts[np.argmin(s)]
    rect[2] = pts[np.argmax(s)]
    d = np.diff(pts, axis=1)
    rect[1] = pts[np.argmin(d)]
    rect[3] = pts[np.argmax(d)]
    return rect


def suppress_moire(img):
    gray = cv2.cvtColor(img, cv2.COLOR_BGR2GRAY)
    rows, cols = gray.shape

    dft = cv2.dft(np.float32(gray), flags=cv2.DFT_COMPLEX_OUTPUT)
    dft_shift = np.fft.fftshift(dft, axes=[0, 1])

    magnitude = cv2.magnitude(dft_shift[:, :, 0], dft_shift[:, :, 1])
    magnitude_log = np.log1p(magnitude)
    threshold = np.mean(magnitude_log) + 3 * np.std(magnitude_log)

    mask = np.ones((rows, cols, 2), np.float32)
    cy, cx = rows // 2, cols // 2

    peaks = np.where(magnitude_log > threshold)
    for y, x in zip(peaks[0], peaks[1]):
        if abs(y - cy) < 15 and abs(x - cx) < 15:
            continue
        cv2.circle(mask, (int(x), int(y)), 6, 0, -1)

    filtered = dft_shift * mask
    img_back = cv2.idft(
        np.fft.ifftshift(filtered, axes=[0, 1]),
        flags=cv2.DFT_SCALE | cv2.DFT_REAL_OUTPUT,
    )
    img_back = np.clip(img_back, 0, 255).astype(np.uint8)
    return cv2.cvtColor(img_back, cv2.COLOR_GRAY2BGR)


def normalize_color(img):
    lab = cv2.cvtColor(img, cv2.COLOR_BGR2LAB)
    l, a, b = cv2.split(lab)
    clahe = cv2.createCLAHE(clipLimit=2.0, tileGridSize=(8, 8))
    l = clahe.apply(l)
    lab = cv2.merge([l, a, b])
    return cv2.cvtColor(lab, cv2.COLOR_LAB2BGR)


def denoise(img):
    return cv2.fastNlMeansDenoisingColored(img, None, 10, 10, 7, 21)


def preprocess(input_path, output_path):
    img = cv2.imread(input_path)
    if img is None:
        print(f"Error: cannot read {input_path}", file=sys.stderr)
        sys.exit(1)

    img = detect_and_warp_screen(img)
    img = detect_rotation(img)
    img = suppress_moire(img)
    img = normalize_color(img)
    img = denoise(img)

    cv2.imwrite(output_path, img)


def main():
    parser = argparse.ArgumentParser(description="Preprocess image for watermark decoding")
    parser.add_argument("--input", required=True, help="Input image path")
    parser.add_argument("--output", required=True, help="Output image path")
    args = parser.parse_args()
    preprocess(args.input, args.output)


if __name__ == "__main__":
    main()
