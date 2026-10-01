# 怎么解决「构建速度」问题

> 2026-10-01 · lilyco 实测
> 这份文档只讲**速度**，不讲体积。因为这两件事的杠杆**几乎不重叠**。

---

## 结论摘要

**我此前部署的那套配置，主体是「省磁盘」的刀，不是「提速」的刀。**
真正提速的只有一刀（`default-members`），已经落地。剩下的速度问题要用速度专属的杠杆。

| 问题 | 杠杆 | 实测 |
|---|---|---|
| **内环慢**（改一行等半天） | `default-members` + 用 `check` 不用 `build` + 留 `incremental` | 2.8 s / 4.9 s |
| **冷构建慢**（CI / 新机器） | `sccache`（≤1.84×） + 砍依赖数量 | 113.1 s → 61.5 s |
| **误以为慢** | 关掉了 `incremental`，或习惯性 `cargo build` | 16.1 s → 2.8 s |

---

## 一、先把「构建速度」拆成两个不同的东西

这是最容易混掉的一步。混着做，两边的收益会互相抵消。

| | 什么时候痛 | 有效的杠杆 | **无效 / 有害** |
|---|---|---|---|
| **内环** 改一行 → 看到结果 | 每天发生几百次 | `default-members`、`cargo check`、`incremental = true` | 依赖 `opt-level`（依赖不重编）、`/DEBUG:NONE` |
| **冷构建** CI / 新机器 / 第一次 clone | 每次 CI | `sccache`、**砍依赖数量** | 同上 |

> 关键机制：**依赖在内环里根本不重编**。所以任何只影响依赖编译的参数
> （`opt-level`、依赖的 `debug`、依赖的 `codegen-units`），
> 对内环速度的贡献是 **0**，只影响那一次冷构建。

---

## 二、内环三刀（收益最大）

### 第 1 刀：`default-members`（已落地，最大的一刀）

| | 包数 |
|---|---|
| 全部 24 成员 | 865 |
| 默认成员 19 个 | **379** |
| **省** | **486（56 %）** |

单刀省掉 56 % 的依赖解析与构建量。CI **完全不受影响** —— 已逐行确认 `ci.yml` 只用 `-p <crate>`。

⚠️ **副作用要知道**：`lilyco-graphite`（**231 包**）和 `lilyco-tauri`（**193 包**）
现在**不在**默认成员里。今天要改 graphite 的话，必须显式：

```bash
cargo check -p lilyco-graphite
```

### 第 2 刀：日常用 `cargo check`，不要用 `cargo build`

| | 冷构建 | 增量 |
|---|---|---|
| `cargo check` | **48.2 s** | **2.8 s** |
| `cargo build` | 90.1 s | 4.9 s |
| 倍数 | **1.87×** | **1.75×** |

原因：`check` 只生成 `rmeta`（接口元数据），不生成机器码，也不跑 codegen。
**改完一行先 `check`，只在真要跑二进制时才 `build`。**

这一刀零配置、零风险，但很多人的"慢"其实来自**习惯性 `cargo build`**。

### 第 3 刀：`incremental = true` 必须留着

`target/debug/incremental/` 占 **325 MB**（总 935 MB 的 35 %）——
看起来是浪费，但**内环 2.8 s 就是拿这块磁盘买的**。谁把它关了就回到每次 48 s。

三个项目对比：lilyco 325 MB（19 个成员）> Tauri 探针 62 MB。
成员越多，这块越大，也越值。

---

## 三、我自查的一个反向项：`codegen-units = 16`（**否定结果**）

我此前配了 `codegen-units = 16`，而 cargo 的 dev 默认是 **256**（为编译并行度）。
我怀疑 16 在拖慢冷构建，于是做了单变量对照（只改这一行）：

| 变体 | `cargo check` 冷 | `cargo build` 冷 | `target/` |
|---|---|---|---|
| A `codegen-units = 16`（我配的） | 48.2 s | **90.1 s** | **935.3 MB** |
| B `codegen-units = 256`（cargo 默认） | 46.0 s | 95.3 s | 948.3 MB |
| **差（256 − 16）** | −2.2 s | **+5.2 s** | **+13.0 MB** |

**怀疑被证伪：16 反而更好。**

- **`check` 的 −2.2 s 是噪声。** `check` 不生成机器码，`codegen-units` 对它
  没有机制上的影响。`check` 冷构建我历史上测到过 **46.0 / 48.2 / 53.6 s**，
  run-to-run 抖动约 **±7 %**，2.2 s（4.6 %）落在噪声里。
- **`build` 的 +5.2 s 和 `target/` 的 +13 MB 是确定性的**（体积测量不受计时抖动影响）。
  机制：`codegen-units` 越大 → CGU 之间**重复 monomorphize 泛型代码**越多
  → 编译更慢、产物更大。这正是 `codegen-units = 1` 运行性能最好、
  cargo 的 release 档默认用 16 的同一个原因。
- **结论：`codegen-units = 16` 保留不动。**
  这次自查的价值是**排除掉一个嫌疑犯**，而不是找到一把新刀。

> 附加验证：`[profile.dev] panic = "abort"` 是否会破坏 CI 的 `cargo test`？
> 实测 `cargo test -p lilyco-core` → **66 passed / 0 failed**。
> cargo 对 test profile 会强制 unwind，dev 档的 `panic = "abort"` 安全。

