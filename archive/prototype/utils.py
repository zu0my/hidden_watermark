"""Utility functions for non-blind watermark prototype."""

import hashlib
import struct

import cv2
import numpy as np
from scipy.fftpack import dctn, idctn


# Block size for DCT operations
BLOCK_SIZE = 32

# Mid-frequency coefficient range (zigzag indices)
MID_FREQ_START = 8
MID_FREQ_END = 24


def generate_prng_sequence(key: str, length: int) -> np.ndarray:
    """Generate pseudo-random sequence of +1/-1 values from key using ChaCha20-like approach."""
    seed = hashlib.sha256(key.encode()).digest()
    # Use seed to generate deterministic sequence
    result = np.empty(length, dtype=np.float64)
    block_index = 0
    generated = 0

    while generated < length:
        # Create unique block for each chunk
        block_data = seed + struct.pack('<I', block_index)
        block_hash = hashlib.sha256(block_data).digest()

        for i in range(0, len(block_hash) - 1, 2):
            if generated >= length:
                break
            # Use two bytes to get more randomness
            val = block_hash[i] ^ block_hash[i + 1]
            result[generated] = 1.0 if val >= 128 else -1.0
            generated += 1

        block_index += 1

    return result


def get_zigzag_indices(block_size: int = BLOCK_SIZE) -> list[tuple[int, int]]:
    """Get zigzag scan order for a block, returning (row, col) pairs."""
    indices = []
    for sum_val in range(2 * block_size - 1):
        if sum_val % 2 == 0:
            # Go up-right
            for row in range(min(sum_val, block_size - 1), max(-1, sum_val - block_size), -1):
                col = sum_val - row
                if 0 <= col < block_size:
                    indices.append((row, col))
        else:
            # Go down-left
            for row in range(max(0, sum_val - block_size + 1), min(sum_val + 1, block_size)):
                col = sum_val - row
                if 0 <= col < block_size:
                    indices.append((row, col))
    return indices


def get_mid_freq_positions(block_size: int = BLOCK_SIZE) -> list[tuple[int, int]]:
    """Get mid-frequency coefficient positions (zigzag indices 8-24)."""
    zigzag = get_zigzag_indices(block_size)
    return zigzag[MID_FREQ_START:MID_FREQ_END]


def compute_texture_weight(block: np.ndarray) -> float:
    """Compute perceptual weight based on block texture complexity."""
    # Use variance as texture measure
    variance = np.var(block)

    # Normalize: low variance = flat region (low weight), high variance = textured (high weight)
    # Clamp to [0.5, 2.0]
    weight = 0.5 + min(variance / 1000.0, 1.5)
    return weight


def apply_dct(block: np.ndarray) -> np.ndarray:
    """Apply 2D DCT to a block."""
    return dctn(block, type=2, norm='ortho')


def apply_idct(block: np.ndarray) -> np.ndarray:
    """Apply inverse 2D DCT to a block."""
    return idctn(block, type=2, norm='ortho')


def calculate_psnr(original: np.ndarray, modified: np.ndarray) -> float:
    """Calculate Peak Signal-to-Noise Ratio between two images."""
    mse = np.mean((original.astype(np.float64) - modified.astype(np.float64)) ** 2)
    if mse == 0:
        return float('inf')
    return 10 * np.log10(255.0 ** 2 / mse)


def split_into_blocks(image: np.ndarray, block_size: int = BLOCK_SIZE) -> tuple[list[np.ndarray], int, int]:
    """Split image into non-overlapping blocks. Returns (blocks, rows, cols)."""
    h, w = image.shape[:2]
    rows = h // block_size
    cols = w // block_size

    blocks = []
    for r in range(rows):
        for c in range(cols):
            y = r * block_size
            x = c * block_size
            block = image[y:y + block_size, x:x + block_size]
            blocks.append(block)

    return blocks, rows, cols


def reconstruct_from_blocks(blocks: list[np.ndarray], rows: int, cols: int,
                            block_size: int = BLOCK_SIZE) -> np.ndarray:
    """Reconstruct image from blocks."""
    h = rows * block_size
    w = cols * block_size
    image = np.zeros((h, w), dtype=np.float64)

    idx = 0
    for r in range(rows):
        for c in range(cols):
            y = r * block_size
            x = c * block_size
            image[y:y + block_size, x:x + block_size] = blocks[idx]
            idx += 1

    return image


def load_image(path: str) -> np.ndarray:
    """Load image as float64 BGR."""
    img = cv2.imread(path)
    if img is None:
        raise FileNotFoundError(f"Cannot load image: {path}")
    return img.astype(np.float64)


def save_image(path: str, image: np.ndarray) -> None:
    """Save float64 image."""
    cv2.imwrite(path, np.clip(image, 0, 255).astype(np.uint8))
