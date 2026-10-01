# rust-build-optimization

**Rust 编译速度与产物体积优化** —— 全部结论基于同机实测，每一条都有原始数据和可复现脚本。

需求背景：**debug 要快 + 产物要小 + release 丢 CI**（硬盘紧张，单个项目 target 就 4 GB）。

---

## 结论速查

### 先看方法论

如果你要**优化一个新项目**，先读 [`docs/01-methodology.md`](docs/01-methodology.md) ——
它讲的是「怎么想」：先分清你优化的是**编译量**还是**编译产物**，
再顺着五层阶梯往下走。这套方法比任何具体配置都更可迁移。

### 如果你的痛点是「**构建太慢**」

直接看 [`docs/03-build-speed.md`](docs/03-build-speed.md)。核心：**速度和体积的杠杆几乎不重叠**，
而本文档里的刀（`/DEBUG:NONE`、`debug = false`、去 `staticlib`）**对速度零收益**。
速度专属的杠杆只有三个：`default-members`、用 `cargo check` 代替 `cargo build`、留 `incremental = true`。

### 如果你还想再快一步 / 想跟 C(xmake) 比

- [`docs/05-toolchain-cards.md`](docs/05-toolchain-cards.md) —— lld / 并行前端 / **cranelift** 三张牌的实测。
  **两台无效，cranelift 有条件可用（冷 build −31%），但它把冷 check 拖慢了 9 秒。**
  另含两个测量陷阱：跨工具链 target 目录污染、硬链接体积重复计数。
- [`docs/04-rust-vs-xmake-speed.md`](docs/04-rust-vs-xmake-speed.md) —— Rust vs C/xmake 三层对标。
  **可比规模上 Rust 全面超越**（冷 0.36×、增量 0.59×、no-op 0.29×），
  但真实项目的绝对冷构建赢不了 —— 那是**编译量**的差异，不是**编译器速度**的差异。

### 如果你只想抄配置

| 场景 | 看这里 |
|---|---|
| Tauri 桌面项目，`target/` 压到 1 GB 以内 | [`configs/Cargo.toml.1GB-config`](configs/Cargo.toml.1GB-config) + [`configs/dot-cargo-config.toml`](configs/dot-cargo-config.toml) |
| 一次性给多个真实项目应用 | `python scripts/apply_tauri_opt.py`（带备份、幂等） |
| 先体检谁在吃硬盘 | `python scripts/tauri_disk_audit.py D:\Code`（只读不删） |

### 最重要的七个数字

| 指标 | 结果 |
|---|---|
| 最小 Tauri 项目的依赖数 | **291 个 crate**（业务代码规模无关） |
| 默认 debug 的 `target/` | **4,135 MB** |
| 优化后（日常档 / 极限档） | **1,014 MB / 953 MB**（−77%） |
| 体积硬地板 | **844 MB**（rlib 512 + rmeta 332，不可删） |
| 冷构建代价 | +78 s（**一次性**，依赖不重编） |
| 增量构建代价 | 8.9 s → 12.3 s（+3.4 s） |
| 零依赖 `no_std` 裸程序 release | **7,168 B**（比 C 的 13,824 B 还小一半） |

### 五个反直觉但实测成立的结论

1. **`opt-level = "z"` 比 `opt-level = 1` 更快，同时更小。**
   176.6 s vs 201.1 s，1,014 MB vs 1,069 MB。`z` 跳过向量化/循环展开等昂贵 pass。
   「体积优化必然更慢」是错的。

2. **`opt-level = 2` 是双输**：最慢（249.7 s）又不小（1,143 MB）。

3. **`/DEBUG:NONE` 是唯一决定性的一刀，省 179 MB。**
   且——即使 `[profile.dev] debug = false`，**rustc 仍会给 MSVC 传 `/DEBUG`，PDB 照产**，
   必须显式关链接器。

4. **`[profile.dev.build-override]` 基本无效**（实测 1,132 → 1,135 MB，反而略增）。

5. **`~/.cargo/config.toml` 里的 `[profile.*]` 完全无效**，
   多 crate 工作区的 `.cargo/config.toml` 只放子 crate 也会**静默失效**（从工作区根构建时读不到）。

