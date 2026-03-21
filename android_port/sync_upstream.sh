#!/bin/bash
# sync_upstream.sh — Synchronize Impulcifer source code from upstream desktop repo
#
# This script copies the Python processing modules from Impulcifer-pip313
# into the Android project's Chaquopy Python directory.
# It does NOT touch scipy_shim or android_bootstrap.py.
#
# Usage:
#   ./sync_upstream.sh /path/to/Impulcifer-pip313
#
# If no path is given, defaults to ../Impulcifer-pip313 (sibling directory)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
UPSTREAM="${1:-$(dirname "$SCRIPT_DIR")/Impulcifer-pip313}"
TARGET="$SCRIPT_DIR/app/src/main/python"

if [ ! -d "$UPSTREAM" ]; then
    echo "ERROR: Upstream directory not found: $UPSTREAM"
    echo "Usage: $0 /path/to/Impulcifer-pip313"
    exit 1
fi

echo "=== Impulcifer Android — Upstream Sync ==="
echo "Source:  $UPSTREAM"
echo "Target:  $TARGET"
echo ""

# Create target directory if needed
mkdir -p "$TARGET"

# Sync core processing modules (the ones that run unmodified)
echo "Syncing impulcifer.py..."
cp "$UPSTREAM/impulcifer.py" "$TARGET/"

echo "Syncing core/ modules..."
mkdir -p "$TARGET/core"
for f in hrir.py impulse_response.py impulse_response_estimator.py \
         room_correction.py microphone_deviation_correction.py \
         virtual_bass.py utils.py channel_generation.py constants.py \
         parallel_processing.py parallel_utils.py parallel_workers.py \
         __init__.py; do
    if [ -f "$UPSTREAM/core/$f" ]; then
        cp "$UPSTREAM/core/$f" "$TARGET/core/"
    fi
done

echo "Syncing i18n/ localization..."
mkdir -p "$TARGET/i18n"
cp -r "$UPSTREAM/i18n/"* "$TARGET/i18n/" 2>/dev/null || true

echo "Syncing infra/ utilities..."
mkdir -p "$TARGET/infra"
for f in logger.py resource_helper.py __init__.py; do
    if [ -f "$UPSTREAM/infra/$f" ]; then
        cp "$UPSTREAM/infra/$f" "$TARGET/infra/"
    fi
done

echo "Syncing data/ directory (test signals, targets)..."
mkdir -p "$TARGET/data"
# Copy essential data files (not the full demo set)
for f in "$UPSTREAM/data/"*.csv "$UPSTREAM/data/"*.pkl "$UPSTREAM/data/"*.wav; do
    if [ -f "$f" ]; then
        cp "$f" "$TARGET/data/"
    fi
done

# Do NOT sync:
# - gui/ (replaced by Kotlin Jetpack Compose)
# - research/ (desktop only)
# - tests/ (separate test suite for Android)
# - build_scripts/ (desktop build tools)
# - updater/ (desktop auto-update)
# - scipy_shim/ (Android-specific, maintained separately)
# - android_bootstrap.py (Android-specific)

echo ""
echo "=== Sync complete ==="
echo "Files NOT synced (maintained separately):"
echo "  - scipy_shim/        (Android scipy replacement)"
echo "  - android_bootstrap.py (Android initialization)"
echo "  - gui/               (replaced by Kotlin UI)"
echo ""
echo "Next: Verify scipy_shim tests pass with updated code"