---

## 四、明确「不要为提速去做」的（这些是体积刀）

| 动作 | 省磁盘 | 对编译速度 |
|---|---|---|
| `/DEBUG:NONE` 关 PDB | 179 MB | ≈ 0 |
| `debug = false` | 82 MB | ≈ 0（链接略快，可忽略） |
| 去掉 `staticlib` / `cdylib` | 183 MB | 0 |
| `strip = "symbols"` | — | 0（只影响 strip 那一步） |
| `lto = "fat"` | ❌ 反而更大 | ❌ **大幅变慢** |

**如果你是为了速度去加这些，白忙一场，还可能更慢。**

### 唯一的例外：依赖 `opt-level = "z"` 既省磁盘又省时间

| 依赖 `opt-level` | 冷构建 | target/ |
|---|---|---|
| `0` | **120.1 s** | 1,157.3 MB |
| `1` | 201.1 s | 1,068.6 MB |
| `2` | **249.7 s** | 1,143.3 MB |
| **`"z"`** | 176.6 s | **1,013.9 MB** ✅ |

`"z"` 比 `1` **更快**（176.6 vs 201.1 s）**且小 55 MB** —— 因为 `z` 跳过了
向量化、循环展开这些昂贵 pass。**这条请留着**，它是唯一双赢项。

---

## 五、冷构建速度（CI）

唯一的大杠杆是 **`sccache`**，但期望值要调对：

```
无缓存冷构建      113.1 s
首次（写缓存）     121.2 s
全部命中           61.5 s   ← 1.84× 上限
```

**命中率只有 57.4 %**（hits 436 / misses 323）。
剩下 43 % 是 `build.rs` 输出和 proc-macro（`serde_derive` / `tauri-macros`）——
**这部分永远不可能命中**，是硬地板。所以 **1.84× 已经是天花板**。

### ⛔ 绝对不要写进 `.cargo/config.toml`

我踩过这个坑，而且**只有真实项目才能复现**：

```
sccache: encountered fatal error
sccache: caused by: 文件名或扩展名太长。 (os error 206)
error: could not compile `web-sys` (lib)
```

根因：`web-sys` 的 rustc 命令行约 **40,000 字符**
（rustc 1.80+ 把每一个 feature 都塞进 `--check-cfg`），
超过 Windows `CreateProcess` 的 **32,767** 上限；
sccache 作为 `rustc-wrapper` 是发起进程的一方，于是直接崩。

最小 Tauri 探针**没有 `web-sys`，复现不了** —— 这就是为什么"部署到真实项目并验证"是必需的一步。

**正确用法：opt-in，只在 CI 的 job 级 env 里开。**

```yaml
# .github/workflows/ci.yml
- name: Check
  env:
    RUSTC_WRAPPER: sccache
  run: cargo check -p lilyco-core -p lilyco-cli ...
```

另一个 CI 杠杆是**砍依赖数量** —— 这是唯一**线性**的杠杆。

---

## 六、行动清单（按性价比排序）

| # | 动作 | 状态 | 预期 |
|---|---|---|---|
| 1 | `default-members` | ✅ 已落地 | −56 % 包数 |
| 2 | 改掉习惯：日常 `cargo check` | ⬜ 待你改 | 1.87× / 1.75× |
| 3 | 保留 `incremental = true`、`codegen-units = 16`、依赖 `opt-level = "z"` | ✅ 已确认 | — |
| 4 | CI 给 `check` / `clippy` 加 `RUSTC_WRAPPER=sccache` | ⬜ 可做 | ≤1.84× 冷构建 |
| 5 | 动依赖树：`lilyco-graphite`(231) / `lilyco-tauri`(193) 是仅有的两个大户 | ⬜ 待定 | 线性 |

---

## 七、lilyco 现在到底什么水平

| 指标 | 实测 |
|---|---|
| 内环 `cargo check` | **2.8 s** |
| 内环 `cargo build` | **4.9 s** |
| 冷 `cargo check` | 48.2 s |
| 冷 `cargo build` | 90.1 s |
| `target/` | 935.3 MB（硬地板 470 MB） |
| `.pdb` | **0** |

**其实已经不慢了。** 如果主观上还觉得慢，先量一下**是 check 还是 build、是冷还是增量** ——
大概率痛点在"习惯性 `cargo build`"，而不是配置。

### 验证口径（别用错基线）

```bash
# ✅ 日常内环，正确的度量对象
cargo check            # 改了核心 crate 之后

# ⚠️ 全工作区构建在真实项目里往往不能当基线
cargo build --workspace
#   - lilyco: 缺 sidecar 二进制，build script 直接失败
#   - 带 sccache: web-sys 命令行超限崩掉
# 用 CI 实际跑的那组命令（只有 -p <crate>）来当基线。
```

> ⚠️ **测增量的陷阱**：`cargo check` 用 `rmeta`，`cargo build` 用 `rlib`，
> **两套产物不共享**。在 `cargo build` 冷构建（它会 `cargo clean`）之后
> 测 `cargo check` 增量，`rmeta` 已被清掉 → 测出 **16.1 s 假值**（真值 2.8 s，**差 5.7×**）。
> **两种 profile 的产物要各自预热。**
