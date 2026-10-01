# lilyco-no_std 实测结论：no_std 不提速也不瘦身

> 目标：回答「把 lilyco 做成 no_std，能否提高 debug 编译速度、减少 debug 体积」
> 机器：rustc 1.98.1-stable（LLVM 22.1.8），x86_64-pc-windows-msvc，MSVC 14.51
> 方法：同源代码体 + 单变量对照，每组 N=3 取最小

---

## 一句话结论

**`#![no_std]` 本身对 debug 编译速度和体积几乎零影响（cold −1.4%，exe 字节数完全相同）。**
它是一条**可移植性**特性，不是**性能**特性。真正决定 debug 快慢与胖瘦的是另外两件事：
**① 依赖里有多少 proc-macro（syn 链）② 有没有关掉 PDB。**

---

## 实验设计：怎么把变量拆开

在 `D:/Code/rust/question/lilyco-nostd/` 建了 5 个变体，**域模型与 lilyco-core 同构**
（ArgSchema / ArgKind / CommandSchema / Progress / LogLevel / AppError / Registry），
driver（std bin）逐字一致，四组 JSON 输出校验逐字相同。

| 变体 | 依赖形态 | 用来隔离的变量 |
|---|---|---|
| **A** | serde(derive) + serde_json + thiserror（**当前 lilyco-core 真实形态**）| 基线 |
| **D** | `#![no_std]` + serde(no_std) + **serde_json(`alloc`)** | 最小改动迁移路径 |
| **B** | `#![no_std]` + serde(no_std) + **serde-json-core** | 换 JSON 库 |
| **C** | 与 B **完全相同，仅删掉 `#![no_std]` 一行** | ← **no_std 属性本身** |
| **E** | `#![no_std]` + serde **不开 derive**（手写 Serialize）+ serde_json | ← **proc-macro 链** |

B 与 C 的 `lib.rs` 差异实测只有 1 行（`diff` 验证），这是本实验最关键的一组对照。

---

## 结果（N=3 最小值）

### 默认 dev profile

| 变体 | cold ms | incr ms | exe B | target B |
|---|---:|---:|---:|---:|
| **A 现状** | 7709 | 877 | 664,064 | 117,032,382 |
| D no_std + serde_json(alloc) | 7190 | 827 | 666,112 | 107,582,348 |
| **B no_std + serde-json-core** | 7673 | 796 | 511,488 | 91,831,583 |
| **C 同 B 但去掉 `#![no_std]`** | **7786** | 817 | **511,488** | 92,312,413 |
| **E no_std + serde 不开 derive** | **3996** | 709 | **291,840** | 56,841,502 |

**B vs C（唯一变量 = `#![no_std]`）**：cold 7673 → 7786（no_std 快 1.4%，在噪声量级），
**exe 511,488 = 511,488 完全相同**，target 差 0.5%。
→ **no_std 对 debug 体积没有任何可观测贡献。**

### 叠加 `-C debuginfo=0 -C link-args=/DEBUG:NONE`（零代码改动）

| 变体 | cold ms | exe B | target B | target 缩小 |
|---|---:|---:|---:|---:|
| A | 7186 | 613,376 | 61,849,889 | **1.9×** |
| D | 6849 | 614,400 | 58,477,172 | 1.8× |
| B | 7466 | 479,744 | 52,570,611 | 1.7× |
| **E** | **3995** | **273,920** | **29,222,731** | — |

### 相对 A 组基线的总账（E + 去 PDB）

| 指标 | A 现状 | E + 去 PDB | 变化 |
|---|---:|---:|---:|
| 冷编译 | 7709 ms | **3995 ms** | **−48%** |
| 增量编译 | 877 ms | 698 ms | −20% |
| exe | 664,064 B | **273,920 B** | **−59%** |
| target/ | 117,032,382 B | **29,222,731 B** | **−75%** |

---

## 归因：7.7 秒到底花在哪（`cargo build --timings`）

**A 组**（单元 CPU 总耗时 22.2 s，wall 7.5 s）：

```
2.70s 12.2%  serde_derive v1.0.229      ← proc-macro 链
2.20s  9.9%  serde_core v1.0.229
1.70s  7.7%  syn v3.0.6                 ← proc-macro 链
1.70s  7.7%  serde_json v1.0.151
1.30s  5.9%  thiserror-impl v2.0.21     ← proc-macro 链
1.00s  4.5%  serde_core build-script (run)
0.90s  4.1%  zmij v1.0.23               ← serde_json 的浮点格式化依赖
0.80s  3.6%  memchr v2.8.3
0.80s  3.6%  lc_dep（自己的代码）
─────────────────────────────────────
proc-macro 链合计 10.20s = 45.9%
```

