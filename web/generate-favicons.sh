#!/bin/bash

# Script to generate all favicon sizes from app-logo.svg
# Requires ImageMagick to be installed

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
STATIC_DIR="$SCRIPT_DIR/static"
SVG_FILE="$STATIC_DIR/app-logo.svg"

echo "Generating favicons from $SVG_FILE..."

# Generate various PNG sizes
echo "Generating favicon-16x16.png..."
convert -background none -resize 16x16 "$SVG_FILE" "$STATIC_DIR/favicon-16x16.png"

echo "Generating favicon-32x32.png..."
convert -background none -resize 32x32 "$SVG_FILE" "$STATIC_DIR/favicon-32x32.png"

echo "Generating apple-touch-icon.png (180x180)..."
convert -background none -resize 180x180 "$SVG_FILE" "$STATIC_DIR/apple-touch-icon.png"

echo "Generating android-chrome-192x192.png..."
convert -background none -resize 192x192 "$SVG_FILE" "$STATIC_DIR/android-chrome-192x192.png"

echo "Generating android-chrome-512x512.png..."
convert -background none -resize 512x512 "$SVG_FILE" "$STATIC_DIR/android-chrome-512x512.png"

# Generate multi-resolution favicon.ico (contains 16x16 and 32x32)
echo "Generating favicon.ico (multi-resolution)..."
convert -background none \
  \( "$SVG_FILE" -resize 16x16 \) \
  \( "$SVG_FILE" -resize 32x32 \) \
  \( "$SVG_FILE" -resize 48x48 \) \
  "$STATIC_DIR/favicon.ico"

echo "✓ All favicons generated successfully!"
echo ""
echo "Generated files:"
echo "  - favicon.ico (multi-resolution: 16x16, 32x32, 48x48)"
echo "  - favicon-16x16.png"
echo "  - favicon-32x32.png"
echo "  - apple-touch-icon.png (180x180)"
echo "  - android-chrome-192x192.png"
echo "  - android-chrome-512x512.png"