### 两个「否定结果」（省得你重复走）

6. **`no_std` 不提速也不瘦身。** 同源代码只差一行 `#![no_std]` → 冷构建
   7673 vs 7786 ms（−1.4 %，噪声级），**exe 字节数完全相同**。它是可移植性特性，不是性能特性。

7. **自己把 `codegen-units` 从 cargo 默认 256 改成 16 不是错，反而更好。**
   lilyco 实测 `build` 冷 90.1 s vs 95.3 s、`target/` 935.3 vs 948.3 MB。
   （`check` 那 2.2 s 差是噪声 —— `check` 不跑 codegen。）**别去"修"它。**

### 三张"再快一点"的牌（实测，两台无效）

8. **lld 链接器 / 并行前端都没用。** lld 让 `build` 从 93.9 → 95.7 s（噪声），
   并行前端 −1.4 s（噪声）——前者分母太小（91 s 里链接只占几秒），
   后者 cargo 本来就跨 crate 并行了。

9. **cranelift 是唯一有肉的：冷 `build` 91.5 → 63.5 s（−31%）、增量 `build` −17%。**
   但 **冷 `check` 反而 +9.3 s（变慢 20%）**，`target/` +7%，且**不支持 fat LTO**（dev-only）。
   ⛔ 千万别写进 `Cargo.toml` —— **stable cargo 会直接报 manifest 解析失败，CI 全挂**。

10. **`target/` 的真实值要比"朴素累加"低 6.8%** —— cargo 会把 rlib 以**硬链接**放到
    `target/debug/` 顶层，遍历累加会把同一份算两遍。lilyco：935.3 → **871.6 MB**。

---

### 在真实工作区上的第一刀：`default-members`

```toml
# 工作区根 Cargo.toml
[workspace]
members = [ ... 24 个 ... ]
default-members = [ ... 19 个 ... ]   # ← 加这一行
```

| | 包数 |
|---|---|
| 全部 24 成员 | 865 |
| 默认成员 19 个 | **379** |
| **省** | **486（56 %）** |

CI **完全不受影响**（它只用 `-p <crate>`）。真实项目实测见 [`docs/02-lilyco-measurements.md`](docs/02-lilyco-measurements.md)。

---

## 仓库结构

```
.
├── docs/
│   ├── 01-methodology.md                 ★★ 优化思路：分层决策（先看这个）
│   ├── 03-build-speed.md                 ★★ 只讲「构建速度」怎么解（痛点在这里就看这个）
│   ├── 05-toolchain-cards.md             ★★ 还想再快？lld/并行前端/cranelift 实测 + 两个测量陷阱
│   ├── 04-rust-vs-xmake-speed.md         ★ Rust vs C(xmake) 三层对标：到底做到了没有
│   ├── 00-experiment-log.md              ★ 全部实验台账 E1~E11（再看这个）
│   ├── 02-lilyco-measurements.md          在真实 865 包工作区上的实测（含三个踩坑）
│   ├── XMAKE_VS_RUST_PIPELINE.md          R1 对标 xmake：汇编层 + 全流程
│   ├── TAURI_DEV_EFFICIENCY_PLAN.md       R2 Tauri 开发效率方案
│   ├── TAURI_1GB_ACHIEVED.md              R3 达成 1 GB 实录（含 opt-level 标定）
│   ├── RUST_NOSTD_VERDICT.md              R4 no_std 到底值不值
│   ├── NO_STD_LIBS.md                     R5 no_std 库替代清单
│   ├── NOSTD_LIB_REPLACEMENT_VERDICT.md   R6 库替换结论
│   ├── CAN_MATCH_C.md                     R7 能否在体积上对标 C
│   ├── TUI_NOSTD_CORRECTION.md            R8 一次误判的纠正
│   └── RUST_FAST_SMALL.md                 R9 速查
├── configs/
│   ├── Cargo.toml.1GB-config              最终定稿配置（含标定数据注释）
│   ├── Cargo.toml.tauri-optimized         中间版本（保留对照）
│   └── dot-cargo-config.toml              .cargo/config.toml 内容
├── scripts/
│   ├── apply_tauri_opt.py                 批量应用到真实项目（幂等 + 备份）
│   ├── tauri_disk_audit.py                磁盘体检（只读）
│   ├── measure_all.py / measure_final.py / measure_one.py   E1 的测量脚本
│   └── _archive.py                        生成本仓库的归档脚本
└── experiments/
    ├── tauri-probe/                       E2~E8 的测量工程（含全部 _bench*.json）
    ├── lilyco-measure/                    E9 真实工作区测量（measure.py + _cgu_test.py）
    ├── ip-nostd/ ip-bare/ ip-bare-gnu/    E1 的零依赖裸程序（Rust）
    ├── ip-c/ ip-c-msvc/ ip-go/            E1 的对照实现（C / Go）
    ├── ip-base/ ip-size/ ip-speed/ ip-tiny/ ip-xr/ ip-lib/ ip-std/
    ├── lilyco-nostd/                      no_std 化真实项目的试验
    ├── _probe/ _offsets/                  中间分析产物
    └── final.json                         E1 汇总数据
```

