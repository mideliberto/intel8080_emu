#!/bin/bash
# zip_source.sh - Create a clean source archive for sharing
# Run this from your project root directory

PROJ_NAME="intel8080_emu"
OUTPUT_FILE="./tmp/${PROJ_NAME}_src_$(date +%Y%m%d_%H%M%S).zip"

# Verify we're in a reasonable location
if [[ ! -f "Cargo.toml" && ! -d "src" ]]; then
    echo "ERROR: Run this script from your project root directory"
    exit 1
fi

# Ensure tmp exists
mkdir -p ./tmp

echo "Creating source archive from: $(pwd)"
echo "Output: $OUTPUT_FILE"
echo ""

zip -r "$OUTPUT_FILE" . \
    -x "target/*" \
    -x ".git/*" \
    -x "tmp/*" \
    -x "storage/*" \
    -x "*.bin" \
    -x "*.o" \
    -x "*.p" \
    -x "*.lst" \
    -x "*.zip" \
    -x ".vscode/*" \
    -x ".idea/*" \
    -x "*.log" \
    -x "Cargo.lock" \
    -x ".DS_Store" \
    -x "*/.DS_Store" \
    -x "*.swp" \
    -x "*~"

echo ""
echo "Done. Size:"
ls -lh "$OUTPUT_FILE"
echo ""
echo "Contents:"
unzip -l "$OUTPUT_FILE"