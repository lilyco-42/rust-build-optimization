# lilyco 实测（首次真实测量）

> 2026-10-01 · Windows · rustc 1.98.1 · 工作区 24 成员 / 865 包
> 此前所有关于 lilyco 的数字都是从依赖图**推算**的，这里是**实测**。

---

## 一句话结论

**日常内环实测 `cargo check` 2.8s / `cargo build` 4.9s，`target/` 871.6 MB。**

> ⚠️ **口径修正（2026-10-01 晚）**：本文初版写的 `934.7 / 935.3 MB` 是**朴素累加**口径 ——
> 旧布局下 cargo 会把 `deps/` 的产物以**硬链接**放到 `target/debug/` 顶层，
> 按文件大小累加会把同一份数据算两遍。用 `(st_dev, st_ino)` 去重后真实值是 **871.6 MB**
> （虚高 63.7 MB / 6.8%）。详见 [`05-toolchain-cards.md`](05-toolchain-cards.md) 第三节。
加了 `default-members` 后，日常只编 19 个成员、252 个 crate —— 而不是全部 24 个成员、481 个 crate。

---

## 一、冷构建

从 `cargo clean` 开始，默认成员（19 个）。

| 命令 | 耗时 | 实际编译 crate 数 |
|---|---|---|
| `cargo check` | **53.6 s** | 64 |
| `cargo build` | **91.1 ~ 134.3 s** | 252 |

两点值得注意：

- **`cargo check` 只编 64 个 crate，`cargo build` 编 252 个**（多 3.9×）。
  因为 check 只需 rmeta（接口元数据），不需要机器码。**日常改完先跑 check。**
- `cargo build` 两次测得 91.1s 与 134.3s。前者带 sccache（可能命中了当天早先
  tauri-probe 遗留的共享缓存项），后者不带 —— **这不是干净对照**，别当成 1.5× 的收益。

## 二、增量构建（稳态，改一行 `lilyco-core/src/lib.rs`）

| 命令 | 耗时 |
|---|---|
| `cargo check` | **2.8 s** |
| `cargo build` | **4.9 s** |

> ⚠️ 测法说明：必须先让 check 和 build 各自的产物都到位，再各测两轮取中位数。
> 我第一版脚本犯了错 —— 在 `cargo build` 冷构建（它先 `cargo clean`）之后才测
> `cargo check` 增量，check 的 rmeta 已被清掉，于是测出 16.1s 的假值。
> **两种 profile 的产物不共享，测增量要各自预热。**

## 三、`target/` 构成（934.7 MB 朴素口径 / **871.6 MB 去重后**）

> ⚠️ 下表所有数字都是**朴素累加**口径，含硬链接重复计数，真实值约低 6.8%。比例关系仍然有效。

| 项 | 体积 | 说明 |
|---|---|---|
| `debug/deps/` | 522.8 MB | 含 rlib / rmeta / .o |
| `debug/incremental/` | **325.1 MB** | 35%，是 2.8s/4.9s 内环的代价 |
| `debug/build/` | 39.1 MB | build script 产物 |
| —— 其中 `.rlib` | 282.7 MB | 机器码 |
| —— 其中 `.rmeta` | 187.3 MB | 接口元数据 |
| —— 其中 `.o` | 26.7 MB | |
| —— 其中 `.lib` | 6.3 MB | 只剩这么点 → `staticlib`/`cdylib` 已去干净 |
| —— 其中 `.pdb` | **0** | ✅ `/DEBUG:NONE` 生效 |

**硬地板 = rlib 282.7 + rmeta 187.3 = 470 MB（朴素口径）。**

`incremental/` 占 325 MB 是三个项目里最大的（Tauri 探针只有 62 MB）——
因为 lilyco 有 19 个成员，每个都在产生增量产物。
**但这 325 MB 正是内环 2.8s 的原因，不要关。**

## 四、对照：全成员（没有 `default-members` 的情况）

| | 包数 |
|---|---|
| 全部 24 成员 | 865 |
| 默认成员 19 个 | **379** |
| 省 | **486（56%）** |

`lilyco-graphite` 单独就占 **231 包**，`lilyco-tauri` **193 包**。

---

## 🔴 五、本次踩到的两个坑（都会让 `cargo build --workspace` 失败）

### 坑 1：sccache 撞上 web-sys 的 Windows 命令行长限制（**我引入的**）

