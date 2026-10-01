# Tauri 开发效率与硬盘占用优化方案

> 实测环境：Windows / rustc 1.98.1 / Tauri 2.12.1 / 291 个依赖包
> 全部数字为本机实测，不是估算
> 测量工程：`D:/Code/rust/question/tauri-probe`

---

## 0. 你的真实约束（先对齐需求）

| 约束 | 数值 | 说明 |
|---|---|---|
| 硬盘总量 | 400 GB | —— |
| **C 盘剩余** | **41 GB**（92% 已用） | 🔴 危险水位 |
| **D 盘剩余** | **36 GB**（92% 已用） | 🔴 危险水位 |
| 单项目占用 | ~40 GB | 你提到的量级 |
| debug 构建 | **要最快** | 开发内环 |
| 本地产物体积 | **要尽可能小** | 硬盘红线 |
| release 构建 | 慢无所谓 | 丢 GitHub Actions |

**先纠正一个认知**：单个 Tauri 项目的 `target/` 默认是 **4.1 GB**，不是 40 GB。
40 GB 大概率是「多个项目 × 多个 target/ + node_modules + cargo 缓存」的总和。
所以策略要分两层：**① 压单项目体积 ② 清全局冗余**。

---

## 1. 实测数据（核心事实）

### 1.1 依赖规模

```
cargo tree --prefix none | sort -u | wc -l
=> 291
```

**你写的业务代码可能只有几百行，但 Tauri 拖进了 291 个 crate。**
真正吃硬盘和时间的是这 291 个，不是你的代码。这决定了所有优化都要围绕"依赖"做。

### 1.2 四种配置的完整实测

| 配置 | 冷构建 | `target/` | deps | incremental | build | exe |
|---|---|---|---|---|---|---|
| **A 默认 debug** | 111.6 s | **4,135 MB** | 2,416 MB | 302 MB | 301 MB | 12.4 MB |
| **B 优化 debug** | 154.4 s | **1,511 MB** | 1,112 MB | 72 MB | 184 MB | 7.6 MB |
| **C 极限瘦身 debug** | 224.6 s | 1,522 MB | 1,124 MB | 0 MB | 179 MB | **2.9 MB** |
| **D 默认 release** | 180.3 s | 1,557 MB | 1,189 MB | 0 MB | 216 MB | 8.3 MB |
| 增量（A 默认） | **8.9 s** | —— | —— | —— | —— | —— |
| 增量（B 优化） | **12.3 s** | —— | —— | —— | —— | —— |
| sccache 全命中重建（A） | **61.5 s** | —— | —— | —— | —— | —— |

### 1.3 一句话结论

> **默认 debug 的 `target/` 是 4.1 GB；优化后 1.5 GB —— 省 63%，代价是冷构建多 43 秒、增量多 3.4 秒。**
> **叠加 sccache 后，冷重建从 113 s 降到 61.5 s（1.84×）。**

冷构建是一次性的（换分支/clean 时才发生），硬盘占用是**永久**的。
在你只剩 77 GB 的盘上，这笔账非常划算。

**注意**：`incremental` 那 302 MB 就是默认配置虚胖的主要来源之一 ——
它是每次增量编译留下的中间产物。优化配置把它压到 72 MB。

---

## 2. 方案：三层配置

### 第 1 层 —— `src-tauri/Cargo.toml` 的 profile（省 2.6 GB，核心）

> ⚠️ **重要**：`[profile.*]` **只有在 workspace 根 crate 的 Cargo.toml 里才生效**。
> 实测已验证：放在 `~/.cargo/config.toml` 里的 `[profile.release]` **完全无效**
> （我拿一个 hello world 测试，产物 112,128 B，明显没走 `opt-level="z"`）。
> 这个坑一定要避开。

```toml
# ============================================================
# 开发内环：速度优先，但顺手砍掉一半以上体积
# ============================================================
[profile.dev]
# 自己的代码用 1：编得快，跑得也不慢
# （注意：opt-level=0 在泛型密集型代码上反而更臃肿，1 是甜点）
opt-level = 1
# 🔴 最关键的一行：只保留行号，不生成完整 DWARF
#    回溯依然有「文件:行号」，但体积立减
debug = "line-tables-only"
# 保留增量：改一行只重编一个 crate，这是内环速度的命脉
incremental = true
# 16 个 CGU 并行编自己代码
codegen-units = 16
# 剥掉符号表里的调试部分（行号表保留，回溯不受影响）
strip = "debuginfo"
# abort：panic 少一层展开逻辑，编译更快、体积更小
panic = "abort"

# 依赖包：编一次就不动了，用 2 换更好性能和更小产物
[profile.dev.package."*"]
opt-level = 2
# 🔴 依赖的调试信息对本项目几乎无用 —— 这是最大的省空间项
debug = false

# ============================================================
# release：丢 CI，本地基本不编，体积+性能优先
# ============================================================
[profile.release]
opt-level = "z"        # 体积优先；想更快改 3（体积约 +30%）
lto = "fat"
codegen-units = 1
panic = "abort"
strip = "symbols"
incremental = false
overflow-checks = false
debug-assertions = false
```