**B 组**（总 14.8 s）：proc-macro 链 6.10 s = 41.2%（去掉了 thiserror-impl）

关键：**wall 时间由串行关键路径决定**，而 proc-macro 链
（proc-macro2 build → syn → serde_derive → serde → …）正好铺在这条路径上。
所以哪怕 B 组 CPU 总量降了 33%（22.2→14.8 s），wall 时间纹丝不动 ——
**并行度救不了串行链**。E 组把这条链整个拔掉，wall 才真正掉下来。

---

## 三个杠杆，按收益排序

### 杠杆 1：砍 proc-macro 依赖（cold −48%，exe −56%）

不是"自己写库"，而是**少让 `derive` 进依赖树**。三条路，从轻到重：

1. **先删 `thiserror`** —— 它只提供一个 `Display` impl，却拖进
   `thiserror-impl`（1.30 s）+ build script（0.80 s）。手写 `impl Display` + `impl Error`
   十几行搞定，`core::error::Error` 自 Rust 1.81 起已在 core 稳定，no_std 可直接用。
2. **`serde` 保留、但按 crate 粒度决定要不要 `derive`** —— 冷路径上的 crate 手写
   `Serialize`（用 serde 自己的 trait，不是自己造序列化库），热路径继续 derive。
3. 别为「少一个依赖」去做不划算的重写 —— lc_dep 自身只占 0.8 s / 3.6%，**优化自己代码的
   收益远小于优化依赖树**。

### 杠杆 2：关掉 PDB（target −47%~−53%，零代码改动）

```toml
# .cargo/config.toml
[build]
rustflags = ["-C", "debuginfo=0", "-C", "link-args=/DEBUG:NONE"]
```

**真实 lilyco-core 实测**（`D:/Code/rust/question/lilyco-src`）：

| | cold | target/ |
|---|---:|---:|
| 默认 | 8105 ms | **129,899,588 B（124 MB）** |
| 加上面两行 | 7584 ms | **61,163,173 B（58 MB）** |

target 直接砍掉 **68 MB / 53%**，代码一行没动。注意 `[profile.dev] debug = false`
**不够** —— rustc 仍然会给 MSVC 传 `/DEBUG`，必须显式 `link-args=/DEBUG:NONE`。
代价：断点调试失效；建议只在 CI 与"跑一下看输出"的日常循环里开。

### 杠杆 3：换 JSON 库（exe −23%，但要付代价）

`serde_json` → `serde-json-core` 让 exe 从 664 KB 降到 511 KB（−23%），但也带来硬伤（见下）。

---

## 坑：`serde-json-core` 反序列化不了 internally-tagged enum

实测报错 **`AnyIsUnsupported`**。原因：serde-json-core 不实现 `deserialize_any`，
而 `#[serde(tag = "type")]`（internally tagged）必须先缓冲再分派，强依赖它。

**而 lilyco-core 的 `ArgKind` 和 `Progress` 偏偏都写了 `#[serde(tag = "type", rename_all = "snake_case")]`。**
序列化正常，**反序列化必挂** —— MCP 侧要接 JSON-RPC，这条走不通。

### 好消息：serde_json 自己就是 no_std 的

```toml
serde_json = { version = "1", default-features = false, features = ["alloc"] }
```

D 组实测：**在 `#![no_std]` 下编译通过，internally-tagged enum 反序列化也正常**。
也就是说 **lilyco 要上 no_std，根本不用换 JSON 库**，改一行 Cargo.toml 即可。
（D 组 exe 666 KB 比 A 组 664 KB 还略大 —— 反而印证了换库不是体积的关键。）

---

## lilyco 迁移的边界（诚实版）

**能 no_std 的**：只有纯领域层。

- `lilyco-core` 的 Schema / Progress / Error 部分：可（D 组已验证路径通）
- 实际上 `lilyco-core` 现在 std 耦合很重（`std::io::Error`、`serde::de::Visitor`、
  `serde_json::Value`），要拆得先动这几个点

**⚠️ 下面这段原结论已被推翻，见修正章节**

> ~~不能 no_std 的：`lilyco-cli`（clap）、`lilyco-tui`（crossterm/ratatui）、
> `lilyco-gui`（tokio）、`lilyco-mcp` —— 全部依赖 io / net / thread / 时间，这些 std 才有~~

**这个推论是错的**，已被用户指出并经实测推翻 → 见下节。

---

