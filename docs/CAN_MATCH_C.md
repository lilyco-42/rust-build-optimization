# 能不能做到「xmake C 那样快、那样小」？

> 三方同口径实测（N=3 取最小），2026-10-01
> 机器：Windows 11 / MSVC 14.51 / rustc 1.98.1-stable / xmake 3.1 / 同一台机器、同一份功能

---

## 先给答案

**能，但有分档，而且每档的代价完全不同。**

| 目标 | 能不能 | 代价 |
|---|---|---|
| 编译**更快** | ✅ 能，而且轻松 | 零成本，`-C debuginfo=0 /DEBUG:NONE` 一行 |
| 体积**一样小/更小** | ✅ 能 | 要付**代码复杂度**，907 行 vs 256 行 |
| 用**常规 Rust 写法**就一样小 | ❌ **不能** | 差 9.7 倍，这是 std 的固有成本 |

一句话：**速度和体积不在同一个难度档上。速度是免费的，体积要用手写换。**

---

## 实测数据

### dev/debug 档（日常开发循环，最该关心的）

| 方案 | cold ms | incr ms | exe B | target B | 源码行数 |
|---|---:|---:|---:|---:|---:|
| **C / xmake** | 919 | 501 | 13,824 | 54,207 | 256 |
| Rust 常规（std，零依赖）| **388** | **313** | 185,344 | 373,012 | 86 |
| Rust 极限（no_std，零依赖）| **360** | **275** | **18,432** | **41,529** | 907 |

### release 档（交付体积）

| 方案 | exe B |
|---|---:|
| C / xmake release | 13,824 |
| Rust 常规 release（std）| 134,144 |
| **Rust 极限 release（no_std）** | **7,168** |

### 相对 C 的倍数（<1 表示比 C 好）

| 方案 | cold | incr | exe(dev) | target |
|---|---:|---:|---:|---:|
| C / xmake | 1.00x | 1.00x | 1.00x | 1.00x |
| Rust 常规（std）| **0.42x** | **0.62x** | 13.41x | 6.88x |
| Rust 极限（no_std）| **0.39x** | **0.55x** | **1.33x** | **0.77x** |

release 体积：Rust 极限 **0.52x**（比 C 小一半），Rust 常规 **9.70x**。

---

## 怎么读这组数据

### 1. 编译速度：Rust 本来就赢，而且不是勉强赢

- 常规 Rust 写法（86 行，零依赖，用 std）：cold **388 ms**，是 C/xmake 的 **0.42 倍**。
- 极限 Rust 写法：cold **360 ms**，0.39 倍。
- 两者差别很小（388 vs 360）—— 说明**速度这条主要靠"零依赖 + 关 PDB"，不靠 no_std**。

**关键**：这里赢的前提是**依赖为零**。上一轮实测过，只要引入 `serde` derive，
proc-macro 链（syn + serde_derive）会占 45.9% CPU 且铺在串行路径上，cold 立刻涨到 7.7 s。

> **所以"Rust 编译慢"的真相是："带 proc-macro 的 Rust"编译慢，不是"Rust"慢。**

### 2. 体积：这条才是真分水岭

| 写法 | release exe | 相对 C |
|---|---:|---:|
| C | 13,824 | 1.00x |
| **常规 Rust（std）** | 134,144 | **9.70x** ← 差一个数量级 |
| **极限 Rust（no_std）** | 7,168 | **0.52x** ← 反超 |

**常规 Rust 写不出 C 的体积。** 差 9.7 倍不是调参数能救的 —— 是 std 自带的东西：

- 格式化机器（`core::fmt`，含 float/escape_debug 表）
- panic 基础设施 + unwind 元数据
- 每个 bounds check 保留的 `panic_bounds_check` 调用点
- std 的启动代码

**极限 Rust 能反超 C（7,168 vs 13,824），靠的是三件事：**

1. **零依赖** —— 没有 format!/Vec/String 被静态链进来
2. **去掉全部 panic 调用点** —— 全改 `get_unchecked`/`from_raw_parts`，`-16,384 B`（23,552 → 7,168）
3. **关 PDB** —— `target/` 直接小 8 倍（零代码改动）

### 3. 代价：体积是用代码复杂度换的

| 方案 | 行数 | release exe |
|---|---:|---:|
| C | 256 | 13,824 |
| Rust 常规 | 86 | 134,144 |
| **Rust 极限** | **907** | **7,168** |

**907 - 86 = 821 行，全是手写的基础设施**：

```
unsafe fn b_at / slice_u / split_host_port / to_cstr   ← 绕过 bounds check
#[no_mangle] memset / memcpy / memcmp / strlen         ← 自己实现 CRT（VS 14.51 不再导出）
#[no_mangle] __CxxFrameHandler3                        ← dev 档 SEH 桩
fn panic(...) -> !                                      ← 自己写 panic handler
#[no_mangle] mainCRTStartup                             ← 自己写入口
```

**这才是"体积达到 C 水平"的真实账单**：不是配置问题，是**把 Rust 当 C 写**。

---

## 结论：分场景给建议

### 场景 A：大多数项目 → 追求速度，不追求极限体积

```toml
# .cargo/config.toml
[build]
rustflags = ["-C", "debuginfo=0", "-C", "link-args=/DEBUG:NONE"]
```

- **收益**：`target/` 立省 53%（lilyco-core 实测 124 MB → 58 MB），零代码改动
- **效果**：cold 已经是 C 的 0.42 倍
- **体积**：接受 exe 大一些（几 MB 级），对桌面/服务端**完全无所谓**
- **别做**：为了省 100 KB 去写 `unsafe` —— 不值

### 场景 B：嵌入式 / WASM / 极端分发 → 追求极限体积

- 用 `no_std` + `heapless`/`smoltix` 等（不是自己写）
- **但其实**：如果只是要小，`opt-level="z" + lto="fat" + panic="abort" + strip` 就能到 100 KB 以下，多数嵌入式场景够用
- **只有到 10 KB 以下**才需要那 821 行手写
- **更聪明的做法**：真要 <20 KB，考虑直接用 C 写那个模块 —— 但会失去 Rust 的安全性

### 场景 C：lilyco 这类项目（多 crate、有 serde/clap）

**答案是：速度能达成，体积别追。**

| 指标 | 能达成吗 | 做法 |
|---|---|---|
| 编译速度接近 C | ✅ | 关 PDB + 砍 proc-macro 依赖（上一轮实测 −48%）|
| `target/` 不膨胀 | ✅ | 关 PDB，−53% |
| 二进制体积接近 C | ⚠️ 部分 | CLI 工具能到 1-3 MB 量级；做不到 14 KB，**也不该做** |

lilyco 是 CLI/TUI 工具框架，不是嵌入式固件。**14 KB 是有意义的嵌入式约束，不是 CLI 的约束。**
花 800 行 unsafe 把 CLI 从 1 MB 压到 100 KB，收益是负的。

---

## 一句话总结

> **速度：Rust 已经赢了（0.42x），代价是一行配置。**
> **体积：要赢就得把 Rust 当 C 写（907 行 vs 256 行），只在嵌入式值得。**
> **"对标 C"这个目标本身要拆开 —— 编译速度该对标，二进制体积不该。**

---

## 复现

```bash
cd D:/Code/rust/question
python measure_final.py     # 三方同口径，N=3
```

项目：`ip-c/`（C/xmake）、`ip-std/`（常规 Rust）、`ip-bare/`（极限 Rust），三者功能对齐。
