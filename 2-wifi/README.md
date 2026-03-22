# 第 2 课：扫描 WiFi 并在屏幕显示 SSID

## 本课目标

这一课只做一个最小闭环：

1. 启动 `ESP32-S3` 的板载 WiFi
2. 扫描周围热点
3. 把结果整理成最多 `5` 条 SSID
4. 在板载 `160x80` 屏幕上显示出来

本课**不做**下面这些事：

- 连接路由器
- DHCP / HTTP / SNTP
- 自动刷新扫描结果
- 中文点阵字体
- RSSI / 加密方式上屏

## 当前硬件事实

- 开发板：`ESP32S3M`
- 屏幕：板载 `0.96"` IPS LCD
- 驱动芯片：`ST7735S`
- 逻辑分辨率：`160x80`
- 当前串口：`/dev/cu.usbmodem59090680081`

本课沿用第 1 课已经验证过的 LCD 引脚：

| 信号 | GPIO |
|------|------|
| `MOSI` | `11` |
| `SCLK` | `12` |
| `MISO` | `13` |
| `CS` | `39` |
| `DC` | `40` |
| `RST` | `38` |
| `BL` | `41` |

## 为什么第二课选 `esp-wifi`

这一课我实际比较了 3 条路：

1. `esp-wifi`
2. `esp-radio`
3. `esp-idf-svc`

最终选 `esp-wifi`，原因很直接：

- 它还能保持纯 Rust / `no_std`
- 不需要像 `esp-radio` 一样，先把抢占式调度器带进来
- 又比 `esp-idf-svc` 更接近第 1 课的学习路径

这里有一个必须记住的现实约束：

- `esp-wifi 0.15.1` 依赖的是 `esp-hal 1.0.0-rc.0`
- 第 1 课用的是 `esp-hal 1.0.0`
- 所以第二课必须做成独立目录 `2-wifi/`，不要把两课强行揉在一起

## 工程结构

```text
2-wifi/
├── .cargo/config.toml
├── .vscode/tasks.json
├── Cargo.toml
├── Makefile
├── README.md
├── build.rs
├── scripts/
│   ├── flash_with_esptool.py
│   └── run_firmware.sh
└── src/
    ├── lib.rs
    └── main.rs
```

文件职责如下：

- `src/lib.rs`
  - 放主机侧可测试逻辑：ASCII 安全化、SSID 截断、列表格式化
- `src/main.rs`
  - 放 LCD 初始化、WiFi 初始化、扫描与上屏流程
- `scripts/flash_with_esptool.py`
  - 继续沿用第 1 课的稳定烧录链路
- `scripts/run_firmware.sh`
  - 烧录后直接打开串口监视器

## 学习过程

### 第 1 步：先把“屏幕上显示什么”锁死

屏幕只有 `160x80`，这一课如果一上来就加 RSSI、信道、加密方式，画面会立刻变挤。

所以本课先固定成这套版式：

- 标题：`WiFi Scan`
- 正文：最多 `5` 行 SSID
- 标题字体：`FONT_6X10`
- 列表字体：`FONT_5X8`

这样做的好处是：

- 复杂度够低
- 结果够直观
- 后面真要加 RSSI，也知道该往哪一行扩

### 第 2 步：先做主机侧 TDD，再碰硬件

这节课先在 `src/lib.rs` 写了 7 个测试，锁死下面这些行为：

- ASCII SSID 必须保持不变
- 非 ASCII 字符先替换成 `?`
- 长 SSID 要截断并补 `...`
- 空 SSID 要显示成 `<hidden>`
- 屏幕列表最多只能显示 `5` 行

这样可以先把“字符串怎么显示”这件事从硬件问题里拆出来。

### 第 3 步：为什么这一课必须引入 `alloc`

第 1 课只显示固定字符串，所以只需要 `core`。

但这一课不一样：

- `esp-wifi::scan_n(max)` 返回 `Vec<AccessPointInfo>`
- `AccessPointInfo.ssid` 里本身就是动态字符串

所以这一课必须：

- 启用 `esp-alloc`
- 在 `main.rs` 里初始化 heap
- 把 `Makefile` 的 `-Zbuild-std=core` 改成 `-Zbuild-std=core,alloc`

这不是过度设计，而是扫描 API 的直接要求。

### 第 4 步：先把背光极性校准对

这一课中途真实遇到过一次经典问题：

- 串口日志完整
- WiFi 扫描成功
- 但屏幕一直黑

最后定位出来，根因不是 WiFi，也不是文字绘制，而是背光极性判断反了。

最终以上板结果为准：

- 这块板子的背光应按高有效处理
- 也就是 `BACKLIGHT_ACTIVE_HIGH = true`
- `GPIO41` 拉高后，屏幕才真正点亮

### 第 5 步：再真正扫描 WiFi

WiFi 扫描不是瞬时完成的。

所以当前正式版启动顺序是：

1. 先初始化 LCD
2. 立刻打开背光
3. 屏幕显示 `Scanning...`
3. 再初始化 WiFi
4. 扫描热点
5. 排序并上屏

这样如果板子停在扫描阶段，你至少能从屏幕和串口知道它已经跑到哪里，而不是只看到黑屏。