## 🔴 修正（2026-10-01 11:xx）：no_std 生态远比我想的完整

原结论犯了「std 是唯一实现」的定势错误。实测证明 **io / net / 线程 / 时间 / 集合在 no_std 下都有成熟替代**。

### 反例：`wtx`（401★，MPL-2.0，edition 2024，2026-09-27 仍在更新）

`c410-f3r/wtx` —— 一个**以 no_std 为一等公民**的 Web/网络协议栈，`categories` 里直接写着 `no-std`。
它的做法是把「运行时」和「网络后端」抽象成可替换层：

```
wtx/src/executor/   no_std_runtime.rs   ← 与 std_executor.rs / tokio_executor.rs 并列
wtx/src/net/        embassy_net.rs      ← 与 std.rs / tokio.rs 并列
wtx/src/collections/ ArrayString / ArrayVector / ShortBoxStr …（无堆集合）
wtx/src/http/       generic_request.rs / protocol.rs / method.rs（纯协议，无 std）
```

依赖清单里**几乎每一项都写了 `default-features = false`** —— 包括 `tokio`、`serde_json`、
`socket2`、`zip`、`uuid`、全套 RustCrypto。`[features] default = []`（默认零 feature）。

**实测（本机编译验证，非读文档）**：

```toml
wtx = { version = "0.52", default-features = false, features = ["http"] }
```
```rust
#![no_std]
extern crate alloc;
use wtx::http::{Method, Request, Response};
use wtx::collections::{ArrayString, ArrayVector};
use wtx::executor::NoStdRuntime;
```
→ **编译通过**（wtx 0.52.1，6.19 s）。`http` 模块可以在**不开 `std` feature** 的情况下使用。

### no_std 生态的真实版图（实测 star）

| 领域 | 库 | ★ | 说明 |
|---|---:|---:|---|
| **网络协议栈** | `wtx` | 401 | HTTP/1.1、HTTP/2、WebSocket、gRPC、SMTP、PG、TLS、X.509 全可 no_std |
| 嵌入式网络抽象 | `rust-embedded-community/embedded-nal` | 201 | 网络抽象层 |
| 嵌入式网络实现 | `embassy-net` | — | 真正跑 TCP/UDP 的 no_std 实现 |
| 嵌入式时间 | `embassy-time` | — | no_std 时钟/定时 |
| async 执行器 | `jamesmunns/cassette` | 86 | 单 future 非阻塞执行器 |
| async 执行器 | `kabergstrom/lonely` | 19 | no_std+alloc，无 TLS |
| no_std 标准库 | `japaric/steed` | 519 | 「无 C 依赖的 std」实验（INACTIVE，但思路可参考） |
| 集合 | `heapless` | 2026 | 静态友好 Vec/String/Deque |
| 集合 | `hashbrown` | 2997 | 哈希map，no_std + alloc |

### 对 lilyco 的修正结论

原结论「cli/tui/gui/mcp 永远 no_std 不了」**不成立**。修正后的真实边界：

- **`lilyco-mcp`** —— 完全可能 no_std。它本质是 HTTP/JSON-RPC 服务器，
  用 `wtx`（http + web-socket）替代当前实现即可，网络后端选 `embassy-net`。
- **`lilyco-cli`** —— 主要障碍是 `clap` 而非 std。no_std 有 `bpaf`、`argh`、
  `lexopt`（极简，手写解析）等，`lexopt` 尤其轻。
- **`lilyco-tui`** —— 主要障碍是 `crossterm`（要 termios/ioctl，属真正硬依赖）。
  纯渲染层（ratatui 的 buffer/diff 部分）可以 no_std，输入/终端控制不行。
  > ⚠️ **此处已二次修正**：`ratatui-core` **已经实现 `#![no_std]`**（tracking issue #1750 已关闭，
  > `default = []`，`std` 为可选 feature，`hashbrown` 替代 `HashMap`，`critical-section` 替代
  > `thread_local`）。本机实测 `ratatui-core 0.1.2` + `#![no_std]` **编译通过，依赖图全链路无 std-only**。
  > 详见 `TUI_NOSTD_CORRECTION.md`。crossterm 自身确实依赖 std（编译器 E0152 实证），
  > 但那是**后端**问题，不是渲染层问题 —— 拆层即可。
- **`lilyco-gui`** —— 依赖 tokio 的部分可换成 `embassy` 或 `NoStdRuntime`；
  但若走 WebView/系统 GUI，那确实绑平台。

**所以"lilyco-no_std"的合理形态仍是一个 feature 开关**，
但覆盖面比我原先写的大得多 —— **至少 core + mcp 是现实目标**。

