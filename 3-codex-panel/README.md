# 第 3 课：通过 WiFi 把正在处理中的 Codex 会话目录显示到板载屏幕

## 本课目标

这一课做一条最小但完整的链路：

1. 电脑上继续正常使用 `codex`
2. 用 hooks 只追踪“当前这轮会话是否还在处理”
3. 电脑启动一个本地 Rust TCP server
4. `ESP32S3M` 通过 WiFi 连到电脑，拉取当前活跃会话快照
5. 屏幕显示活跃会话目录，LED 在有活跃会话时慢闪，没有活跃会话时熄灭

这一课不做：

- 不接 `codex app-server`
- 不做 websocket
- 不做 transcript 同步
- 不做 token 级流式渲染
- 不做桌面 GUI

## 最终方案

链路固定为：

```text
Codex hooks
  -> runtime/codex-state/*.json
  -> Rust panel-bridge TCP server
  -> ESP32S3 WiFi TCP client
  -> LCD + GPIO1 LED
```

职责划分如下：

- `scripts/codex_panel_hook.mjs`
  - 接收 Codex hook 的 stdin JSON
  - 把每个 `session_id` 的状态写进 `runtime/codex-state/*.json`
- `src/bin/panel_bridge.rs`
  - 在电脑上启动 TCP server
  - 轮询状态目录并向所有已连接板子广播快照
- `src/panel_bridge_host.rs`
  - host 侧状态聚合、目录标签裁剪、TCP 推送逻辑
- `src/main.rs`
  - 板端接入 WiFi
  - 通过 DHCP 获取地址
  - 作为 TCP client 连接电脑
  - 解析快照并刷新 LCD/LED
- `src/lib.rs`
  - 板端协议解析与共享工具函数

## 为什么用 hooks 而不是重方案

这次需求的核心不是“订阅 Codex 全量内部事件”，而是：

- 哪些会话现在还在处理
- 每个会话对应哪个工作目录
- 板子如何低成本显示出来

对这个目标，hooks 已经足够。

本课的“活跃”定义非常明确：

- `SessionStart`
  - 只建档，不算活跃
- `UserPromptSubmit`
  - 标记本轮会话进入处理中
- `Stop`
  - 标记本轮会话退出处理中

所以屏幕展示的是“当前这轮还在处理的会话”，不是“Codex 进程还开着的终端”。

## 为什么标签显示目录

标题不再取 prompt，而是取 `cwd` 最后两级目录：

1. 目录比 prompt 稳定
2. 同类任务的 prompt 很像，目录更容易快速辨认
3. 板载屏幕空间有限，目录更适合做短标签

当前规则：

1. 取会话 `cwd`
2. 只保留最后两级目录
3. 非 ASCII 替换成 `?`
4. 超过长度后裁成 `...`

例如：

```text
/Users/langhuam/workspace/self/embedded-systems-lab
-> self/embedded-systems-lab

/Users/langhuam/workspace/self/embedded-systems-lab/3-codex-panel
-> embedded-systems-lab/3-co...
```

## Host 和 Board 的最小协议

板端是 `no_std`，不值得为了教学 demo 搬 JSON 解析器进去。

所以 host 侧只发最简单的行协议：

```text
SNAP
ACTIVE 1
COUNT 2
TITLE 0 embedded-systems-lab/3-co...
TITLE 1 self/embedded-systems-lab
END
```

其中：

- `ACTIVE 1`
  - 当前是否存在至少一个活跃会话
- `COUNT`
  - 当前快照包含多少条目录
- `TITLE i`
  - 第 `i` 条目录标签

除此之外，连接建立后还会互发最小握手/心跳：

```text
HELLO codex-panel 1
HELLO esp32-panel
PING <uptime_ms>
```

电脑端只维护会话快照，不负责闪烁节奏；LED 慢闪由板端本地每 `500ms` 翻转一次。

## 当前硬件事实

- 开发板：`ESP32S3M`
- 屏幕：板载 `0.96"` IPS LCD
- 驱动芯片：`ST7735S`
- 逻辑分辨率：`160x80`
- 当前 USB 口：`/dev/cu.usbmodem59090680081`
- WiFi：
  - SSID: `Xiaomi_2E16`
  - Password: `fangtang1234`

LCD 引脚沿用前两课：

| 信号   | GPIO |
| ------ | ---- |
| `MOSI` | `11` |
| `SCLK` | `12` |
| `MISO` | `13` |
| `CS`   | `39` |
| `DC`   | `40` |
| `RST`  | `38` |
| `BL`   | `41` |

状态灯：

| 信号  | GPIO | 极性       |
| ----- | ---- | ---------- |
| `LED` | `1`  | 低电平点亮 |

## 工程结构

```text
3-codex-panel/
├── .cargo/config.toml
├── .vscode/tasks.json
├── Cargo.toml
├── Makefile
├── README.md
├── build.rs
├── hooks/
│   └── hooks.json
├── runtime/
├── scripts/
│   ├── codex_panel_hook.mjs
│   ├── flash_with_esptool.py
│   ├── install_hooks.py
│   └── run_panel.sh
├── src/
│   ├── bin/
│   │   └── panel_bridge.rs
│   ├── lib.rs
│   ├── main.rs
│   └── panel_bridge_host.rs
└── tests/
    ├── test_codex_panel_hook.mjs
    └── test_panel_bridge.rs
```