### 第 6 步：扫描结果先按 RSSI 排序

屏幕只能放 `5` 条，所以当前实现不是“扫到什么就原样显示什么”，而是：

1. 先扫描最多 `12` 条 AP
2. 按 `signal_strength` 从强到弱排序
3. 取前 `5` 条显示

这能让屏幕更稳定地显示“离你最近、最有用”的几个热点。

## 关键代码解读

### `src/lib.rs`

- `ascii_safe()`
  - 把当前 ASCII 字体无法显示的字符替换成 `?`
- `truncate_with_dots()`
  - 把超长 SSID 截成单行并补 `...`
- `display_ssid()`
  - 统一处理空 SSID、ASCII 安全化与长度裁剪
- `format_ssid_lines()`
  - 生成最终上屏文本，例如 `1. MyWiFi`

### `src/main.rs`

- LCD 初始化参数仍沿用第 1 课：
  - `80x160`
  - `offset(26, 1)`
  - `Rotation::Deg270`
  - `Bgr + Inverted`
- WiFi 初始化最小步骤如下：
  - `esp_alloc::heap_allocator!`
  - `TimerGroup::new(peripherals.TIMG0)`
  - `Rng::new(peripherals.RNG)`
  - `esp_wifi::init(...)`
  - `wifi::new(...)`
  - `set_mode(WifiMode::Sta)`
  - `start()`
  - `scan_n(...)`

## 运行前准备

如果你的机器还没装 ESP Rust 工具链，先执行：

```bash
cargo install espup
espup install
```

重新打开 shell 后，再安装镜像生成工具：

```bash
cargo install espflash
```

然后在 lesson 目录里准备一个本地 Python 虚拟环境，用来装 `esptool`：

```bash
cd 2-wifi
make setup-esptool
```

## 常用命令

主机侧测试：

```bash
make test
```

构建固件：

```bash
make build
```

烧录并打开串口日志：

```bash
make flash
```

如果串口设备名不同，直接覆盖 `PORT`：

```bash
make flash PORT=/dev/cu.usbmodemXXXX
```

## 预期现象

正常情况下，烧录后应看到下面这些现象：

1. 屏幕先显示 `Scanning...`
2. 扫描完成后，标题仍是 `WiFi Scan`
3. 下方出现最多 `5` 条 SSID
4. 串口至少会打印：

```text
lesson 2: boot
lesson 2: scanning wifi...
lesson 2: found N access points
lesson 2: wifi list rendered
```

中间还会打印每一条 SSID 的简要信息，例如：

```text
#1: ssid='...', rssi=..., channel=...
```

## 本轮实际验证结果

本轮已经完成下面这些验证：

- `cargo test` 通过，`7` 个主机侧测试全部为绿灯
- `make build` 通过
- `sh -n scripts/run_firmware.sh` 通过
- `python3 -m py_compile scripts/flash_with_esptool.py` 通过
- `make flash PORT=/dev/cu.usbmodem59090680081` 已实际跑通
- 串口已实际读到：
  - `lesson 2: boot`
  - `lesson 2: scanning wifi...`
  - `lesson 2: found 9 access points`
  - `lesson 2: wifi list rendered`
- 用户已肉眼确认：屏幕成功显示

说明这条链路已经至少确认：

- 应用成功启动
- 背光极性已经修正正确
- WiFi 扫描成功
- 列表格式化成功
- 绘制代码已经跑到最终上屏分支
- 屏幕肉眼显示已确认正常

## 排障建议

### 目标机构建时报 `can't find crate for alloc`

说明你把第 2 课的构建参数改坏了。

这一课必须保留：

```text
-Zbuild-std=core,alloc
```

不要改回第 1 课的 `core`。

### 串口能看到 `boot`，但扫描失败

优先检查：

- WiFi 是否已被设置成 `WifiMode::Sta`
- `start()` 是否成功执行
- CPU 频率是否仍保持在 `CpuClock::max()`

### 串口有日志，但屏幕没变化

先优先检查背光极性 `BACKLIGHT_ACTIVE_HIGH`。

这一块板子已经实测确认：

- `GPIO41` 拉高才会亮背光
- 所以代码里应保留 `BACKLIGHT_ACTIVE_HIGH = true`

如果仍有问题，再回头检查第 1 课里已经提到过的显示参数：

- `DISPLAY_OFFSET_X`
- `DISPLAY_OFFSET_Y`
- `Rotation::Deg270`

### `make flash` 没有 `.venv`

先执行：

```bash
make setup-esptool
```

## 这一课你已经学到什么

做完这一课后，你已经多掌握了 4 件事：

- `ESP32-S3` 的 WiFi 扫描最小初始化路径
- 为什么无线扫描天然会把 `alloc` 引进来
- 如何把动态扫描结果压缩成固定小屏上的稳定列表
- 如何把“显示逻辑”和“硬件调试”拆开处理

## 下一课建议

下一课可以继续做下面两条之一：

1. 连接指定 WiFi，并把连接状态显示到屏幕上
2. 保持扫描能力不变，再增加 RSSI 或信道显示
