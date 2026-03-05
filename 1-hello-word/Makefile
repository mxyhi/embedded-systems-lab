TARGET ?= riscv32imfc-unknown-none-elf
TARGET_SPEC ?= riscv32imfc-unknown-none-elf.json
PROFILE ?= release
BIN ?= opench-rust-hello-word
ELF := target/$(TARGET)/$(PROFILE)/$(BIN)
BIN_IMG := target/$(TARGET)/$(PROFILE)/$(BIN).bin

OPENOCD_MRS ?= $(HOME)/Applications/MRS_Toolchain_V230/OpenOCD/OpenOCD/bin/openocd

ifeq ($(wildcard $(OPENOCD_MRS)),)
OPENOCD ?= openocd
OPENOCD_CFG ?= tools/openocd/wch-riscv.cfg
else
OPENOCD ?= $(OPENOCD_MRS)
OPENOCD_CFG ?= $(dir $(OPENOCD_MRS))wch-riscv.cfg
endif

.PHONY: help check build run bin flash flash-openocd probe-isp flash-isp clean size

help:
	@echo "make check         # cargo check"
	@echo "make build         # cargo build --release"
	@echo "make bin           # 生成 bin 固件"
	@echo "make flash         # cargo run --release (runner=wlink)"
	@echo "make flash-openocd # openocd 方式下载 $(ELF)"
	@echo "make probe-isp     # wchisp 检查 USB-ISP 设备"
	@echo "make flash-isp     # wchisp 方式下载（需 BOOT/下载模式 + USB-OTG）"
	@echo "make size          # 查看 ELF 体积"
	@echo "make clean         # cargo clean"

check:
	cargo check --target $(TARGET_SPEC)

build:
	cargo build --profile $(PROFILE) --target $(TARGET_SPEC)

run: build
	cargo run --profile $(PROFILE) --target $(TARGET_SPEC)

bin: build
	@if command -v riscv-wch-elf-objcopy >/dev/null 2>&1; then \
		riscv-wch-elf-objcopy -O binary $(ELF) $(BIN_IMG); \
	elif command -v riscv-none-embed-objcopy >/dev/null 2>&1; then \
		riscv-none-embed-objcopy -O binary $(ELF) $(BIN_IMG); \
	else \
		echo "objcopy 工具未找到，请先执行: source ~/.zshrc"; \
		exit 1; \
	fi
	@echo "BIN: $(BIN_IMG)"

flash: run

flash-openocd: build
	$(OPENOCD) -f $(OPENOCD_CFG) -c init -c halt -c "program $(ELF) verify reset exit"

probe-isp:
	wchisp -u probe

flash-isp: build
	wchisp -u flash $(ELF)

size: build
	@if command -v riscv-wch-elf-size >/dev/null 2>&1; then \
		riscv-wch-elf-size -d $(ELF); \
	elif command -v riscv-none-embed-size >/dev/null 2>&1; then \
		riscv-none-embed-size -d $(ELF); \
	else \
		echo "size 工具未找到，请先执行: source ~/.zshrc"; \
	fi

clean:
	cargo clean
