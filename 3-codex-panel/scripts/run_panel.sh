#!/bin/sh
set -eu

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
PROJECT_DIR=$(CDPATH= cd -- "$SCRIPT_DIR/.." && pwd)
PYTHON_BIN="$PROJECT_DIR/.venv/bin/python"
PORT="${ESP_PORT:-/dev/cu.usbmodem59090680081}"
CHIP="${ESP_CHIP:-esp32s3}"

if [ "$#" -ne 1 ]; then
    echo "用法: $0 <firmware.elf>" >&2
    exit 2
fi

if [ ! -x "$PYTHON_BIN" ]; then
    echo "缺少 $PROJECT_DIR/.venv，请先执行: make setup-esptool" >&2
    exit 1
fi

"$PYTHON_BIN" "$SCRIPT_DIR/flash_with_esptool.py" --port "$PORT" --chip "$CHIP" "$1"

echo "烧录完成。下一步请执行: make panel" >&2