## 学习过程

### 第 1 步：先把“运行中”定义清楚

最容易犯的错，是把“Codex 终端还开着”误判成“会话正在运行”。

这一课只承认：

- `UserPromptSubmit` 到 `Stop` 之间
  - 算处理中的会话
- 其他状态
  - 不算

这一步把问题先从“抓所有内部事件”收缩成“只判断当前这轮是否还没结束”。

### 第 2 步：先把目录标签做稳定

如果标题继续用 prompt，会导致：

1. 同类任务看起来都差不多
2. prompt 太长，不适合小屏幕
3. prompt 会变化，目录不会

因此这课直接以目录为准，保证板子上能一眼看出“哪个工作区正在跑”。

### 第 3 步：host 改为 Rust TCP server

用户不要重型 `codex server`，也不要 Python 串口桥。

所以最终 host 端改成：

- 用 Node hooks 落本地 JSON 状态
- 用 Rust TCP server 对外广播快照

这样电脑端一直只是一个很轻的本地桥，不引入额外服务复杂度。

### 第 4 步：板端直接走 WiFi + TCP client

板子不再依赖 USB 串口收快照，而是：

1. 连入本地 WiFi
2. DHCP 获取 IP
3. 主动连接电脑的 `31337` 端口
4. 持续读取快照

当前默认通过 `Makefile` 在构建时注入：

- `CODEX_PANEL_WIFI_SSID`
- `CODEX_PANEL_WIFI_PASSWORD`
- `CODEX_PANEL_SERVER_IP`
- `CODEX_PANEL_SERVER_PORT`

默认电脑 IP 通过 `ipconfig getifaddr en0` 探测。

### 第 5 步：LED 闪烁节奏放在板端

电脑端只发“当前是否活跃”的状态，板端自己控制慢闪。

这样做的好处是：

- 协议最简单
- host 不需要高频推送亮灭节奏
- 网络抖动不会直接影响 LED 节奏

## 运行前准备

先安装 ESP Rust 工具链：

```bash
cargo install espup
espup install
```

重新打开 shell 后，准备 lesson 环境：

```bash
cd 3-codex-panel
make setup-esptool
node --version
```

## 一次性安装 hooks

在第三课目录执行：

```bash
make install-hooks
```

它会把本课 hooks 合并进用户级：

```text
~/.codex/hooks.json
```

不会覆盖你原来的其他 hooks，只会增删本课自己的 Node 命令。

卸载时执行：

```bash
make uninstall-hooks
```

如果你确实还想装成仓库级 hooks，再用：

```bash
make install-hooks-repo
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

烧录固件：

```bash
make flash
```

启动 Rust TCP server：

```bash
make panel
```

如需覆盖默认网络参数，可在命令行传入：

```bash
make build WIFI_IFACE=en0 PANEL_SERVER_PORT=31337
make flash WIFI_SSID=Xiaomi_2E16 WIFI_PASSWORD=fangtang1234
make panel PANEL_SERVER_BIND=0.0.0.0 PANEL_SERVER_PORT=31337
```

## 真实联调结果

本课已经完成以下真实验证：

1. `make flash PORT=/dev/cu.usbmodem59090680081`
   - 固件成功烧录到板子
2. `make panel`
   - Rust server 成功监听 `0.0.0.0:31337`
3. 板子成功通过 WiFi 建立 TCP 连接
   - 实测连接：`192.168.31.61:49500 -> 192.168.31.52:31337`
4. 真实 `codex exec` 会话触发 hooks
   - 状态文件在处理期间变为 `active=true`
   - 完成后回落为 `active=false`
5. Rust server 能实时输出快照
   - 处理期间实测发出 `ACTIVE 1`
   - 快照内包含目录标签 `embedded-systems-lab/3-co...`
6. host 断链容错已验证
   - 板子断链 / 重连不会再把 `panel-bridge` 直接打挂
   - macOS 上出现 `Os { code: 60, kind: TimedOut }` 时，host 会把它当作瞬时网络错误处理
   - 修复后本地探针可继续读到 `HELLO/SNAP` 快照

## 预期现象

正常情况下：

1. 当没有活跃会话时：
   - 屏幕显示 `Codex Panel`
   - 正文显示 `No active chats`
   - LED 熄灭
2. 当你在本仓库里发起新的 `codex` prompt 时：
   - 当前工作目录标签出现在屏幕上
   - LED 开始慢闪
3. 当本轮处理结束时：
   - 该会话从活跃列表移除
   - 如果没有其他活跃会话，LED 熄灭

## 关键实现约束

- 本课只追踪本仓库内的 Codex 会话
- 显示标签来自会话 `cwd` 的最后两级目录
- “运行中”由 `UserPromptSubmit -> Stop` 定义
- 协议是快照协议，不是流协议
- 电脑端只负责状态汇总和广播
- 闪烁节奏固定在板端本地实现

这课的重点不是做一个大而全的面板系统，而是把“电脑上的 agent 状态”稳定、低成本地投到一块嵌入式屏幕上。
