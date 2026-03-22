# 第 1 课：让 ESP32S3M 屏幕显示 Hello World

## 本课目标

这一课只做一件事：让 `ESP32S3M` 板载屏稳定显示 `Hello World`。

为了把学习路径压到最短，本课只保留下面 4 个知识点：

1. 用 Rust 初始化 `ESP32-S3` 裸机工程。
2. 配置 `SPI2 + GPIO` 驱动板载 `ST7735S` 屏幕。
3. 用 `mipidsi` 完成 LCD 初始化。
4. 用 `embedded-graphics` 在屏幕中心绘制一行文字。

## 当前硬件事实

- 开发板：`ESP32S3M`
- 屏幕：板载 `0.96"` IPS LCD
- 分辨率：`160x80`
- 驱动芯片：`ST7735S`
- 接口：`SPI`
- 当前串口：`/dev/cu.usbmodem59090680081`

本课按下面这组引脚实现：

| 信号 | GPIO |
|------|------|
| `MOSI` | `11` |
| `SCLK` | `12` |
| `MISO` | `13` |
| `CS` | `39` |
| `DC` | `40` |
| `RST` | `38` |
| `BL` | `41` |

## 工程结构

```text
1-hello-word/
├── .cargo/config.toml
├── .vscode/tasks.json
├── Cargo.toml
├── Makefile
├── README.md
├── scripts/
│   ├── flash_with_esptool.py
│   └── run_firmware.sh
└── src/
    ├── lib.rs
    └── main.rs
```

文件职责如下：

- `src/lib.rs`
  - 放主机侧可测试的常量和居中布局逻辑。
- `src/main.rs`
  - 放 `ESP32-S3` 的 GPIO、SPI、LCD 初始化与绘制代码。
- `scripts/flash_with_esptool.py`
  - 生成合并镜像，并用小块 `FLASH_DATA` 方式稳定烧录。
- `scripts/run_firmware.sh`
  - 供 `cargo run` / `make flash` 调用，负责烧录后打开串口监看。

## 学习过程

### 第 1 步：先在电脑上锁定布局逻辑

`src/lib.rs` 先写了 3 个测试：

- 屏幕宽高必须是 `160x80`
- `Hello World` 用 `FONT_10X20` 时文本框尺寸必须正确
- 文本居中后起点必须落在屏幕内

这样做的目的很直接：先把“文字应该画到哪”这件事在主机侧锁死，避免把布局问题也拖到硬件调试里。

### 第 2 步：把板级 LCD 参数翻译成 Rust 常量

`src/main.rs` 里当前用的是这组板级参数：

- 逻辑显示尺寸：`160x80`
- 控制器 framebuffer 尺寸：`80x160`
- 控制器坐标系偏移：`(26, 1)`
- `orientation = Deg270`
- `color_order = Bgr`
- `invert_colors = Inverted`

这不是拍脑袋猜的，而是按板级 C 驱动的这套行为翻译过来的：

- 横屏模式使用 `lcd_display_dir(1)`
- 显存窗口写入时固定做 `x + 1 / y + 26`
- 初始化命令表里显式包含 `0x21`，也就是开颜色反转

这里有一个很容易踩的坑：

- `mipidsi` 的 `display_size` / `display_offset` 不是按最终横屏坐标填的
- 它要求你按控制器默认竖屏 framebuffer 坐标系来填
- 所以代码里实际写的是 `80x160 + (26, 1)`，再通过 `Deg270` 旋转成最终的 `160x80 + (1, 26)`

### 第 3 步：最后再点亮背光

当前代码会先：

1. 初始化 SPI
2. 初始化 LCD
3. 清黑屏
4. 绘制 `Hello World`
5. 最后再开背光

这样做的好处是，如果参数不对，屏幕不会先把脏画面直接暴露出来。

实际回板验证后，最终结论已经确定：

- 这块板子的背光应该按高有效处理
- 也就是 `GPIO41` 拉高时，屏幕会真正点亮

原因也已经很清楚：

- 文档正文虽然写了 `LEDK` 低电平点亮
- 但同页示例代码里的 `lcd_on()` 实际把背光脚拉到 `1`
- 最终以上板现象为准，当前代码固定保留 `BACKLIGHT_ACTIVE_HIGH = true`

### 第 4 步：把下载链路问题和显示问题拆开

这块板在我这台机器上通过 `WCH USB-UART` 桥下载时，还踩到了两个和业务逻辑无关的坑：

1. `espflash` 能连上芯片，但会在写 flash 或上传 stub 时失败。
2. 直接把 `esp-hal` 生成的 app image 写进去后，若没有 `ESP-IDF` 风格的 `app descriptor`，bootloader 会把随机数据误判成 eFuse 版本要求，导致应用不启动。