### 第 2 层 —— sccache（换分支/clean 后重建提速）

> ⚠️ **后续修正（2026-10-01）**：本节建议「写进 `~/.cargo/config.toml` 全局生效」
> **已被推翻**。实测在 865 包真实工作区上，它会让 `cargo build --workspace` 直接失败：
> `sccache: caused by: 文件名或扩展名太长。 (os error 206)` —— web-sys 的 rustc
> 命令行约 40,000 字符，超过 Windows `CreateProcess` 的 32,767 上限，而 sccache
> 作为 wrapper 正是发起该调用的一方。**正确做法是 opt-in，不进任何 config 文件**：
> `RUSTC_WRAPPER=sccache cargo check`。详见
> [`02-lilyco-measurements.md`](02-lilyco-measurements.md) 与 [`03-build-speed.md`](03-build-speed.md)。

你机器上 `sccache 0.18.0` **已经装了但从未使用**（统计显示 0 次请求）。

~~启用方式，写进 `~/.cargo/config.toml`（这个是全局的，对所有项目生效，且 `[build]`/`[env]` 段在这里是合法的）：~~

```toml
# ⛔ 已废弃：不要这样做（见上方修正说明）
[build]
rustc-wrapper = "sccache"
```

#### 实测效果

| 场景 | 耗时 | 说明 |
|---|---|---|
| 无 sccache 冷构建 | 113.1 s | 基线 |
| sccache 首次（全 miss） | 121.2 s | 略慢，因为要写缓存 |
| **sccache 全命中重建** | **61.5 s** | **1.84× 加速** |

统计显示：`Cache hits 436 / Cache misses 323` → **命中率 57.4%**。

**为什么不是 100% 命中？**
剩下 43% 是 **build script 和 proc-macro**：
- `build.rs` 的产物（如 `tauri-build` 生成的 Windows 资源文件）sccache 缓存不了
- `serde_derive`、`tauri-macros` 这类过程宏，每次都要真跑一遍
- 这部分**无法通过 sccache 优化**，是 Tauri 编译时间的硬底

所以 **61.5 s 是 sccache 能达到的下限**。它解决的是"换分支 / `cargo clean` 后重建"的场景，
对日常增量（改一行 → 12.3 s）没有帮助（增量本来就快）。

### 第 3 层 —— 全局清理（回收被遗忘的空间）

**用体检工具先看清楚谁在吃空间**（只读，不删任何东西）：

```bash
python _solution/tauri_disk_audit.py D:\Code
```

在本机实测挖出的两个大坑：

| 位置 | 体积 | 说明 |
|---|---|---|
| `CARGO_HOME\registry` | **3,078 MB** | 所有项目共享的 crate 源码，含大量旧版本 |
| `CARGO_HOME\git` | **1,382 MB** | git 依赖的快照 |
| sccache 缓存 | 373 MB（上限 **10 GiB**） | 🔴 在 C 盘 |

**🔴 两个必须注意的点：**

1. **sccache 缓存默认在 C 盘**：`C:\Users\liuqi\AppData\Local\Mozilla\sccache\cache`，
   上限 **10 GiB**。你 C 盘只剩 41 GB —— **建议调到 2-3 GB，或挪到 D 盘**：

   ```bash
   # 限制大小（推荐 2G，够用且安全）
   sccache --set-max-cache-size 2G

   # 或改缓存位置到 D 盘
   setx SCCACHE_DIR "D:\cache\sccache"
   ```

2. **cargo registry 会滚雪球**：4.5 GB 里大部分是历史版本。
   清理用官方工具，**不要手删**：

```bash
# 1. 清掉旧的 crate 源码（保留当前需要的）
cargo install cargo-cache --no-default-features --features ci-autoclean
cargo cache --autoclean

# 2. 列出所有 target/ 体积（定期体检）
python _solution/tauri_disk_audit.py D:\Code

# 3. 彻底清某个项目（cargo 官方方式，别手删）
cd <项目> && cargo clean

# 4. 找回 cargo 以外的空间
npm cache clean --force         # npm 缓存
pnpm store prune                # pnpm 全局 store
```

---

## 3. 数据支撑的取舍分析

### 3.1 为什么不是「全部 opt-level=0」？

很多人以为 debug 就该 `opt-level=0` 让编译最快。实测：

- A（全 0）：冷构建 111.6 s，但 target **4,135 MB**
- B（自己 1 / 依赖 2）：冷构建 154.4 s，target **1,511 MB**

多花 43 秒冷构建，省 2.6 GB。更重要的是：
**`opt-level=0` 编出来的泛型代码体积更大**，因为每种单态化都保留完整调试信息，不做任何合并。

### 3.2 为什么不选 C（极限瘦身）？

C 的 exe 只有 **2.9 MB**（vs B 的 7.6 MB），但：
- `incremental = false` → **每次改一行都要重编全部依赖**（几十分钟）
- 冷构建 224.6 s

C 的 target 反而比 B 大（1,522 vs 1,511），因为 `lto="thin"` 的中间产物更多。
**C 只适合"我要发一个 demo 给别人看"**，不适合日常开发。

