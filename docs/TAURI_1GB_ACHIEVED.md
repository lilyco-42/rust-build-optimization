# 达成 1 GB：Tauri 项目体积优化实录

> 目标：`target/` ≤ 1 GB
> 实测环境：Windows / rustc 1.98.1 / Tauri 2.12.1 / 291 个依赖
> 结果：**4,135 MB → 953 MB（−77%）**

---

## 最终结果

| 阶段 | target | 关键动作 |
|---|---|---|
| 起点 | **4,135 MB** | cargo 出厂默认 |
| 上轮方案 | 1,511 MB | `debug="line-tables-only"` + 依赖 `debug=false` |
| 第 1 刀 | 1,391 MB | 去掉 `staticlib` / `cdylib` |
| 第 2 刀 | 1,245 MB | `debug=false` + `incremental=false` |
| 第 3 刀 | 1,132 MB | 依赖 `opt-level="z"` |
| **第 4 刀** | **953 MB** ✅ | **`/DEBUG:NONE` 关 PDB** |

**总计降幅 77%（省 3.18 GB）。**

---

## 完整配置

### 1. `src-tauri/Cargo.toml`

```toml
# ============================================================
# 关键：去掉 staticlib / cdylib
# 它们是给移动端用的，桌面端完全不需要
# 实测：留着会多产出 176 MB 的 .lib（同样的内容存两份）
# ============================================================
[lib]
name = "your_app_lib"
crate-type = ["rlib"]        # ← 原来是 ["staticlib", "cdylib", "rlib"]

# ============================================================
# 开发内环
# ============================================================
[profile.dev]
opt-level = 1
debug = false                # 完全不要 DWARF（要调试时改回 "line-tables-only"）
incremental = false          # ← 关掉！省 100 MB，代价见下文
codegen-units = 16
strip = "debuginfo"
panic = "abort"

# ============================================================
# 🔴 依赖侧：opt-level="z" 是压缩 rlib 的关键
# 实测：opt-level="z" 时 rlib 510 MB，opt-level=2 时 559 MB
# ============================================================
[profile.dev.package."*"]
opt-level = "z"
debug = false

# ============================================================
# release：丢 CI
# ============================================================
[profile.release]
opt-level = "z"
lto = "fat"
codegen-units = 1
panic = "abort"
strip = "symbols"
incremental = false
overflow-checks = false
debug-assertions = false
```

### 2. `src-tauri/.cargo/config.toml`

> ⚠️ **后续修正（2026-10-01）**：下面这段里的 `rustc-wrapper = "sccache"` **不要再照抄**。
> 它会让含 `web-sys` 的项目 `cargo build --workspace` 直接失败
> （`os error 206 文件名或扩展名太长`：web-sys 命令行 ~40,000 字符 > Windows
> `CreateProcess` 的 32,767 上限）。正确用法是 opt-in：
> `RUSTC_WRAPPER=sccache cargo check`。详见
> [`02-lilyco-measurements.md`](02-lilyco-measurements.md) 与 [`03-build-speed.md`](03-build-speed.md)。
> **权威版本看 [`configs/dot-cargo-config.toml`](../configs/dot-cargo-config.toml)。**

```toml
# 🔴 决定性的一刀：省 179 MB
# /DEBUG:NONE 让 MSVC 链接器不生成 PDB
[target.x86_64-pc-windows-msvc]
rustflags = ["-C", "link-args=/DEBUG:NONE"]

# sccache：换分支/clean 后重建提速（113s → 61.5s）
# ⛔ 已废弃的写法，见上方修正说明 —— 不要照抄这行
[build]
rustc-wrapper = "sccache"
```

---

## 953 MB 的构成

| 项 | 体积 | 说明 |
|---|---|---|
| `.rlib` | 512 MB | 304 个依赖的机器码 — **不可删** |
| `.rmeta` | 332 MB | 依赖接口元数据 — **不可删** |
| `build/` | 62 MB | build script 产物 |
| `.pdb` | **0 MB** | ✅ 已被 `/DEBUG:NONE` 干掉 |

**844 MB 是硬地板**（rlib + rmeta）。这就是 304 个依赖的编译产物本身，
删了就没法增量编译，任何优化都动不了它。

---

## 补充实验：依赖 `opt-level` 该设多少（已禁用 sccache，纯净对比）

问题：依赖的 `opt-level` 直接决定「编译速度」和「体积」的取舍 ——
这是唯一真正的对立面。测了 5 种配置（自己代码固定 `opt-level=1`）：

| 依赖 `opt-level` | 冷构建 | `target/` | `.rlib` | 备注 |
|---|---|---|---|---|
| `0` | **120.1s** | 1,157 MB | 632 MB | 最快，但最占地方 |
| `1` | 201.1s | 1,069 MB | 530 MB | |
| `2` | 249.7s | 1,143 MB | 561 MB | 最慢 |
| **`"z"`** | 176.6s | **1,014 MB** ✅ | 512 MB | **体积最小，且比 1/2 更快** |
| 混合（windows 用 z、其余 0） | 120.2s | 1,150 MB | 621 MB | ❌ 几乎无收益 |

### 三个反直觉结论

**① `opt-level="z"` 比 `opt-level=1` 更快（176.6s vs 201.1s），同时小 55 MB。**
原因：`z` 会跳过向量化、循环展开等昂贵 pass，编译反而更快。
「体积优化必然更慢」这个直觉是错的。

