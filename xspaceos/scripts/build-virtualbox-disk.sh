#!/usr/bin/env sh

set -eu

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
PROJECT_DIR=$(CDPATH= cd -- "$SCRIPT_DIR/.." && pwd)
TARGET_DIR="$PROJECT_DIR/target/x86_64-xspaceos/debug"
RAW_IMAGE="$TARGET_DIR/bootimage-xspaceos.bin"
VDI_IMAGE="$PROJECT_DIR/xspaceos.vdi"

if ! command -v cargo >/dev/null 2>&1; then
  echo "error: cargo is not installed" >&2
  exit 1
fi

if ! command -v qemu-img >/dev/null 2>&1; then
  echo "error: qemu-img is not installed" >&2
  echo "install QEMU tools first, then rerun this script" >&2
  exit 1
fi

cd "$PROJECT_DIR"

echo "building bootable raw disk image..."
cargo bootimage

if [ ! -f "$RAW_IMAGE" ]; then
  echo "error: expected raw image was not created: $RAW_IMAGE" >&2
  exit 1
fi

echo "converting raw disk image to VirtualBox VDI..."
qemu-img convert -f raw -O vdi "$RAW_IMAGE" "$VDI_IMAGE"

echo
echo "created:"
echo "  raw: $RAW_IMAGE"
echo "  vdi: $VDI_IMAGE"
echo
echo "VirtualBox setup:"
echo "  1. Create a new VM using 'Other/Unknown (64-bit)' or similar."
echo "  2. Disable EFI and use legacy BIOS boot."
echo "  3. Attach $VDI_IMAGE as the VM disk."
echo "  4. Boot the VM."