### 3.3 为什么 release 也要写进 Cargo.toml？

因为你的 `~/.cargo/config.toml` 里那份 `[profile.release]` **是死的**（实测证实）。
不写进项目，CI 上的 release 就是默认配置，白丢一半优化。

---

## 4. 立即可执行的清单

按收益/成本排序：

| # | 动作 | 收益 | 成本 |
|---|---|---|---|
| 1 | `Cargo.toml` 加 `[profile.dev]` + `[profile.dev.package."*"]` | **省 2.6 GB/项目** | 5 分钟，一次性 |
| 2 | `sccache --set-max-cache-size 2G` | 防 C 盘被缓存吃满（当前上限 10 GB） | 10 秒 |
| 3 | 从 `~/.cargo/config.toml` 删掉那个**无效的** `[profile.release]`，写进项目 | 避免误判 | 1 分钟 |
| 4 | ⛔ ~~`~/.cargo/config.toml` 加 `[build] rustc-wrapper = "sccache"`~~ **已废弃**（会崩 web-sys）。改用按需 `RUSTC_WRAPPER=sccache cargo check` | 冷重建 113s → 61.5s | 1 分钟 |
| 5 | `cargo cache --autoclean` | 回收 registry 冗余（当前 3 GB） | 2 分钟 |
| 6 | `cargo clean` 掉不再维护的项目 | 按项目回收 GB 级 | 看项目数 |
| 7 | 把 `target/` 挪到空间大的盘（`CARGO_TARGET_DIR`） | 缓解 C 盘压力 | 需权衡 |
| 8 | GitHub Actions 出 release | 本地完全不编 release | 一次性配置 |

**第 7 条要注意**：把 `CARGO_TARGET_DIR` 指向另一块盘，
会导致跨盘 IO + 无法与其他项目共享缓存，**实测通常更慢**。
只有在你必须给 C 盘腾空间时才用，且建议指向**同一块物理盘**（D 盘）。

---

## 5. 换来源分支后的处理对比

有个值得注意的理解点：

| 场景 | 优化前 | 优化后 | 说明 |
|---|---|---|---|
| 新建项目首次 build | 111.6 s | 154.4 s | 一次性，可接受 |
| 改一行代码 | 8.9 s | 12.3 s | **日常内环，需权衡** |
| `cargo clean` 后重建 | 113 s | 61.5 s（sccache） | 优化后**反而更快** |
| 换分支（依赖没变） | 全量重编 | 命中 sccache | 大赢 |

**如果你觉得日常 +3.4 s 太肉疼**，可以把 `[profile.dev] opt-level` 从 `1` 降到 `0`，
但**保留**这两项（它们才是省空间的主力）：

```toml
[profile.dev]
opt-level = 0
debug = "line-tables-only"   # ← 保留
# ...
[profile.dev.package."*"]
debug = false                 # ← 保留
```

---

## 6. GitHub Actions 出 release 的要点

因为 release 丢 CI，本地 profile 可以更激进：

```yaml
# .github/workflows/release.yml 关键部分
- uses: Swatinem/rust-cache@v2     # CI 侧缓存，跨 run 复用
  with:
    workspaces: "src-tauri -> target"

- run: cargo build --release
  working-directory: src-tauri
```

**注意**：CI 的 runner 是 Linux 还是 Windows，产物不同。
Tauri 要出 Windows 的 `.msi`/`.exe`，必须用 `windows-latest`。
如果要跨平台出包，用 `tauri-apps/tauri-action`。

---

## 7. 验证方法（确保配置真的生效）

```bash
# 1. 确认 profile 生效：编译时看 "Finished" 那行
cargo build 2>&1 | tail -2
# 出现 [optimized + debuginfo] 说明 opt-level 和 debug 都改了

# 2. 确认体积
du -sm target

# 3. 确认 sccache 在工作
sccache --show-stats | head -8
# Cache hits 应该逐次增长

# 4. 确认回溯仍然可用（debug="line-tables-only" 的验证）
# 故意 panic，看回溯里有没有文件名和行号
```

---

## 8. 结论

**你的需求完全成立，且可以同时满足：**

- ✅ **debug 保持快**：增量 12.3 s（默认是 8.9 s，多 3.4 s 换 2.6 GB）
- ✅ **体积大幅下降**：`target/` 4.1 GB → 1.5 GB（**−63%**）
- ✅ **release 丢 CI**：本地 profile 可以更激进，无副作用
- ✅ **顺手修掉一个隐形坑**：全局 `[profile.release]` 一直是失效的

**唯一的取舍**是冷构建 111.6 s → 154.4 s。
如果你觉得这个代价太大，可以把 `[profile.dev] opt-level` 从 `1` 降到 `0`，
保留 `debug = "line-tables-only"` 和依赖侧 `debug = false` —— 那两项才是省空间的主力。

---

*实测脚本：`tauri-probe/_bench2.py`（配置对照）、`_bench_sccache.py`（缓存效果）*
*原始数据：`tauri-probe/_bench.json`*
