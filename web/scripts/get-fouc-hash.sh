#!/bin/bash
# Calculate SHA-256 hash of the minified FOUC script for CSP
# Usage: ./get-fouc-hash.sh

SCRIPT_DIR=$(dirname "$0")
JS_FILE="$SCRIPT_DIR/fouc.min.js"

if [ ! -f "$JS_FILE" ]; then
    echo "Error: $JS_FILE not found"
    exit 1
fi

HASH=$(cat "$JS_FILE" | tr -d '\n' | openssl dgst -sha256 -binary | base64)
echo "CSP Hash: sha256-$HASH"
