#!/usr/bin/env python3

from __future__ import annotations

import argparse
import subprocess
import tempfile
from pathlib import Path

DEFAULT_PORT = "/dev/cu.usbmodem59090680081"
PROJECT_DIR = Path(__file__).resolve().parent.parent


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="通过 espflash + esptool 为 lesson 固件生成镜像并烧录。"
    )
    parser.add_argument("elf", help="待烧录的 ELF 固件路径")
    parser.add_argument("--chip", default="esp32s3", help="目标芯片，默认 esp32s3")
    parser.add_argument("--port", default=DEFAULT_PORT, help="串口设备路径")
    return parser.parse_args()


def build_merged_image(elf: Path, output: Path, chip: str) -> None:
    subprocess.run(
        [
            "espflash",
            "save-image",
            "--chip",
            chip,
            "--merge",
            "--skip-padding",
            "--ignore-app-descriptor",
            "--flash-mode",
            "dio",
            "--flash-freq",
            "40mhz",
            "--flash-size",
            "16mb",
            str(elf),
            str(output),
        ],
        check=True,
        cwd=PROJECT_DIR,
    )


def flash_image(image: Path, port: str, chip: str) -> None:
    import esptool
    from esptool.loader import ESPLoader
    from esptool.targets.esp32s3 import ESP32S3ROM

    # 这块板当前通过 WCH USB-UART 桥下载时，默认 0x400 字节的 FLASH_DATA
    # 包会在 ROM bootloader 阶段损坏。把块大小降到 0x100 后可以稳定写入。
    ESPLoader.FLASH_WRITE_SIZE = 0x100
    ESP32S3ROM.FLASH_WRITE_SIZE = 0x100
    ESPLoader.WRITE_FLASH_ATTEMPTS = 1

    esptool.main(
        [
            "--chip",
            chip,
            "--port",
            port,
            "--baud",
            "115200",
            "--before",
            "default-reset",
            "--after",
            "hard-reset",
            "--no-stub",
            "write-flash",
            "--flash-mode",
            "dio",
            "--flash-freq",
            "40m",
            "--flash-size",
            "16MB",
            "0x0",
            str(image),
        ]
    )


def main() -> None:
    args = parse_args()
    elf = Path(args.elf).resolve()

    with tempfile.TemporaryDirectory(prefix="lesson-1-hello-word-") as temp_dir:
        image = Path(temp_dir) / "firmware.bin"
        build_merged_image(elf, image, args.chip)
        flash_image(image, args.port, args.chip)


if __name__ == "__main__":
    main()
