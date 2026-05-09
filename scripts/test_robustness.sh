#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
BINARY="$PROJECT_DIR/target/release/hidden_watermark"
IMAGE_DIR="$PROJECT_DIR/assets/images"
TMPDIR_BASE="/tmp/wm_robustness_$$"

KEY="testkey123"
DEFAULT_TRANSFORMS="clean blur1 blur2 jpeg75 jpeg60 jpeg40 resize75 resize50 brightness contrast noise screencam aggressive"

# Check if OpenCV is available for rotation tests
if python3 -c "import cv2" 2>/dev/null; then
    DEFAULT_TRANSFORMS="$DEFAULT_TRANSFORMS rot5 rot10 rot15"
    HAS_OPENCV=1
else
    echo "Note: OpenCV not found, skipping rotation tests (install python3-opencv)"
    HAS_OPENCV=0
fi
DEFAULT_BACKENDS="frequency-v2 jpeg-dct"

usage() {
    cat <<EOF
Usage: $(basename "$0") [OPTIONS]

Test watermark robustness across images and transforms.

Options:
    --backend <name>    Test only this backend (frequency-v2 or jpeg-dct)
    --transforms <list> Comma-separated transform names (default: all)
    --image <path>      Test only this image
    -h, --help          Show this help

Transforms: clean blur1 blur2 jpeg75 jpeg60 jpeg40 resize75 resize50
            brightness contrast noise screencam aggressive
EOF
    exit 0
}

BACKENDS="$DEFAULT_BACKENDS"
TRANSFORMS="$DEFAULT_TRANSFORMS"
SINGLE_IMAGE=""

while [[ $# -gt 0 ]]; do
    case "$1" in
        --backend) BACKENDS="$2"; shift 2 ;;
        --transforms) TRANSFORMS="${2//,/ }"; shift 2 ;;
        --image) SINGLE_IMAGE="$2"; shift 2 ;;
        -h|--help) usage ;;
        *) echo "Unknown option: $1"; exit 1 ;;
    esac
done

if [[ ! -x "$BINARY" ]]; then
    echo "Binary not found. Run: cargo build --release"
    exit 1
fi

if ! command -v magick &>/dev/null; then
    echo "ImageMagick not found. Install it first."
    exit 1
fi

mkdir -p "$TMPDIR_BASE"
trap "rm -rf '$TMPDIR_BASE'" EXIT

declare -A RESULTS
IMAGES=()
BACKEND_LIST=()
TRANSFORM_LIST=($TRANSFORMS)
TOTAL=0
PASS=0
FAIL=0
SKIP=0

generate_id() {
    head -c 16 /dev/urandom | base64 | tr -dc 'a-zA-Z0-9' | head -c 10
}

apply_transform() {
    local src="$1" transform="$2" dst="$3"
    case "$transform" in
        clean)      cp "$src" "$dst" ;;
        blur1)      magick "$src" -blur 0x1 "$dst" ;;
        blur2)      magick "$src" -blur 0x2 "$dst" ;;
        jpeg75)     magick "$src" -quality 75 "$dst" ;;
        jpeg60)     magick "$src" -quality 60 "$dst" ;;
        jpeg40)     magick "$src" -quality 40 "$dst" ;;
        resize75)   magick "$src" -resize 75% "$dst" ;;
        resize50)   magick "$src" -resize 50% "$dst" ;;
        brightness) magick "$src" -modulate 120 "$dst" ;;
        contrast)   magick "$src" -brightness-contrast 20x20 "$dst" ;;
        noise)      magick "$src" -attenuate 0.02 +noise Gaussian "$dst" ;;
        screencam)  magick "$src" -blur 0x1 -quality 60 -attenuate 0.02 +noise Gaussian -modulate 110 "$dst" ;;
        aggressive) magick "$src" -blur 0x3 -quality 30 -attenuate 0.05 +noise Gaussian -resize 75% "$dst" ;;
        rot5)       magick "$src" -rotate 5 "$dst" ;;
        rot10)      magick "$src" -rotate 10 "$dst" ;;
        rot15)      magick "$src" -rotate 15 "$dst" ;;
        *) echo "Unknown transform: $transform"; return 1 ;;
    esac
}

get_output_ext() {
    local transform="$1"
    case "$transform" in
        jpeg75|jpeg60|jpeg40|screencam|aggressive) echo ".jpg" ;;
        *) echo ".png" ;;
    esac
}