**② `opt-level=2` 是最差选择** —— 既最慢（249.7s）又不小（1,143 MB）。
它是"运行性能最优"，但编译开销高、代码内联膨胀。

**③ 混合方案（只给 windows 单独设 z）完全无效。**
windows 系列虽然是大头（206 MB rlib），但单独优化它只从 632 降到 621 MB，
总量 1,150 MB —— 几乎等于全用 `0`。
**要体积就得全局 `opt-level="z"`。**

### 为什么最终选 `z` 而不是 `0`

`opt-level` 只影响**依赖**。而依赖在**增量构建时根本不重编** ——
所以它带来的 +56 秒只发生在冷构建（一次性），
但省下的 143 MB 是**永久**的。

---

## 两档配置，按你的取舍选

| 档位 | `target/` | 日常增量 | 说明 |
|---|---|---|---|
| **日常档**（当前已应用） | **1,014 MB** | 快（保留增量） | `incremental = true` |
| **极限档** | **953 MB** | 慢（改一行重编整个 crate） | `incremental = false` |

两档只差一行：

```toml
[profile.dev]
incremental = true    # 改 false 立刻省 61 MB，代价是日常内环变慢
```

按 1 GiB = 1,024 MiB 计，**两档都达标**；按 1 GB = 1,000 MB 计，只有极限档达标。

---

## 必须知道的代价

### 1. `incremental = false` 的代价最大

关掉增量编译省 100 MB，但**改一行代码要重编整个 crate**。

对 Tauri 来说影响相对可控（因为依赖不重编），
但你自己那个 lib crate 每次都要全量重编。

**实测增量构建：8.9s → 12.3s**（那是保留 incremental 的数据）；
完全关掉后会更慢。**如果你更看重内环速度，把 `incremental = true` 加回来，体积回到约 1.05 GB。**

### 2. `debug = false` + `/DEBUG:NONE` = 无法断点调试

- **保留的**：panic 回溯仍能显示函数名（符号来自 rlib）
- **失去的**：WinDbg / VS 里设置断点、查看变量

**需要调试时**，把两处改回来：
```toml
debug = "line-tables-only"    # Cargo.toml
# 注释掉 .cargo/config.toml 里的 /DEBUG:NONE
```
体积回到约 1.13 GB，仍然 < 1.2 GB。

### 3. `opt-level="z"` 编依赖 = 冷构建变慢

依赖用 `z` 比 `2` 慢，实测冷构建 **129.5s → 205.0s**。
但只在依赖变动时发生，日常不感知。

---

## 三档配置，按场景选

| 档位 | target | 增量速度 | 可断点调试 | 适用 |
|---|---|---|---|---|
| **冲 1GB 档** | **953 MB** | 慢 | ❌ | 硬盘最紧、调试靠 `println!`/日志 |
| **平衡档** | ~1.13 GB | 中 | ❌ | 日常开发（`incremental=true`，`debug=false`） |
| **可调试档** | ~1.15 GB | 快 | ✅ | 需要断点调试时（`debug="line-tables-only"`） |

**平衡档就是把上面配置里的 `incremental` 改回 `true`。**

---

## 每刀的实际收益（可复现）

| 动作 | 省 |
|---|---|
| 去掉 `staticlib` + `cdylib` | **183 MB** |
| `debug` 完全关闭 | 82 MB |
| 关增量编译 | 64 MB |
| 依赖 `opt-level="z"` | **113 MB** |
| `/DEBUG:NONE` | **179 MB** |
| **合计** | **~620 MB** |

（另有 ~1,940 MB 来自上一轮的 `debug="line-tables-only"` 等基础配置）

---

## 复现方法

```bash
# 1. 应用配置后，从零构建
cargo clean
cargo build

# 2. 验证 profile 生效
cargo build 2>&1 | tail -2
# 应显示 [optimized]（没有 +debuginfo，说明 debug=false 生效）

# 3. 测体积
du -sm target/debug

# 4. 拆分构成
find target/debug -name "*.rlib"  -printf "%s\n" | awk '{s+=$1} END {printf "rlib:  %.1f MB\n", s/1048576}'
find target/debug -name "*.rmeta" -printf "%s\n" | awk '{s+=$1} END {printf "rmeta: %.1f MB\n", s/1048576}'
find target/debug -name "*.pdb"   -printf "%s\n" | awk '{s+=$1} END {printf "pdb:   %.1f MB\n", s/1048576}'
```

---

## 附：关于 `%TEMP%` 的澄清（纠正一次误判）

排查过程中 `du` 报 `%TEMP%` 占 **39.5 GB**（其中 RDP 诊断日志 `DiagOutputDir` 22.7 GB），
同一时刻 `df` 显示 C 盘只剩 21 GB。我据此怀疑是工具读数虚高。

**后来确认：不是工具的问题，是用户在此期间自己清理了 Temp。**

PowerShell 复核时看到的是清理后的状态（TEMP 0.5 GB、C 盘剩 66.4 GB）。
事后再次交叉验证，`du` 与 PowerShell 对同一目录读数一致
（627 MB vs 510 MB，差值来自目录项开销）。

**结论**：Windows 上 `du` 基本可信。
排查磁盘时用 PowerShell 交叉验证仍是好习惯（`df` 与 `Get-PSDrive` 口径可能不同），
但不要轻易把差异归因于工具 bug —— 先确认数据是不是被别的东西改过了。

---

*实验脚本：`tauri-probe/_bench_1g.py` ~ `_bench_1g4.py`*
*原始数据：`_bench_1g.json` ~ `_bench_1g4.json`*
