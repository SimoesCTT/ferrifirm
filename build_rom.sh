#!/bin/bash
set -euo pipefail

cd "$(dirname "$0")"

echo "Building FerriFirm..."

cargo build --release --target x86_64-unknown-none

ELF="target/x86_64-unknown-none/release/ferrifirm"

echo "Creating ROM image..."

objcopy \
    -O binary \
    --gap-fill=0xFF \
    --pad-to=0x100000000 \
    "$ELF" \
    ferrifirm.bin

echo "Checking ROM size..."

test "$(stat -c%s ferrifirm.bin)" -eq 1048576

cp ferrifirm.bin ferrifirm.rom

echo
echo "FerriFirm ROM generated successfully."
echo "ELF: $ELF"
echo "ROM: ferrifirm.bin"
echo "ROM: ferrifirm.rom"

sha256sum ferrifirm.bin ferrifirm.rom