run_test() {
    local image_path="$1" backend="$2" transform="$3"
    local image_name
    image_name="$(basename "$image_path")"
    local id
    id="$(generate_id)"
    local ext
    ext="$(get_output_ext "$transform")"
    local encoded="$TMPDIR_BASE/${image_name}_${backend}_${transform}_encoded${ext}"
    local transformed="$TMPDIR_BASE/${image_name}_${backend}_${transform}_transformed${ext}"

    TOTAL=$((TOTAL + 1))

    # Encode
    local encode_output
    if ! encode_output=$("$BINARY" encode \
        --input "$image_path" \
        --output "$encoded" \
        --id "$id" \
        --key "$KEY" \
        --backend "$backend" \
        --output-format json 2>&1); then
        RESULTS["${image_name}|${backend}|${transform}"]="ENCODE_FAIL"
        FAIL=$((FAIL + 1))
        return
    fi

    # Apply transform
    if ! apply_transform "$encoded" "$transform" "$transformed" 2>/dev/null; then
        RESULTS["${image_name}|${backend}|${transform}"]="TRANSFORM_FAIL"
        FAIL=$((FAIL + 1))
        return
    fi

    # Decode
    local decode_output
    local preprocess_flag="--no-preprocess"
    # Use preprocessing for rotation transforms (to detect and correct rotation)
    case "$transform" in
        rot*) preprocess_flag="" ;;
    esac
    if decode_output=$("$BINARY" decode \
        --input "$transformed" \
        --key "$KEY" \
        --backend "$backend" \
        $preprocess_flag 2>&1); then
        local decoded_id
        decoded_id=$(echo "$decode_output" | head -1)
        local confidence
        confidence=$(echo "$decode_output" | grep -oP 'confidence=\K[0-9.]+' || echo "0.00")
        if [[ "$decoded_id" == "$id" ]]; then
            RESULTS["${image_name}|${backend}|${transform}"]="PASS|${confidence}"
            PASS=$((PASS + 1))
        else
            RESULTS["${image_name}|${backend}|${transform}"]="MISMATCH|${confidence}|${decoded_id}"
            FAIL=$((FAIL + 1))
        fi
    else
        RESULTS["${image_name}|${backend}|${transform}"]="FAIL"
        FAIL=$((FAIL + 1))
    fi
}

# Collect images
if [[ -n "$SINGLE_IMAGE" ]]; then
    IMAGES=("$SINGLE_IMAGE")
else
    for f in "$IMAGE_DIR"/*; do
        [[ -f "$f" ]] && IMAGES+=("$f")
    done
fi

for backend in $BACKENDS; do
    BACKEND_LIST+=("$backend")
done

echo "=========================================="
echo "  Watermark Robustness Test Suite"
echo "=========================================="
echo ""
echo "Images:    ${#IMAGES[@]}"
echo "Backends:  ${BACKEND_LIST[*]}"
echo "Transforms: ${TRANSFORM_LIST[*]}"
echo ""

# Run tests
for image in "${IMAGES[@]}"; do
    image_name="$(basename "$image")"
    for backend in "${BACKEND_LIST[@]}"; do
        for transform in "${TRANSFORM_LIST[@]}"; do
            printf "  %-25s %-14s %-12s ... " "$image_name" "$backend" "$transform"
            run_test "$image" "$backend" "$transform"
            result="${RESULTS[${image_name}|${backend}|${transform}]}"
            status="${result%%|*}"
            case "$status" in
                PASS) echo "✓ (${result#*|})" ;;
                FAIL) echo "✗" ;;
                MISMATCH) echo "✗ (mismatch)" ;;
                ENCODE_FAIL) echo "✗ (encode failed)" ;;
                TRANSFORM_FAIL) echo "✗ (transform failed)" ;;
            esac
        done
    done
done

echo ""
echo "=========================================="
echo "  Summary"
echo "=========================================="
echo ""
echo "Total: $TOTAL  Pass: $PASS  Fail: $FAIL"
echo ""

# Detailed table
printf "%-25s %-14s %-12s %s\n" "Image" "Backend" "Transform" "Result"
printf "%-25s %-14s %-12s %s\n" "-----" "-------" "---------" "------"
for key in $(printf '%s\n' "${!RESULTS[@]}" | sort); do
    IFS='|' read -r image_name backend transform <<< "$key"
    value="${RESULTS[$key]}"
    status="${value%%|*}"
    confidence="${value#*|}"
    confidence="${confidence%%|*}"
    case "$status" in
        PASS) printf "%-25s %-14s %-12s ✓ %s\n" "$image_name" "$backend" "$transform" "$confidence" ;;
        *) printf "%-25s %-14s %-12s ✗\n" "$image_name" "$backend" "$transform" ;;
    esac
done

echo ""
if [[ $FAIL -eq 0 ]]; then
    echo "All tests passed!"
    exit 0
else
    echo "$FAIL test(s) failed."
    exit 1
fi
