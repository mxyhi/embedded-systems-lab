TARGET := xtensa-esp32s3-none-elf
FEATURES := firmware
PORT ?= /dev/cu.usbmodem59090680081
PYTHON ?= python3
ESP_EXPORT ?= $(HOME)/export-esp.sh
STATE_DIR := $(CURDIR)/runtime/codex-state
WIFI_IFACE ?= en0
PANEL_SERVER_BIND ?= 0.0.0.0
PANEL_SERVER_PORT ?= 31337
PANEL_SERVER_IP ?= $(shell ipconfig getifaddr $(WIFI_IFACE) 2>/dev/null)
WIFI_SSID ?= Xiaomi_2E16
WIFI_PASSWORD ?= fangtang1234

.PHONY: test build flash panel flash-run setup-esptool setup-host install-hooks uninstall-hooks install-hooks-repo uninstall-hooks-repo clean-state

test:
	cargo test
	node --test tests/test_codex_panel_hook.mjs
	$(PYTHON) -m unittest tests.test_install_hooks

build:
	set -eu; \
	if [ -z "$(PANEL_SERVER_IP)" ]; then echo "未检测到 $(WIFI_IFACE) 的 IPv4 地址"; exit 1; fi; \
	if [ -f "$(ESP_EXPORT)" ]; then . "$(ESP_EXPORT)"; fi; \
	export CODEX_PANEL_WIFI_SSID="$(WIFI_SSID)"; \
	export CODEX_PANEL_WIFI_PASSWORD="$(WIFI_PASSWORD)"; \
	export CODEX_PANEL_SERVER_IP="$(PANEL_SERVER_IP)"; \
	export CODEX_PANEL_SERVER_PORT="$(PANEL_SERVER_PORT)"; \
	cargo +esp build -Zbuild-std=core,alloc --features $(FEATURES) --target $(TARGET) --release --bin lesson-3-codex-panel

setup-esptool:
	test -x .venv/bin/python || $(PYTHON) -m venv .venv
	.venv/bin/pip show esptool >/dev/null 2>&1 || .venv/bin/pip install esptool

setup-host:
	@true

flash: setup-esptool
	set -eu; \
	if [ -z "$(PANEL_SERVER_IP)" ]; then echo "未检测到 $(WIFI_IFACE) 的 IPv4 地址"; exit 1; fi; \
	if [ -f "$(ESP_EXPORT)" ]; then . "$(ESP_EXPORT)"; fi; \
	export CODEX_PANEL_WIFI_SSID="$(WIFI_SSID)"; \
	export CODEX_PANEL_WIFI_PASSWORD="$(WIFI_PASSWORD)"; \
	export CODEX_PANEL_SERVER_IP="$(PANEL_SERVER_IP)"; \
	export CODEX_PANEL_SERVER_PORT="$(PANEL_SERVER_PORT)"; \
	ESP_PORT=$(PORT) cargo +esp run -Zbuild-std=core,alloc --features $(FEATURES) --target $(TARGET) --release --bin lesson-3-codex-panel

panel: setup-host
	mkdir -p "$(STATE_DIR)"
	cargo run --bin panel-bridge -- --bind "$(PANEL_SERVER_BIND)" --port "$(PANEL_SERVER_PORT)" --state-dir "$(STATE_DIR)"

flash-run: flash panel

install-hooks:
	$(PYTHON) scripts/install_hooks.py --project-root "$(CURDIR)" --scope user

uninstall-hooks:
	$(PYTHON) scripts/install_hooks.py --project-root "$(CURDIR)" --scope user --uninstall

install-hooks-repo:
	$(PYTHON) scripts/install_hooks.py --project-root "$(CURDIR)" --scope repo

uninstall-hooks-repo:
	$(PYTHON) scripts/install_hooks.py --project-root "$(CURDIR)" --scope repo --uninstall

clean-state:
	rm -rf "$(STATE_DIR)"
