#!/bin/bash
# zip_source.sh - Create a clean source archive for sharing
# Run this from your project root directory
# Excludes: target/, .git/, *.bin, debug builds, IDE cruft

PROJ_NAME="intel8080_emu"
OUTPUT_FILE="./tmp/${PROJ_NAME}_src_$(date +%Y%m%d_%H%M%S).zip"

# Verify we're in a reasonable location (has Cargo.toml or src/)
if [[ ! -f "Cargo.toml" && ! -d "src" ]]; then
    echo "ERROR: Run this script from your project root directory"
    echo "       (the directory containing Cargo.toml)"
    exit 1
fi

echo "Creating source archive from: $(pwd)"
echo "Output: $OUTPUT_FILE"
echo ""

zip -r "$OUTPUT_FILE" . \
    -x "target/*" \
    -x ".git/*" \
    -x "*.bin" \
    -x "*.o" \
    -x "*.lst" \
    -x "*.zip" \
    -x ".vscode/*" \
    -x ".idea/*" \
    -x "*.log" \
    -x "Cargo.lock" \
    -x "*.DS_Store" \
    -x "*/.DS_Store" \
    -x "*.swp" \
    -x "*~"

echo ""
echo "Done. Size:"
ls -lh "$OUTPUT_FILE"
echo ""
echo "Contents:"
unzip -l "$OUTPUT_FILE"
