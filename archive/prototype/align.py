"""Image alignment functions for non-blind watermark detection."""

import cv2
import numpy as np


def align_images(original: np.ndarray, suspect: np.ndarray,
                 rotation_range: float = 15.0, rotation_step: float = 1.0
                 ) -> tuple[np.ndarray, dict]:
    """Align suspect image to original.

    Args:
        original: Original image (BGR, float64)
        suspect: Suspect image (BGR, float64)
        rotation_range: Max rotation angle to try (degrees)
        rotation_step: Rotation angle step (degrees)

    Returns:
        (aligned_image, transform_params)
    """
    # Convert to grayscale
    gray_orig = cv2.cvtColor(original.astype(np.uint8), cv2.COLOR_BGR2GRAY).astype(np.float64)
    gray_suspect = cv2.cvtColor(suspect.astype(np.uint8), cv2.COLOR_BGR2GRAY).astype(np.float64)

    # Step 1: Resize suspect to match original dimensions
    h_orig, w_orig = gray_orig.shape[:2]
    h_sus, w_sus = gray_suspect.shape[:2]

    if (h_sus, w_sus) != (h_orig, w_orig):
        scale_x = w_orig / w_sus
        scale_y = h_orig / h_sus
        gray_suspect = cv2.resize(gray_suspect, (w_orig, h_orig))
        suspect_resized = cv2.resize(suspect.astype(np.uint8), (w_orig, h_orig)).astype(np.float64)
    else:
        scale_x = scale_y = 1.0
        suspect_resized = suspect.copy()

    # Step 2: Estimate rotation by template matching (three-stage search)
    downsample_size = (256, 256)
    gray_orig_small = cv2.resize(gray_orig, downsample_size).astype(np.float32)
    gray_suspect_small = cv2.resize(gray_suspect, downsample_size).astype(np.float32)

    best_angle = 0.0
    best_score = -1.0

    # Stage 1: Coarse search with 3° steps
    coarse_angles = np.arange(-rotation_range, rotation_range + 3.0, 3.0)
    for angle in coarse_angles:
        center_small = (downsample_size[0] // 2, downsample_size[1] // 2)
        M_rot = cv2.getRotationMatrix2D(center_small, angle, 1.0)
        rotated = cv2.warpAffine(gray_suspect_small, M_rot, downsample_size,
                                 borderMode=cv2.BORDER_REFLECT)
        result = cv2.matchTemplate(gray_orig_small, rotated, cv2.TM_CCOEFF_NORMED)
        _, max_val, _, _ = cv2.minMaxLoc(result)
        if max_val > best_score:
            best_score = max_val
            best_angle = angle

    # Stage 2: Medium search with 0.5° steps around best angle
    medium_angles = np.arange(best_angle - 3.0, best_angle + 3.0 + 0.5, 0.5)
    for angle in medium_angles:
        center_small = (downsample_size[0] // 2, downsample_size[1] // 2)
        M_rot = cv2.getRotationMatrix2D(center_small, angle, 1.0)
        rotated = cv2.warpAffine(gray_suspect_small, M_rot, downsample_size,
                                 borderMode=cv2.BORDER_REFLECT)
        result = cv2.matchTemplate(gray_orig_small, rotated, cv2.TM_CCOEFF_NORMED)
        _, max_val, _, _ = cv2.minMaxLoc(result)
        if max_val > best_score:
            best_score = max_val
            best_angle = angle

    # Stage 3: Fine search with 0.1° steps around best angle
    fine_angles = np.arange(best_angle - 0.5, best_angle + 0.5 + 0.1, 0.1)
    for angle in fine_angles:
        center_small = (downsample_size[0] // 2, downsample_size[1] // 2)
        M_rot = cv2.getRotationMatrix2D(center_small, angle, 1.0)
        rotated = cv2.warpAffine(gray_suspect_small, M_rot, downsample_size,
                                 borderMode=cv2.BORDER_REFLECT)
        result = cv2.matchTemplate(gray_orig_small, rotated, cv2.TM_CCOEFF_NORMED)
        _, max_val, _, _ = cv2.minMaxLoc(result)
        if max_val > best_score:
            best_score = max_val
            best_angle = angle

    # Step 3: Apply best rotation to full-resolution image
    center_orig = (w_orig // 2, h_orig // 2)
    M_rot = cv2.getRotationMatrix2D(center_orig, best_angle, 1.0)
    suspect_rotated = cv2.warpAffine(suspect_resized, M_rot, (w_orig, h_orig),
                                     borderMode=cv2.BORDER_REFLECT)

    # Step 4: Fine alignment with template matching
    best_aligned = fine_align(gray_orig,
                               cv2.cvtColor(suspect_rotated.astype(np.uint8), cv2.COLOR_BGR2GRAY).astype(np.float64),
                               suspect_rotated)

    transform_params = {
        'scale_x': scale_x,
        'scale_y': scale_y,
        'shift_x': 0.0,
        'shift_y': 0.0,
        'rotation': best_angle,
        'alignment_score': best_score,
    }

    return best_aligned, transform_params


def compute_alignment_score(img1: np.ndarray, img2: np.ndarray) -> float:
    """Compute normalized cross-correlation between two images."""
    # Normalize
    img1_norm = (img1 - np.mean(img1)) / (np.std(img1) + 1e-10)
    img2_norm = (img2 - np.mean(img2)) / (np.std(img2) + 1e-10)

    # Compute correlation
    correlation = np.mean(img1_norm * img2_norm)
    return correlation


def fine_align(original_gray: np.ndarray, suspect_gray: np.ndarray,
               suspect_color: np.ndarray, search_range: int = 20) -> np.ndarray:
    """Fine alignment using template matching.

    Args:
        original_gray: Original grayscale image
        suspect_gray: Suspect grayscale image
        suspect_color: Suspect color image
        search_range: Search range in pixels

    Returns:
        Aligned color image
    """
    h, w = original_gray.shape[:2]

    # Use center region of original as template
    margin = search_range * 2
    template = original_gray[margin:h - margin, margin:w - margin]

    # Template matching
    result = cv2.matchTemplate(suspect_gray.astype(np.float32),
                                template.astype(np.float32),
                                cv2.TM_CCOEFF_NORMED)

    # Find best match
    _, max_val, _, max_loc = cv2.minMaxLoc(result)

    # Compute offset
    offset_x = max_loc[0] - margin
    offset_y = max_loc[1] - margin

    # Apply offset
    M = np.float64([[1, 0, -offset_x], [0, 1, -offset_y]])
    aligned = cv2.warpAffine(suspect_color, M, (w, h))

    return aligned


def normalize_histogram(original: np.ndarray, suspect: np.ndarray) -> np.ndarray:
    """Normalize suspect histogram to match original."""
    result = suspect.copy()

    for channel in range(3):
        orig_ch = original[:, :, channel].astype(np.float64)
        suspect_ch = suspect[:, :, channel].astype(np.float64)

        # Compute statistics
        orig_mean = np.mean(orig_ch)
        orig_std = np.std(orig_ch)
        suspect_mean = np.mean(suspect_ch)
        suspect_std = np.std(suspect_ch)

        # Normalize
        if suspect_std > 1e-10:
            normalized = (suspect_ch - suspect_mean) * (orig_std / suspect_std) + orig_mean
        else:
            normalized = suspect_ch

        result[:, :, channel] = np.clip(normalized, 0, 255)

    return result
