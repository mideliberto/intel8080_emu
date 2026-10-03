#!/bin/bash
# fetch_exercisers.sh - Download the four standard 8080 exercisers for tests/exerciser.rs.
# Pinned by SHA-256; a mismatch deletes the file and fails.
# Run from the project root, then:
#   cargo test --release --test exerciser -- --ignored --nocapture
set -euo pipefail

DIR="tests/data/exercisers"
BASE="https://raw.githubusercontent.com/superzazu/8080/master/cpu_tests"
mkdir -p "$DIR"

status=0
while read -r name sum; do
    curl -fsSL "$BASE/$name" -o "$DIR/$name"
    if [[ "$(shasum -a 256 "$DIR/$name" | cut -d' ' -f1)" == "$sum" ]]; then
        echo "ok        $name"
    else
        echo "MISMATCH  $name"
        rm -f "$DIR/$name"
        status=1
    fi
done <<'LIST'
TST8080.COM 9561c6fb6c99efe3de00eb77e4044fd102151058b39ac2d7bce10483838a08e7
8080PRE.COM 18eb3c79cba42c0718f160be6a1853cb64cdce7aa47d65780189a57bdd98c4e0
CPUTEST.COM e61a9a75348c774486c2207080ea4effbf6c2367fdace31b0731081a4144030b
8080EXM.COM 6e3286e11bb1a8f47b8ee1280b4a067be813193363e3223c99b0d21912f44aeb
LIST
exit $status