---

## 复现方法

### 测 Tauri

```bash
cd experiments/tauri-probe
cargo clean
python _bench_opt.py        # 依赖 opt-level 五组标定（会自己禁/恢复 sccache）
```

`_bench*.py` 全程用 `cargo clean` 而非删目录，计时用 `time.perf_counter()`，
并且会剥掉 `BASH_ENV` 以免 profile 干扰。

### 验证体积构成

```bash
du -sm target
find target -name "*.rlib"  -printf "%s\n" | awk '{s+=$1} END {printf "rlib:  %.1f MB\n", s/1048576}'
find target -name "*.rmeta" -printf "%s\n" | awk '{s+=$1} END {printf "rmeta: %.1f MB\n", s/1048576}'
find target -name "*.pdb"   -printf "%s\n" | awk '{s+=$1} END {printf "pdb:   %.1f MB\n", s/1048576}'
```

---

## 三个容易踩的坑

| 坑 | 事实 |
|---|---|
| `sccache --set-max-cache-size 2G` | ❌ sccache 0.18 已移除该子命令。**唯一有效：`SCCACHE_CACHE_SIZE=2G` 环境变量**（配置文件它不读） |
| **把 `rustc-wrapper = "sccache"` 写进默认配置** | ❌ **`cargo build --workspace` 会直接失败** —— web-sys 的 rustc 命令行约 4 万字符，超 Windows `CreateProcess` 的 32767 上限，sccache 作为 wrapper 发起调用就挂。改用 `RUSTC_WRAPPER=sccache cargo check` 按需开 |
| 把 `[profile.*]` 写进全局 config | ❌ 完全无效，cargo 只在 workspace 根 `Cargo.toml` 读 |
| 删 `staticlib` / `cdylib` 后再跑移动端 | ⚠️ 要改回 `["staticlib", "cdylib", "rlib"]`，否则 `tauri android init` / `ios init` 会挂 |
| 把 `cargo build --workspace` 当基线 | ⚠️ 很多真实工作区本来就不通（缺 sidecar 二进制、sccache 撞长命令行）。用 CI 实际使用的那组命令测 |

---

## 已知代价（务必知悉）

- **失去断点调试能力**：`debug = false` + `/DEBUG:NONE` 之后，WinDbg/VS 里没法设断点、看变量。
  保留的：`dbg!()` 输出（`file:line` 是编译期字面量，不受影响）、`println!`、panic 回溯的函数名。
- **需要调试时**：`debug = "line-tables-only"` 并注释掉 `/DEBUG:NONE`，体积回到约 1.13 GB。
- **Tauri 探针的 953 MB 是"最小项目"的数字**；真实项目请以实测为准 ——
  lilyco（24 成员 / 865 包）的 19 成员日常档实测 **871.6 MB**（硬链接去重后；
  朴素累加口径 935.3 MB），已在真实工作区验证过（见 [`docs/02-lilyco-measurements.md`](docs/02-lilyco-measurements.md)）。
  依赖数量是主导因素，业务代码影响很小。

---

*实验日期 2026-09-30 ~ 2026-10-01 · Windows · rustc 1.98.1 · Tauri 2.12.1*