收益依然主要是**可移植性**（嵌入式 / WASM / Termux / 无 OS 环境），
不是速度 —— 速度那条结论（no_std 本身 ≈ 0 收益）不受影响，有 B/C 对照实测支撑。

---

## 给 lilyco 的行动清单（按性价比）

| 优先级 | 动作 | 预期收益 | 代价 |
|---|---|---|---|
| 1 | 加 `-C debuginfo=0 /DEBUG:NONE` | target −53%（124 MB → 58 MB） | 1 行配置，丢断点 |
| 2 | 删 `thiserror`，手写 `Display` + `core::error::Error` | 去掉 1.3 s + build script | ~15 行代码 |
| 3 | `serde_json` 开 `default-features=false, features=["alloc"]` | 解锁 no_std 可能 | 1 行 |
| 4 | 冷路径 crate 去掉 `serde` derive，手写 `Serialize` | cold −48%（视覆盖面） | 中等重构 |
| 5 | `#![no_std]` | ≈ 0 | **别为性能做，只为可移植性做** |

---

## awesome-rust：no_std 库清单（star 数 2026-10-01 实测）

### 序列化 / 编解码

| 库 | ★ | 说明 |
|---|---:|---|
| `hashbrown` | 2997 | SwissTable 哈希map，no_std + alloc |
| `postcard` | 1536 | no_std + serde 兼容的紧凑二进制格式 |
| `serde` | — | **本身 no_std 就绪**：`default-features=false, features=["derive","alloc"]` |
| `serde_json` | — | **本身 no_std 就绪**：`default-features=false, features=["alloc"]` |
| `serde-json-core` | 199 | serde-json 的 no_std 版，**不支持 `deserialize_any`** |
| `heapless` | 2026 | 静态友好数据结构（Vec/String/Deque） |
| `sval` | 111 | 轻量 no_std、object-safe 序列化 |
| `micropb` | 143 | 面向嵌入式的 Protobuf |
| `rasn` | 385 | ASN.1 / BER / CER / DER |
| `lite-json` | 49 | WASM/no_std 就绪的简易 JSON |
| `nojson` | 51 | 零依赖、无宏、无 unsafe 的 JSON |
| `core-json` | 29 | 非分配式 no_std JSON 反序列化器 |
| `microjson` | 21 | 无分配的 no_std JSON 解析器 |
| `bitflags` | 1163 | 位标志宏，no_std |

### 错误 / 日志

| 库 | ★ | 说明 |
|---|---:|---|
| `defmt` | 1234 | 嵌入式高效延迟格式化日志 |
| `core-error` | 39 | 在 no_std 下提供 `Error` trait |
| `no_error` | 18 | no_std + no_alloc 极简错误库 |
| — | — | **推荐**：`core::error::Error` 自 Rust 1.81 已在 core 稳定，通常不需要额外库 |

### 内存分配

| 库 | ★ | 说明 |
|---|---:|---|
| `talc` | 562 | 快且灵活的 no_std / WASM 分配器 |
| `embedded-alloc` | 476 | 嵌入式堆分配器 |
| `rust-alloc-no-stdlib` | 180 | Dropbox 出品，让 no_std 库能分配 |
| `buddy-alloc` | 32 | 伙伴分配器 |

### 终端 / 显示 / 网络 / 其它

| 库 | ★ | 说明 |
|---|---:|---|
| `embedded-hal` | 2656 | 嵌入式硬件抽象层 |
| `owo-colors` | 810 | 零分配 no_std 终端着色 |
| `LibAFL` | 2646 | 模糊测试框架 |
| `esp-hal` | 2127 | ESP32 系列 HAL |
| `gdbstub` | 414 | 可嵌入的 GDB stub |
| `reqwless` | 289 | 嵌入式 HTTP 客户端 |
| `rust-mqtt` | 129 | MQTT 客户端 |
| `embedded-websocket` | 128 | WebSocket |
| `caches-rs` | 113 | 缓存结构 |
| `cobs-rs` | 12 | COBS 编码 |

> 检索方式：`gh api -X GET search/repositories -f q='no_std rust' -f sort=stars`

---

## 复现

```bash
cd D:/Code/rust/question/lilyco-nostd
python measure3.py     # 5 变体 × 默认/去PDB
python timings.py      # A/B 两组逐 crate 归因
```

`_body.rs` 是 B/C 共用的代码体，`diff lc-nostd/src/lib.rs lc-std/src/lib.rs` 应只出现 1 行差异。
