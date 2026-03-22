TARGET := xtensa-esp32s3-none-elf
FEATURES := firmware
PORT ?= /dev/cu.usbmodem59090680081
PYTHON ?= python3
ESP_EXPORT ?= $(HOME)/export-esp.sh

.PHONY: test build flash setup-esptool monitor

test:
	cargo test

build:
	set -eu; \
	if [ -f "$(ESP_EXPORT)" ]; then . "$(ESP_EXPORT)"; fi; \
	cargo +esp build -Zbuild-std=core,alloc --features $(FEATURES) --target $(TARGET) --release

setup-esptool:
	test -x .venv/bin/python || $(PYTHON) -m venv .venv
	.venv/bin/pip show esptool >/dev/null 2>&1 || .venv/bin/pip install esptool

flash: setup-esptool
	set -eu; \
	if [ -f "$(ESP_EXPORT)" ]; then . "$(ESP_EXPORT)"; fi; \
	ESP_PORT=$(PORT) cargo +esp run -Zbuild-std=core,alloc --features $(FEATURES) --target $(TARGET) --release

monitor: setup-esptool
	.venv/bin/python -m serial.tools.miniterm $(PORT) 115200
