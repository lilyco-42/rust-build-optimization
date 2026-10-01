# 实验台账 — Rust 编译速度与产物体积优化

> 全部数据来自 **同一台机器、同一 rustc、同一天**，可复现。
> 环境：Windows / rustc 1.98.1 / cargo 1.98.1 / nightly 1.100.0 ·
> MSVC 14.51.36231 · Windows SDK 10.0.26100.0 · LLVM 22.1.8 · zig 0.16.0
> 日期：2026-09-30 ~ 2026-10-01

原始 JSON 在 `experiments/tauri-probe/_bench*.json`，脚本是同目录的 `_bench*.py`。

---

## 目录

| 实验 | 主题 | 数据 |
|---|---|---|
| [E1](#e1) | 裸程序：Rust vs C vs Go，汇编层与全流程 | `final.json` |
| [E2](#e2) | Tauri 四配置基准 | `_bench.json` |
| [E3](#e3) | 冲 1 GB · 第一波（四刀） | `_bench_1g.json` |
| [E4](#e4) | 冲 1 GB · 第二波（三个失败思路） | `_bench_1g2.json` |
| [E5](#e5) | 冲 1 GB · 第三波（依赖 opt-level 初测） | `_bench_1g3.json` |
| [E6](#e6) | 冲 1 GB · 第四波（达成 953 MB） | `_bench_1g4.json` |
| [E7](#e7) | sccache 真实收益 | `_bench_sccache.json` `_sccache_result.json` |
| [E8](#e8) | 依赖 opt-level 五组标定（终测） | `_bench_opt.json` |
| [E9](#e9) | lilyco 首次真实测量 · `default-members` · `codegen-units` 单变量 | `_cgu.json` |

---

<a id="e1"></a>
## E1 · 裸程序：Rust vs C vs Go

同一个「读端口/IP 工具」分别用三种语言、零依赖实现，测冷构建、增量构建、产物大小。

### 开发档（debug）

| 实现 | 冷构建 | 增量构建 | exe | target/ |
|---|---|---|---|---|
| **C / xmake** | 918.8 ms | 501.4 ms | 13,824 B | 54,207 B |
| **Rust 常规**（std，零依赖） | **387.9 ms** | **313.0 ms** | 185,344 B | 373,012 B |
| **Rust 极限**（`no_std`，零依赖） | **359.6 ms** | **274.6 ms** | 18,432 B | **41,529 B** |

**Rust 冷构建比 C/xmake 快 2.4×，target 小 24%。**

### release 产物

| 实现 | exe |
|---|---|
| C（xmake） | 13,824 B |
| Rust `std` | 134,144 B |
| **Rust `no_std`** | **7,168 B** ✅ |

### 工具链交叉对比（零依赖 no_std 版为基准）

| 工具链 | 产物 | 备注 |
|---|---|---|
| **Rust + MSVC（`no_std` + `no_main`）** | **7,168 B** | 3 个 DLL 导入 |
| C `clang-cl /MD` | 12,288 B | |
| C `cl.exe /MT` | 143,872 B | |
| C `clang-cl /MT` | **152,576 B** | ⚠️ **`/O1 /O2 /Oz /-flto` 全部恒定不变** |
| C `zig cc`（gnu） | 192,000 B | `-nostartfiles` / `-Wl,-e,` 改不动 |
| C `zig cc`（msvc） | 566,784 B | |
| Rust `cargo zigbuild`（windows-gnu） | 78,336 B / 1,826 ms | ⚠️ 比 MSVC 版大 11×，**且更慢** |

**结论**：Windows 上追求「最小体积极速构建」，**Rust + MSVC 是唯一正解**；
`cargo-zigbuild` 是反方向的（`windows-gnu` 必然拖进 6 个 `api-ms-win-crt-*` 转发 DLL）。
`clang-cl` 的 `/MT` 会拉进整个 `libcmt.lib`，没有 `/OPT:REF` 级别的抽取，**比 `cl.exe` 还大 6%**。

### 为什么 C 慢：预处理膨胀 480×

C 的慢不是编译后端慢，而是 **`#include` 展开量爆炸** —— 实测某翻译单元预处理后
膨胀约 480×。Rust 没有这个问题（模块是编译期直接解析的）。

---

<a id="e2"></a>
## E2 · Tauri 四配置基准

最小 Tauri 2.12.1 项目，**291 个依赖**。`cargo clean` 后冷构建。

| 配置 | 冷构建 | target/ | deps | incremental | build/ | exe |
|---|---|---|---|---|---|---|
| A1 默认 debug | 111.6 s | **4,135.1 MB** | 2,416.4 | 302.1 | 300.7 | 12,399 KB |
| A2 默认 debug · 增量 | **8.9 s** | — | — | — | — | — |
| B1 优化 debug | 154.4 s | 1,511.3 MB | 1,111.8 | 71.8 | 183.9 | 7,588 KB |
| B2 优化 debug · 增量 | 12.3 s | — | — | — | — | — |
| C1 极限瘦身 debug | 224.6 s | 1,522.0 MB | 1,124.4 | 0 | 178.7 | **2,936 KB** |
| D1 默认 release | 180.3 s | 1,556.6 MB | 1,188.5 | 0 | 216.0 | 8,295 KB |

**要点**
- 默认 debug 的 `target/` 是 **4.1 GB**；业务代码规模与此无关，全是依赖。
- 优化后 **1,511 MB（−63%）**，代价是冷构建 +42.8 s（一次性）、增量 +3.4 s。
- `C1` 的 exe 只有 2,936 KB，但 `target/` 反而更大（1,522 MB）—— **`dev-lean` 破坏增量编译**，
  省的是 exe 不是硬盘。要硬盘就别用。
- release 的 `target/` 并不比 debug 小（1,557 MB），**因为它同样要编 291 个依赖**。

---

<a id="e3"></a>
## E3 · 冲 1 GB · 第一波

从 `B1`（1,511 MB 配置）出发，逐刀叠加。**每刀单独测，互相不叠加解释。**

| # | 配置 | 冷构建 | target/ | incremental | build/ | pdb | rlib | rmeta | `.lib` |
|---|---|---|---|---|---|---|---|---|---|
| 0 | 基线 | 212.1 s | 1,573.5 MB | 101.7 | 184.2 | 190.0 | 585.5 | 369.6 | **206.7** |
| 1 | 去掉 `staticlib`/`cdylib` | 96.8 s | 1,390.7 MB | 101.7 | 184.2 | 183.8 | 585.5 | 369.9 | **30.1** |
| 2 | + `debug = false` | 92.3 s | 1,309.2 MB | 62.7 | 184.2 | 165.9 | 560.9 | 369.9 | 30.1 |
| 3 | + 关增量编译 | 92.7 s | 1,244.9 MB | **0** | 184.2 | 165.8 | 559.4 | 369.5 | 30.1 |

**`.lib` 从 206.7 → 30.1 MB**：`staticlib` 和 `cdylib` 各产一份，与 `rlib` **硬链接到同一 inode**，
内容完全相同，只多占目录项和 PDB。桌面端不用这两个 type，Tauri 模板却默认带上。

---

<a id="e4"></a>
## E4 · 冲 1 GB · 第二波（三个失败思路）

从 1,245 MB 出发，试三条捷径 —— **全部失败，记录以免重走**。

| 思路 | 冷构建 | target/ | 结论 |
|---|---|---|---|
| A 基线 | 74.4 s | 1,244.9 MB | — |
| B 减少 `windows` crate feature | 181.2 s | 1,235.5 MB | ❌ 只省 **9.4 MB**，构建却慢 2.4× |
| C 只编 `--lib` | 107.9 s | 1,225.7 MB | ❌ 省 19 MB 是因为 **压根没编 bin**，不算优化 |

---

<a id="e5"></a>
## E5 · 冲 1 GB · 第三波（依赖 opt-level 初测）

| 依赖 opt-level | 冷构建 | target/ | rlib |
|---|---|---|---|
| `2`（基线） | 129.5 s | 1,244.9 MB | 559.4 |
| **`"z"`** | 205.0 s | **1,132.0 MB** | **510.5** |
| `1` | 216.8 s | 1,170.9 MB | 528.5 |

**`"z"` 一举省 112.9 MB。**

---

<a id="e6"></a>
## E6 · 冲 1 GB · 第四波（达成）

| 配置 | 冷构建 | target/ | build/ | pdb | 结论 |
|---|---|---|---|---|---|
| A 基线（依赖 `opt=z`） | 58.7 s | 1,132.0 MB | 183.6 | 180.6 | |
| B `+ [profile.dev.build-override] opt-level="z"` | 207.7 s | 1,134.6 MB | 183.7 | 181.6 | ❌ **无效，反而 +2.6 MB** |
| **C `+ MSVC /DEBUG:NONE`** | 189.6 s | **953.0 MB** ✅ | **61.7** | **0** | ✅✅ 达标 |

**`build-override` 对 build script 产物基本无效**（实测变差）。真正的杠杆在链接器层：
`/DEBUG:NONE` 让 `.pdb` **181 → 0 MB**，`build/` 连带 **184 → 62 MB**。

> ⚠️ 即使 `[profile.dev] debug = false`，**rustc 仍会给 MSVC 传 `/DEBUG`，PDB 照产**。
> 必须显式加 `link-args=/DEBUG:NONE`。

---

<a id="e7"></a>
## E7 · sccache 真实收益

| 场景 | 耗时 |
|---|---|
| 无 sccache 冷构建 | 113.1 s |
| sccache 首次（全部 miss） | 121.2 s |
| **sccache 全命中重建** | **61.5 s（1.84×）** |

| 指标 | 值 |
|---|---|
| Cache hits | 436 |
| Cache misses | 323 |
| **命中率** | **57.4 %** |

**剩下 43 % 是硬底**，sccache 结构上缓存不了：
- `build.rs` 产物（如 `tauri-build` 生成的 Windows 资源）
- 过程宏（`serde_derive`、`tauri-macros`）

**61.5 s 就是这台机器上加 sccache 之后的构建下限。**

---

<a id="e8"></a>
## E8 · 依赖 opt-level 五组标定（终测）

⚠️ **本组是唯一禁用了 sccache 的纯净对比**，也是最终结论的来源。
自己代码固定 `opt-level = 1`，变量只有依赖的 opt-level。

| # | 依赖 opt-level | 冷构建 | target/ | rlib | rmeta |
|---|---|---|---|---|---|
| A | `0` | **120.1 s** | 1,157.3 MB | 632.0 | 322.9 |
| B | `1` | 201.1 s | 1,068.6 MB | 529.8 | 332.1 |
| C | `2` | **249.7 s** | 1,143.3 MB | 560.9 | 369.9 |
| **D** | **`"z"`** | 176.6 s | **1,013.9 MB** ✅ | **511.8** | 332.0 |
| E | 混合（`windows*` 用 `z`，其余 `0`） | 120.2 s | 1,150.5 MB | 621.4 | 326.8 |

### 三个反直觉结论

**① `opt-level="z"` 比 `opt-level=1` 更快（176.6 s vs 201.1 s），同时小 55 MB。**
`z` 跳过向量化、循环展开等昂贵 pass，编译反而更快。
**「体积优化必然更慢」这个直觉是错的。**

**② `opt-level=2` 是双输**：最慢（249.7 s）又不小（1,143 MB）。
它是「运行性能最优」，但编译开销高、内联膨胀。

**③ 混合方案无效。** 只给 `windows*` 单独设 `z`、其余用 `0` → **1,150.5 MB**，
几乎等于全用 `0`（1,157.3 MB）。**要体积就必须全局 `z`。**

### 为什么选 `z` 而不是 `0`

`opt-level` 只影响**依赖**，而依赖在**增量构建时根本不重编**
（依赖不变 → 不重新编译 → 这个耗时日常感知不到）。
所以 `z` 相对 `0` 多出的 ~56 s **只发生在冷构建（一次性）**，
而省下的 **143 MB 是永久的**。

---

## 最终配置与地板

| 档位 | `incremental` | target/ | 日常内环 |
|---|---|---|---|
| **日常档** | `true` | **1,013.9 MB** | 快 |
| **极限档** | `false` | **953.0 MB** | 慢（改一行重编整个 crate） |

**硬地板 844 MB** = `rlib` 511.8 + `rmeta` 332.0 —— 304 个依赖的编译产物本身，
删了就没法增量编译，**任何优化都动不了这块**。

完整配置见 `configs/Cargo.toml.1GB-config`。

---

## 一句话总账

**4,135 MB → 953 MB（−77%），冷构建 +78 s（一次性），增量 +3.4 s，代价是失去断点调试。**

五刀的贡献（独立测量）：

| 动作 | 省 |
|---|---|
| 去掉 `staticlib` + `cdylib` | 183 MB |
| `/DEBUG:NONE` 关 PDB | 179 MB |
| 依赖 `opt-level = "z"` | 113 MB |
| `debug = false` | 82 MB |
| 关增量编译 | 64 MB |
| **合计** | **621 MB** |

（另有 ~2,600 MB 来自基础 debug 配置）

---

<a id="e9"></a>
## E9 · lilyco 首次真实测量 + `codegen-units` 单变量

> 前面 E1–E8 全部在 **Tauri 最小探针**上做。E9 是第一次在**真实产品工作区**上量。
> 数据：`experiments/lilyco-measure/` · 完整报告见 [`02-lilyco-measurements.md`](02-lilyco-measurements.md)
> 与 [`03-build-speed.md`](03-build-speed.md)

### E9.1 `default-members`（工作区第一刀）

| | 包数 |
|---|---|
| 全部 24 成员 | 865 |
| 默认成员 19 个 | **379** |
| **省** | **486（56 %）** |

`lilyco-graphite` 单独 **231 包**、`lilyco-tauri` **193 包**。
CI 逐行确认只用 `-p <crate>`，**不受 `default-members` 影响**。

### E9.2 lilyco 冷 / 增量基线

| 命令 | 冷构建 | 增量（稳态） |
|---|---|---|
| `cargo check` | **48.2 s**（另测 53.6 s） | **2.8 s** |
| `cargo build` | **90.1 s**（另测 91.1 / 134.3 s） | **4.9 s** |

`target/` **935.3 MB**（`deps` 522.8 + `incremental` 325.1 + `build` 39.1），
硬地板 **470 MB**（`rlib` 282.7 + `rmeta` 187.3），`.pdb` = **0**。

### E9.3 `codegen-units` 单变量（否定结果）

只改 `[profile.dev] codegen-units` 一行：

| 变体 | `check` 冷 | `build` 冷 | `target/` |
|---|---|---|---|
| A `codegen-units = 16`（部署值） | 48.2 s | **90.1 s** | **935.3 MB** |
| B `codegen-units = 256`（cargo dev 默认） | 46.0 s | 95.3 s | 948.3 MB |
| **差（256 − 16）** | −2.2 s | **+5.2 s** | **+13.0 MB** |

**结论：怀疑被证伪，16 更优，保留不动。**

- `check` 的 −2.2 s 是**噪声**：`check` 不跑 codegen，`codegen-units` 对其无机制影响。
  `check` 冷构建跨次测量 **46.0 / 48.2 / 53.6 s**，抖动 **±7 %**，2.2 s 在其中。
- `build` 的 +5.2 s 与 `target/` 的 **+13 MB 是确定性的**：CGU 越多
  → 泛型 monomorphize 在 CGU 间重复越多 → 更慢更大。
  （与"`codegen-units = 1` 运行性能最好"是同一机制。）

### E9.4 附加验证：dev 档 `panic = "abort"` 是否破坏测试

`cargo test -p lilyco-core` → **66 passed / 0 failed**。
cargo 对 test profile 强制 unwind，dev 档的 `panic = "abort"` **安全**。
（`cargo test --no-run` 输出 `Finished \`test\` profile [optimized]`，
无 `+debuginfo` → 确认 `opt-level = 1` + `debug = false` 已生效。）

### E9.5 本次暴露的两个"假故障"与一个测量陷阱

| 现象 | 真因 | 处置 |
|---|---|---|
| `cargo build --workspace` 崩 `os error 206 文件名或扩展名太长` | `web-sys` rustc 命令行 ~40,000 字符 > Windows `CreateProcess` 32,767；sccache 作为 wrapper 发进程 | 从 `.cargo/config.toml` 摘掉 `rustc-wrapper`，改 opt-in |
| `cargo build --workspace` 报 `resource path 'binaries\lbin-...exe' doesn't exist` | `tauri.conf.json` 声明了 `externalBin`，sidecar 需先构建拷贝 | **不是配置问题**，该命令在真实项目里本就不曾可用 |
| `cargo check` 增量测出 16.1 s 假值 | check 用 `rmeta`、build 用 `rlib`，两套产物不共享；在 build 冷构建（含 `cargo clean`）之后测 check 增量 | 两种 profile **各自预热**后重测 → 真值 2.8 s（**差 5.7×**） |