```
sccache: encountered fatal error
sccache: caused by: 文件名或扩展名太长。 (os error 206)
error: could not compile `web-sys` (lib)
```

- **根因**：`web-sys` 的 rustc 命令行长达约 **4 万字符**（rustc 1.80+ 会把全部
  feature 列表塞进 `--check-cfg`），超过 Windows `CreateProcess` 的 **32767** 上限。
  sccache 作为 `rustc-wrapper`，由它发起 `CreateProcess`，于是直接挂掉。
- **单变量确认**：只注释掉 `rustc-wrapper = "sccache"`（保留 `/DEBUG:NONE`），
  同一命令的 web-sys 错误立即消失。
- **这是已知问题**，搜「sccache web-sys MAX_PATH os error 206」有大量案例。
- **处置**：4 个项目的配置里 `rustc-wrapper` 全部改成**按需 opt-in**
  （注释掉 + 写明原因），需要时用 `RUSTC_WRAPPER=sccache cargo check`。
  理由：sccache 只在冷重建有用（且命中率上限约 57%），
  而日常增量循环用不到它 —— 为这点收益换一个「构建直接失败」的陷阱不值得。

### 坑 2：`cargo build --workspace` 需要先准备 lbin sidecar（**项目既有要求，不是我改的**）

```
resource path `binaries\lbin-x86_64-pc-windows-msvc.exe` doesn't exist
error: failed to run custom build command for `lilyco-tauri v0.3.0`
```

`lilyco-tauri/tauri.conf.json` 里声明了 `"externalBin": ["binaries/lbin"]`，
但该二进制要先构建再拷贝到 `lilyco-tauri/binaries/`。**`cargo build --workspace`
从来就跑不通**，必须先：

```bash
cargo build -p lilyco-binfmt --bin lbin
# 再把它拷成 lilyco-tauri/binaries/lbin-x86_64-pc-windows-msvc.exe
```

> 这正好说明 `default-members` 的价值：日常路径里 `lilyco-tauri` 根本不在编译集内，
> 上面两个坑都碰不到。

---

## 六、`codegen-units` 单变量（否定结果）

我怀疑自己配的 `codegen-units = 16` 拖慢了冷构建（cargo 的 dev 默认是 256），实测：

| 变体 | `check` 冷 | `build` 冷 | `target/` |
|---|---|---|---|
| A `codegen-units = 16`（部署值） | 48.2 s | **90.1 s** | **935.3 MB**（朴素口径） |
| B `codegen-units = 256`（cargo 默认） | 46.0 s | 95.3 s | 948.3 MB |

**怀疑被证伪，16 更优，保留。** 详见 [`03-build-speed.md`](03-build-speed.md)。

---

## 七、可复现

```bash
cd D:/Code/lilyco

cargo clean && cargo check     # ~48s
cargo clean && cargo build     # ~90s
du -sm target                  # 935 MB（朴素）/ 872 MB（硬链接去重后）

# 增量（注意先各自预热！）
#   改一行 lilyco-core/src/lib.rs 后：
cargo check                    # 2.8s
cargo build                    # 4.9s

# codegen-units 对照（会自己还原 Cargo.toml）
python _cgu_test.py            # 约 5 分钟，结果写 _cgu.json
```

> 📐 **crate 计数的口径**：`cargo check` 输出的 `Checking xxx` **行数约 188**，
> 但其中包含同一 crate 因 feature 组合不同而被重复检查的次数；
> 去重后的 crate 数约 **64**。两个数都对，看你要什么口径 —— **跨变体比较时用同一个口径即可**。
> 同理 `cargo build` 的 `Compiling` 行数为 252（这个基本等于去重后的数量）。

---

## 八、关于「构建速度」的独立文档

本文只记录**测量事实**。要把速度作为目标去优化，见
[`03-build-speed.md`](03-build-speed.md) —— 那里区分了**内环速度**与**冷构建速度**，
并列出哪些刀对速度**零收益**（`/DEBUG:NONE`、`debug = false`、去掉 `staticlib`…）。

还想再快一步的话，[`05-toolchain-cards.md`](05-toolchain-cards.md) 记录了
lld / 并行前端 / **cranelift** 三张牌的实测结果（两台无效、cranelift 有条件可用），
以及本轮踩到的**跨工具链 target 目录污染**与**硬链接体积重复计数**两个测量陷阱。