最终的处理方式是：

- 在 `src/main.rs` 顶部补 `esp_bootloader_esp_idf::esp_app_desc!()`
- 保留 `espflash save-image` 来生成合并镜像
- 真正写 flash 时改走 `esptool --no-stub`
- 并把 `FLASH_DATA` 包大小从默认 `0x400` 临时降到 `0x100`

这样之后，串口已经实际验证能稳定打印：

```text
lesson 1: boot
lesson 1: hello world rendered
```

## 关键代码解读

### `src/lib.rs`

- `display_size()`
  - 返回板载屏逻辑尺寸 `160x80`
- `hello_world_text_size()`
  - 根据 `FONT_10X20` 计算 `"Hello World"` 的像素尺寸
- `centered_top_left()`
  - 计算左上角坐标，并在内容比屏幕大时自动钳到 `0`

### `src/main.rs`

- `SPI2` 负责和 `ST7735S` 通信
- `GPIO38/39/40/41` 分别负责 `RST/CS/DC/BL`
- `mipidsi::Builder` 负责把 `ST7735S` 初始化序列发出去
- `embedded-graphics` 负责真正把文字绘到显存窗口

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
cd 1-hello-word
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

如果串口设备名和我这里不同，直接覆盖 `PORT`：

```bash
make flash PORT=/dev/cu.usbmodemXXXX
```

注意：当前 Xtensa 工具链会在构建时现场编译 `core`，所以 `Makefile` 已经内置了 `-Zbuild-std=core`，不要手动删掉。

如果你的机器上已经有 `~/export-esp.sh`，当前 `Makefile` 会在 `make build` / `make flash`
时自动先加载这份环境脚本，所以正常情况下不需要你再手动 `source` 一遍。

另外，本课还补了一层 Xtensa 专用链接配置：

- `.cargo/config.toml` 里为目标机加了 `-nostartfiles`
- `build.rs` 会给固件二进制注入 `-Tlinkall.x`

这两项都是 `esp-rs` 官方模板在 Xtensa 路线下的最小必需配置。

另外，本课当前的 `cargo run` runner 不再直接调用 `espflash flash`，而是改成：

1. 先用 `espflash save-image` 生成合并镜像
2. 再用本地 `.venv` 里的 `esptool` 做 `--no-stub` 小块烧录
3. 最后打开串口监视器

这是因为当前这块板通过 `WCH` 串口桥下载时，默认大块写入不稳定，小块写入才稳定。

## 预期现象

烧录成功后，正常现象应该是：

1. 屏幕先亮起
2. 背景为黑色
3. 中央出现白色 `Hello World`
4. 串口打印两行日志：

```text
lesson 1: boot
lesson 1: hello world rendered
```

这一版代码已经在串口侧实际验证到上面两行日志。
并且已经由本轮上板实际确认：屏幕可以正常显示 `Hello World`。

## 排障建议

### `make flash` 一上来就提示缺少 `.venv`

先执行：

```bash
make setup-esptool
```

### 背光不亮

先改 `src/main.rs` 里的 `BACKLIGHT_ACTIVE_HIGH`：

- 现在默认是 `false`
- 如果屏幕完全不亮，再试切回 `true`

### 背光亮了，但文字不在中间或画面错位

优先试这 3 个常量：

- `DISPLAY_OFFSET_X`
- `DISPLAY_OFFSET_Y`
- `Rotation::Deg270`

### 屏幕有背光，但画面花屏

先把 SPI 频率从 `20MHz` 降到 `10MHz`：

```rust
const SPI_FREQUENCY_MHZ: u32 = 10;
```

### `espflash` 报 `The bootloader returned an error`

这块板在当前 `WCH USB-UART` 链路上，直接走 `espflash flash` 不稳定。

不要再手工改回旧 runner，直接用本课自带的：

- `make setup-esptool`
- `make flash`

### 串口只看到 bootloader，不进应用

如果你自己改动了工程，又重新出现下面这种日志：

```text
Image requires efuse blk rev ...
```

通常说明应用里缺了 `ESP-IDF` 风格的 app descriptor。

本课现在已经通过 `esp_bootloader_esp_idf::esp_app_desc!()` 处理了，不要删掉它。

## 这一课你已经学到什么

做完这一课后，你至少已经掌握：

- 如何把板级资料转换成 Rust 的 GPIO/SPI 配置
- 如何把 LCD 初始化问题拆成“背光链路”和“像素链路”
- 如何先用主机侧测试锁定布局，再去做硬件验证

## 下一课建议

下一课可以继续做下面两件事之一：

1. 把 `Hello World` 扩展成多行文本和简单状态栏。
2. 接入按键，让屏幕内容能根据输入变化。
