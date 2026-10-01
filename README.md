# rust-build-optimization

**Rust 编译速度与产物体积优化** —— 全部结论基于同机实测，每一条都有原始数据和可复现脚本。

需求背景：**debug 要快 + 产物要小 + release 丢 CI**（硬盘紧张，单个项目 target 就 4 GB）。

---

## 结论速查

### 先看方法论

如果你要**优化一个新项目**，先读 [`docs/01-methodology.md`](docs/01-methodology.md) ——
它讲的是「怎么想」：先分清你优化的是**编译量**还是**编译产物**，
再顺着五层阶梯往下走。这套方法比任何具体配置都更可迁移。

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

---

## 仓库结构

```
.
├── docs/
│   ├── 01-methodology.md                 ★★ 优化思路：分层决策（先看这个）
│   ├── 00-experiment-log.md              ★ 全部实验台账（再看这个）
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
| 把 `[profile.*]` 写进全局 config | ❌ 完全无效，cargo 只在 workspace 根 `Cargo.toml` 读 |
| 删 `staticlib` / `cdylib` 后再跑移动端 | ⚠️ 要改回 `["staticlib", "cdylib", "rlib"]`，否则 `tauri android init` / `ios init` 会挂 |

---

## 已知代价（务必知悉）

- **失去断点调试能力**：`debug = false` + `/DEBUG:NONE` 之后，WinDbg/VS 里没法设断点、看变量。
  保留的：`dbg!()` 输出（`file:line` 是编译期字面量，不受影响）、`println!`、panic 回溯的函数名。
- **需要调试时**：`debug = "line-tables-only"` 并注释掉 `/DEBUG:NONE`，体积回到约 1.13 GB。
- **没有在真实业务代码上验证过**：`953 MB` 是最小 Tauri 探针项目的结果；
  依赖集不同的项目数字会变（依赖数量是主导因素，业务代码影响很小）。

---

*实验日期 2026-09-30 ~ 2026-10-01 · Windows · rustc 1.98.1 · Tauri 2.12.1*
