# no_std 库清单（权威索引 + 实测数据）

> 数据来源：`rust-unofficial/awesome-rust`（59,618★）+ `rust-embedded/awesome-embedded-rust`（8,119★）
> + crates.io API 实际下载量。抓取时间 2026-10-01。

---

## 先说一个重要发现

**`awesome-rust` 本身没有独立的 `no_std` 章节。**

它的做法是：no_std 条目散落在各分类（Serialization、Computer vision、
Cryptography、Robotics…），而嵌入式相关内容**全部外链**到
[`rust-embedded/awesome-embedded-rust`](https://github.com/rust-embedded/awesome-embedded-rust)。

那份才是 no_std 库的**权威索引**，里面有专门的 `## no-std crates` 章节，列了 **95 个 crate**。

### awesome-rust 里散落的 no_std 条目（全部）

| crate | 分类 | 说明 |
|---|---|---|
| `jamesmunns/postcard` | Serialization | `#![no_std]` 优先的 serde 序列化器 |
| `0xlane/pe-sign` | Cryptography | 跨平台 no-std PE 签名验证 |
| `esp-rs/esp-hal` | Embedded | ESP32 裸机 HAL |
| `AFLplusplus/LibAFL` | Fuzzing | 模糊测试，支持 no_std |
| `mikwielgus/undoredo` | Data structures | Undo/Redo，兼容 no_std + serde |
| `rust-cv/cv` | Computer vision | 尽可能支持 `#[no_std]` |
| `infinition/waveshare-watch-rs` | Embedded | 100% Rust no_std 手表固件 |
| `rsasaki0109/rust_robotics` | Robotics | 机器人算法，no_std 支持 |

---

## awesome-embedded-rust 的 `## no-std crates` 全章节（95 个）

### 网络 / 协议栈 ← lilyco 最关心

| crate | 下载量 | 说明 |
|---|---:|---|
| **`wtx`** | 70,493 | **HTTP / WebSocket / gRPC / SMTP / PG / TLS / X.509，no_std 一等公民**（已实测编译通过）|
| **`smoltcp`** | **8,460,822** | 无 alloc 的 TCP/IP 栈，4,610★。`wtx` 的可选底层后端之一 |
| `embassy-net` | 630,757 | embassy 异步网络（TCP/UDP），embassy 总仓库 9,895★ |
| `minimq` | 144,586 | 极简 MQTT5 客户端，面向 no_std |
| `mqtt-sn` | — | MQTT-SN 协议实现 |
| `embedded-tls` | 164,010 | **TLS 1.3，no_std 环境**（246★）|
| `embedded-websocket` | 58,589 | 轻量 WebSocket 服务端+客户端 |
| `ieee802154` | — | IEEE 802.15.4 部分实现 |
| `lorawan-encoding` / `lorawan-device` | — | LoRaWAN 编解码 + MAC |
| `sntpc` | — | SNTP 客户端（从 NTP 取时间）|
| `bluetooth-hci` | — | 设备无关的蓝牙 HCI |
| `usbpd` | — | USB-PD（Sink 模式）|

### 集合 / 数据结构 ← 替 String/Vec/HashMap

| crate | 下载量 | 说明 |
|---|---:|---|
| **`heapless`** | **147,383,626** | `Vec`/`String`/`LinearMap`/`RingBuffer`，固定容量无堆（2,026★）|
| `managed` | 14,702,199 | `ManagedSlice`/`ManagedMap`，可在堆与固定缓冲间切换 |
| `intrusive-collections` | — | 侵入式（零分配）链表 + 红黑树 |
| `scapegoat` | 41,546 | **仅在栈上**的 `BTreeSet`/`BTreeMap` 替代 |
| `bbqueue` | — | SPSC 静态队列（BipBuffer），适合 DMA |
| `null-terminated` | — | 泛型 NUL 结尾数组 |
| `static-bytes` | — | 无动态分配缓冲区 |
| `wyhash` | — | 快速可移植哈希 + RNG |
| `sized-dst` | — | 栈上的 DST 容器（如 trait object）|

### 序列化

| crate | 下载量 | 说明 |
|---|---:|---|
| `postcard` | 68,148,579 | `#![no_std]` serde 序列化器 |
| `miniconf` | — | 按路径读写异构树节点 |
| `micropb` | — | Protobuf，无 allocator |
| `endian_codec` | — | 打包字节编解码，支持 derive |
| `scroll` | 88,762,874 | 端序感知的 Read/Write traits |

### 错误 / panic 处理

清单里有专门的 `## Panic handling` 章节（第 1425 行起），
但**没有独立的错误处理 crate** —— 因为 `core::error::Error` 自 Rust 1.81 起已在 core 稳定，
这正是我在 lilyco 实验里手写替换 `thiserror` 的依据。

### CLI ← 替 clap（lilyco-cli 相关）

| crate | 下载量 | 说明 |
|---|---:|---|
| `embedded-cli` | 53,678 | **自动补全、子命令、选项、help、历史** |
| `light-cli` | 1,949 | 轻量 heapless CLI（WIP）|
| `menu` | — | 基础 CLI 库，嵌套菜单 + help |
| `embedded-gui` | — | 受 Pebble / LVGL 启发的 GUI |
| `guillotine` | — | 零分配 GUI，建在 `embedded-graphics` 上 |
| `Slint` | — | 声明式 GUI，可在微控制器上跑 |

### 解析器 ← 替通用文本处理

| crate | 下载量 | 说明 |
|---|---:|---|
| `nom` | **756,402,004** | parser combinator（no_std 支持）|
| `combine` | 275,358,715 | parser combinator，categories 明确含 no-std |
| `miniconf` | — | 树形结构路径访问 |

### 图形 / 显示

`embedded-graphics` 生态：`tinybmp`（no-alloc BMP 解析）、`embedded-3dgfx`（3D 引擎）、
`vga-framebuffer`（VGA 信号生成 + 字体渲染）、`pc-keyboard`（PS/2 键盘协议）、
`console-traits`（文本控制台抽象）

### 数学 / DSP

`nalgebra`（低维线性代数）、`micromath`（浮点近似）、`microfft`（无 alloc FFT）、
`fixed-fft`（定点 FFT）、`idsp`（整数 DSP）、`biquad`（IIR 滤波）、
`adskalman`（卡尔曼滤波）、`cam-geom`（相机几何模型）、
`metrology_insight`（IEC 61000-4-30 Class S 电能质量）

### 位操作 / 寄存器

`bitbybit`、`bitfield-struct`、`bit_field`、`bitmatch`、`arbitrary-int`（`u5`/`u120` 等）、
`register-rs`（MMIO + CPU 寄存器统一接口）、`bounded-registers`（带越界检查）、
`atomic`（泛型 `Atomic<T>`）

### 调试 / 工具

| crate | 说明 |
|---|---|
| `gdbstub` | 零分配、纯 Rust 的 GDB Remote Serial Protocol（414★）|
| `probe-rs` | 嵌入式调试工具链（烧录 + 调试 ARM/RISC-V）|
| `qemu-exit` | 用自定义退出码结束 QEMU（单测用）|

### WIP（进行中）

| crate | 说明 |
|---|---|
| `krypteia` | 纯 Rust no_std 后量子密码（ML-KEM/ML-DSA/SLH-DSA），零依赖 |
| `Rubble` | 纯 Rust 嵌入式 BLE 栈 |
| `OxCC` | 开源汽车控制（Open Source Car Control）移植 |
| `post-haste` | no_std / 无 alloc 的模块化 async 库 |

---

## 对 lilyco 的直接映射

| lilyco 现状依赖 | no_std 替代 | 实测状态 |
|---|---|---|
| `thiserror` | `core::error::Error`（1.81+ 已稳定）| ✅ 已在实验里手写替换成功 |
| `serde_json` | **不用换** —— 开 `default-features=false, features=["alloc"]` | ✅ 实测通过 |
| `serde` derive | 冷路径改手写 `Serialize` | ✅ 实测 cold −48% |
| `clap`（lilyco-cli）| `embedded-cli`（53,678 dl）/ `light-cli` | ⚠️ 未实测 |
| `tokio`（lilyco-gui）| `embassy` / `wtx::executor::NoStdRuntime` | ✅ NoStdRuntime 实测通过 |
| `crossterm`（lilyco-tui 后端）| **渲染层改用 `ratatui-core`**（已 `#![no_std]`）| ✅ 实测通过；crossterm 本身 ❌ |

> ⚠️ **本条已修正**（原写「crossterm 无替代」）。正确做法见
> `TUI_NOSTD_CORRECTION.md`：**ratatui-core 已经 no_std**（issue #1750 已关闭），
> 只需把终端后端抽象掉。crossterm 自身确实依赖 std（编译器 E0152 实证）。
| HTTP/JSON-RPC（lilyco-mcp）| `wtx` | ✅ 已实测编译通过 |

> 注：下载量是「累积下载次数」，受 CI 与镜像放大，只用于横向比较量级，
> 不等于用户数。star 数同理。

---

## 检索方法（可复现）

```bash
# 权威索引 1：通用 awesome-rust（no_std 条目散落，无独立章节）
gh api repos/rust-unofficial/awesome-rust/contents/README.md \
  --jq '.content' | base64 -d > awesome-rust.md
grep -niE 'no[_-]std' awesome-rust.md

# 权威索引 2：嵌入式 awesome（有专门的 ## no-std crates 章节，95 个 crate）
gh api repos/rust-embedded/awesome-embedded-rust/contents/README.md \
  --jq '.content' | base64 -d > awesome-embedded.md
awk '/^## no-std crates/,/^## Panic handling/' awesome-embedded.md

# 下载量 / no-std 分类（crates.io API，注意需要 User-Agent）
python -c "
import json,urllib.request
r=urllib.request.Request('https://crates.io/api/v1/crates/heapless',headers={'User-Agent':'x'})
d=json.load(urllib.request.urlopen(r))['crate']
print(d['downloads'], d['max_version'], d['categories'])
"
```

**踩坑**：crates.io API 缺 `User-Agent` 会返回 **403**（不是空 body，而是被 WAF 拦），
`curl` 不加 `-H` 时容易误判成「解析失败」。sparse index（`index.crates.io`）
的路径分片规则是 1/2/3 字符分别走 `1/` `2/` `3/{首字母}/`，写错会 404。
