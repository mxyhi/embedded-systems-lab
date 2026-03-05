# 2-wifi（Rust 版）

这是 `openCH 赤菟 (CH32V307VCT6)` 的第二课：
- 使用 `UART6` 驱动 `ESP8266`（AT 固件）
- 完成最小闭环：`AT` + `AT+GMR` 指令交互
- 保留最小实现，先打通链路，再扩展联网功能

## 课前确认

1. 硬件连接（按板级默认）
- `PC0 -> ESP8266_RX`
- `PC1 -> ESP8266_TX`
- ESP-01/ESP-01S 天线朝板外

2. 固件建议
- ESP8266 使用 AT 固件（推荐可稳定响应 `AT` 与 `AT+GMR`）

3. 工具链

```bash
rustup toolchain install nightly
cargo install wlink --locked
```

## 命令

```bash
cd 2-wifi
source ~/.zshrc
make test   # 主机侧单元测试：AT 响应 token 解析
make check  # 交叉编译检查
make build  # release 构建
```

下载运行：

```bash
make flash
```

## 运行现象（验收标准）

1. `make test` 通过（4/4）。
2. `make check` / `make build` 通过。
3. 上板后，如果 ESP8266 通信正常：
- SDI 日志可看到 `AT` 与 `AT+GMR` 响应
- `LED1(PE11)` 快速闪烁（约 150ms 切换）
4. 若通信失败：
- SDI 日志显示 `wifi init check: fail`
- `LED1` 慢闪（短亮长灭，便于区分失败状态）

## 代码结构

- `src/main.rs`
- 初始化 `USART6`（`REMAP=0`, `PC1/PC0`）
- 发送 AT 命令并轮询接收响应（`nb_read` + 空闲超时）
- 通过 `contains_token` 判断响应中是否包含 `OK`

- `src/lib.rs`
- `contains_token`（no_std 可用）
- 配套单元测试（主机侧）

## 排障

### 1) 一直失败慢闪
- 检查 TX/RX 是否交叉连接（`PC0->RX`, `PC1->TX`）
- 检查 ESP8266 供电是否稳定（3.3V）
- 模块可能处于透传态，先断电重上

### 2) `make bin` 报 objcopy 不存在
先执行：

```bash
source ~/.zshrc
```

再重试 `make bin`。

### 3) 能收到回包但无 `OK`
- 先单独发送 `AT\r\n`
- 确认波特率 `115200`
- 确认 AT 固件版本与指令兼容

## 第二课学习打卡

- [x] 理解 openCH 的 WiFi 引脚映射（UART6）
- [x] 完成 USART6 阻塞发送 + 轮询接收
- [x] 完成 `AT` / `AT+GMR` 最小闭环
- [ ] 下一步：`AT+CWMODE=1` + `AT+CWJAP` 入网
- [ ] 再下一步：`AT+CIPSTART` + HTTP GET
