# 1-hello-word（Rust 版）

这是 `openCH 赤菟(CH32V307VCT6)` 的 Rust 第一课最小闭环工程：
- `LED1(PE11)` 每 300ms 翻转（低电平点亮）
- 仅保留最小 GPIO + Delay 逻辑，先保证“能跑起来”

## 学习资源（按优先级）

1. 本地 HAL 参考：`.ref/ch32-hal`
- `examples/ch32v307/src/bin/blinky.rs`

2. 板级资源（引脚/硬件）：`.ref/opench-ch32v307`
- `README.md` 的引脚分配章节（LED1=PE11）

3. 官方教学代码：`.ref/open-ch-chitu-tutorial-code`
- 对照 C 教程思路，不再复用 C 构建链。

## 先决条件

```bash
rustup toolchain install nightly
cargo install wlink --locked
```

如果网络慢，可先执行：

```bash
source ~/proxy.sh
```

## 命令

在仓库根目录执行：

```bash
cd 1-hello-word
source ~/.zshrc
make check
make build
make bin
```

下载运行（默认 `wlink`）：

```bash
make flash
```

备选下载（OpenOCD）：

```bash
make flash-openocd
```

USB-ISP 下载（推荐用于当前联调）：

```bash
make probe-isp
make flash-isp
```

## VSCode 任务

- `hello: check`
- `hello: build`
- `hello: flash(wlink)`
- `hello: flash(openocd)`

## 预期现象

1. LED1 快速稳定闪烁（约 3.3Hz）。

## Rust 学习进度（第 1 课）

- [x] 认识 `no_std/no_main` 入口（`#[hal::entry]`）
- [x] 使用 HAL 初始化 GPIO 输出
- [x] 使用阻塞式 Delay 实现闪烁节拍
- [x] 完成构建链与下载链
- [ ] 下一课：加入 `USART2(PA2/PA3)` 日志输出
- [ ] 下下一课：按键中断（`Wake_Up/PA0`）驱动 LED 状态切换
- [ ] 后续：迁移到 Embassy 异步任务模型（定时 + 串口并发）

## 排障提示

### 1) `wlink` 不存在

```bash
cargo install wlink --locked
```

### 2) 下载成功但程序不跑 / 上电即异常

优先检查 CH32V3 的 `ROM/RAM split` 配置是否与默认链接脚本匹配（`ch32-hal` README 明确提示）。

### 3) `make probe-isp` 显示 `Found 0 USB devices`

- 检查是否已连接 `USB-OTG` 口（不是 `WCH-Link` 口）
- 检查是否切到 BOOT/下载模式
- 切换后按一次复位键再重试
