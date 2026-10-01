# cargo-xmake

`cargo` 的子命令，兼容 cargo 的全部用法（`cargo new` / `run` / `build` / `test` …），
额外给你两样东西：

1. **dev 档像 C/xmake 一样快、一样小** —— 默认打满「关调试信息 / 关 overflow check /
   关 debug_assert / 关 PDB」这一套；
2. **泛型热点排序 + 去泛型改写** —— dev 走 `&dyn Trait`（1 份单态化），
   release 自动切回 `<T: Trait>`（内联 + 去虚化，满血性能）。

零依赖，`cargo install --path .` 大约 1 秒。

---

## 快速开始

```bash
# 1. 安装（得到 cargo-xmake.exe，于是 `cargo xmake` 就能用了）
cargo install --path cargo-xmake

# 2. 在项目里写入 xmake 档剖面设置（只碰 .cargo/config.toml）
cargo xmake setup

# 3. 之后照常敲 cargo —— 只是顺带享受设置
cargo xmake build
cargo xmake run
cargo xmake test

# 看看他到底会做什么
cargo xmake doctor

# 找出"被复制了 N 份"的泛型热点
cargo xmake audit

# 预览改写建议（不落盘）
cargo xmake dynify
# 落盘 + 自动编译验证 + 失败自动回滚
cargo xmake dynify --rewrite

# 【真想提速，先跑这个】workspace 上算每个成员独占多少包，推荐 default-members
cargo xmake slim
# 自动挑掉独占最多的成员，试算能省多少（只读）
cargo xmake slim --auto
# 确认后写进工作区根 Cargo.toml（带标记，undo 可逆）
cargo xmake slim --drop=lilyco-graphite,lilyco-tauri --apply

# 后悔：配置删掉、源码折叠回泛型原样、slim 那行也一起删（逐字节还原）
cargo xmake undo
```

`cargo xmake build` 和 `cargo build` **共用同一份缓存和同一份指纹**，
可以随意混用，不会互相触发全量重编。

改完想验一下整条链路没坏，跑 `bash scripts/smoke_cargo_xmake.sh`：
它会建一个临时工程，走完 `setup → dynify --rewrite → build → undo → 再 undo`，
断言「源码逐字节还原」「二次 undo 幂等」「release 零代价」「dev 产物确实更小」等 27 项。

---

## 每一刀实际值多少

### 剖面设置（`cargo xmake setup`）

| 设置 | 值 | 解决什么 |
|---|---|---|
| `debug` | `false` | 不生成 DWARF。dev 产物体积的绝对大头 |
| `strip` | `"debuginfo"` | 双保险，链接期再剥一次 |
| `incremental` | `true` | 内环速度的命根子 |
| `opt-level` | `0` | 不跑 LLVM 优化 pass，codegen 最快 |
| `debug-assertions` | `true` ※ | 见下方「一个刻意的耦合」 |
| `overflow-checks` | `false` | C 的默认就是静默回绕 |
| `codegen-units` | `16` | cargo dev 默认 256；16 让产物更小更可复现 |
| `[target.*] rustflags` | `-C link-args=/DEBUG:NONE` | MSVC 上 rustc **即使 `debug=false` 也照传 `/DEBUG`**，PDB 照生成。实测 Tauri 项目这一刀省 179 MB |

三档激进度：`--xmk-tier=safe`（只砍调试信息，语义零改动）/ `fast`（默认）/
`extreme`（再加 `panic=abort`）。

### release 剖面

`opt-level=3` · `lto="fat"` · `codegen-units=1` · `panic="abort"` · `strip="symbols"`。
**这里不需要任何「关虚函数」设置** —— 静态派发本来就是 Rust 的默认，
cargo-xmake 在 release 只做一件事：把优化开关推到顶。

### 去泛型（`cargo xmake dynify`）

单变量对照实验（N=200 个具体类型，只改一行签名）：

| | LLVM `define` 总数 | IR 字节 | `drive` 副本 |
|---|---|---|---|
| `fn drive<T: Work>(t: &T, n: u64)` | 416 | 994,153 | **200** |
| `fn drive(t: &dyn Work, n: u64)` | 217 | 310,661 | **1** |

端到端实测（三份**独立干净副本**，只跑 `build` / `build --release`）：

| | dev 单态化 | `target/` | dev exe | release exe | release 单态化 |
|---|---|---|---|---|---|
| 原样 | 416 | 5,042,395 B | 229,888 B | 135,168 B ※ | 416 |
| 只 `setup` | 420 | 4,062,616 B | 230,912 B | 143,872 B | 406 |
| `setup` + `dynify` | **218（−47.6%）** | **3,716,406 B** | **177,664 B（−22.7%）** | **143,872 B** | **406** |

※ 原样的 release 更小，是因为它吃到了用户**全局** `~/.cargo/config.toml` 里的
`opt-level="z"` —— 见下面「用户全局配置是隐形的」。不是我们的功劳。

**`dynify` 后 release 的四项数字与只 `setup` 时逐项相同**，`drive` 恢复成 200 份
—— 也就是 **「dev 开虚函数、release 关」真的做到了**，因为 release 编的是另一个 `cfg` 分支。

⚠️ 顺带一个必须记住的对比：**只 `setup` 时 dev 单态化是 420 份，比原样的 416 还多 4 份。**
`setup` 本身**不减少单态化**（它改的是调试信息 / opt-level / 链接参数）；
减少单态化的只有 `dynify`。把两者混为一谈会得出错误的因果。

---

---

## 🔴🔴 先减编译量，再谈别的：`cargo xmake slim`

**如果你的目标是"让 cargo 跑得比 xmake 快"，这一节是整个工具里最该看的一段。**

`dynify` 改的是「泛型被复制成多少份」，但真实项目里那份收益只有 **0.04%** ——
21623 份单态化、9616 份可回收副本中，
**只有 8 份落在你自己的源码里**。它只能改你的源码，而热度大头在标准库和依赖里，
结构上够不着。

真正的大杠杆是**减编译量**：少编几个成员，就少编几百个包。

| | 包数 |
|---|---|
| 全部 24 个成员 | **865** |
| 显式 `default-members`（19 个） | **379** |
| **省** | **486（56%）** |

`slim` 把当初手工算这件事的过程自动化：

```bash
cargo xmake slim                          # 列出每个成员的独占包数
cargo xmake slim --auto                   # 自动挑独占 ≥5% 的成员试算（只读）
cargo xmake slim --drop=a,b --apply       # 确认后写进工作区根 Cargo.toml
cargo xmake undo                          # 删掉那一行，逐字节还原
```

**它怎么算的**

1. 跑一次 `cargo metadata`（带依赖图），解析 `resolve.nodes` 得到有向图；
2. 对每个成员做一次闭包（它自己 + 它能到达的所有包）；
3. **独占包数** = 该成员闭包里、别的成员闭包都覆盖不到的那些包 ——
   也就是「去掉它就真的不再编」的部分；
4. 按独占数排序，并给出丢掉某几个之后**确切**的新包数（不是估算）。

**安全约定**

- **绝不覆盖你自己写的 `default-members`** —— 遇到没有我们标记的就报错退出，让你自己处理；
- 写入的那一行带 `# cargo-xmake-slim` 标记，`cargo xmake undo` 靠标记删除，**逐字节可逆**；
- `--drop` 里给了不存在的成员名会**明确报错**，不会静默变成「什么都没丢」；
- CI 不受影响：CI 一般用 `-p <crate>` 显式指定，本来就绕过 `default-members`。

---

## 🔴 关于泛型，必须先说清楚三件事

### 1. 没有任何编译标志能做到这件事

我在 nightly 上把所有相关开关都试了一遍：

| 开关 | 实测结果 |
|---|---|
| `-Zshare-generics=yes` vs `=no` | 产出的 LLVM IR **字节数完全相同**（单 crate 没有东西可共享） |
| `-Zmir-opt-level=0` | ±0.7% |
| `-Zinline-mir=no` | ±0.6% |
| `-Zpolymorphize` | **已从 nightly 1.100 移除**（`unknown unstable option`） |
| `-Zmonomorphization-stats` | 不存在（某篇文章编的，真实名字是 `-Zdump-mono-stats`） |

> **教训**：`-Z` 开关要问编译器本人，别信文章。上面那条 `-Zpolymorphize` 甚至
> 最早是我自己记错的，是 `rustc -Zhelp` 纠正过来的。
>
> 还有一条我自己的错判也留在这里：我曾断定 `-Zdump-mono-stats`「什么都不输出、是死路径」。
> **错。** 它只是不写进 `target/` —— 报告落在 **crate 根目录的 `human/`**，
> 而我当时只翻了 `target/`。**"在预期位置没找到" ≠ "没产生"**，
> 下这种结论前先全仓 `find` 一遍。现在它是本工具**首选**的数据源。

所以 `dynify` 是一个**显式的源码变换**，不是悄悄生效的默认行为。

### 2. 它生成的形态，调用点一个字都不用改

```rust
#[cfg(debug_assertions)]                 // dev  → 1 份
pub fn drive(t: &dyn Work, n: u64) -> u64 { /* 原函数体 */ }

#[cfg(not(debug_assertions))]            // release → 每 T 一份，能内联
pub fn drive<T: Work>(t: &T, n: u64) -> u64 { /* 原函数体 */ }
```

`drive(&x, n)` 在这两种签名下**都能编译**，因为 `&Concrete → &dyn Trait`
的 unsize 强转是自动发生的。这是整件事能成立的关键。

代价：源码里同一段函数体出现两份。收益集中在 `audit` 排出来的头部热点上。

### 3. 收益不均匀，所以必须先排序

收益 ≈ **单态化份数 × 函数体大小**。一个只被实例化 2 次的小函数改了白改。
`audit` 就是干这个的：

```
$ cargo xmake audit
  包 dynproof-mono   剖面 dev   数据源 -Zdump-mono-stats (nightly)
  ──────────────────────────────────────────────────────────────────
  单态化份数 416      不同的「根」 18       平均复制 23.1×

  排名     份数      代价        根
  #1     200     9800      drive  · 可 dyn 化
  #2     200     2600      <# as Work>::step  · vtable，每类型一份，dyn 去不掉
  #3     1       1011      main
  …

结论： 有 199 份是「同一个泛型函数体被复制」（黄色那些），估算代价 9751（占全部代价的 72%）。
  把那些函数改成 `&dyn Trait`，预期能少掉约 48% 的单态化份数。
  另有 199 份是 `<# as Trait>::` 形态（青色）—— 每个具体类型必然一份，dyn 化消不掉；
    想减这部分只能减少具体类型本身的数量（或让它们共用一份 impl）。
```

**青色和黄色必须分开看。** 我第一版把它们都算进"可回收"，报出 95%，
而实测只降 47% —— vtable 槽位是每个具体类型必然一份的，dyn 化动不了。

**三种数据源，逐级降级**（`--xmk-mode=auto`（默认）/ `stats` / `mono` / `ir`）：

| 模式 | 用什么 | 需要 | 代价列 | 备注 |
|---|---|---|---|---|
| `stats` | `-Zdump-mono-stats` | nightly | ✅ | **首选**，rustc 自带代价模型，按收益排序 |
| `mono` | `-Zprint-mono-items` | nightly | ❌ | 按份数排序 |
| `ir` | `--emit=llvm-ir` | stable | ❌ | 兜底 |

> ⚠️ `ir` 模式**数的是 IR 符号，不是单态化实例**：实测 `main` 数成 2 份、
> `std::rt::lang_start` 数成 4 份，而 rustc 的实例化计数都是 1。
> 排名与 nightly 一致（`<# as Work>::step` 200 / `drive` 200），但绝对值会偏多 ——
> 所以 `audit` 在这个模式下会打一行黄字说明。想要精确份数请装 nightly。

> 📌 `-Zdump-mono-stats` 的报告**不写在 `target/`**，而是 crate 根目录的 `human/`。
> 工具读完会把**本次自己生成的那些**报告文件精确删掉，你原有的一个都不碰。

### 一个刻意的耦合

`dynify` 默认用 `cfg(debug_assertions)` 做开关，零接线。但这样 setup 就
**必须**把 dev 的 `debug-assertions` 设成 `true`，否则 release 分支会在 dev 也被选中。

这和「关掉 debug_assert 更快」是矛盾的，所以 `setup` 会专门打一段提示。
不接受这个耦合的话：`cargo xmake dynify --switch=feature`，
改用 `#[cfg(feature = "xmake-dyn")]`，由 `cargo xmake` 在 dev 时自动加 `--features`。

---

## 为什么设置写 `.cargo/config.toml`，而不是注入环境变量

本来最自然的做法是注入 `CARGO_PROFILE_DEV_*=…` 环境变量（不碰任何文件）。
实测否掉了：

```
[1] cargo build          → Compiled
[2] cargo build          → Fresh (0.01s)      ← 同环境，缓存命中
[3] 带 env 的 cargo build → Compiled          ← 指纹变了，全量重编
[4] 纯 cargo build        → Compiled          ← 又变了，又全量重编
[5] 带 env 的 cargo build → Compiled
```

cargo 会因为 `the profile configuration changed` 把**整棵依赖树**标记为脏。
379 个 crate 的项目就是每切一次 91 秒。

写进 `.cargo/config.toml` 之后指纹稳定，`cargo` 和 `cargo xmake` 共用一份缓存，
CI 也能自动吃到（配置文件跟着仓库走）。

顺带实测确认的几条 cargo 行为：

| 行为 | 结论 |
|---|---|
| `.cargo/config.toml` 里的 `[profile.*]` | **生效，且优先级高于 `Cargo.toml`** |
| `.cargo/config.toml` 是不是 manifest？ | 不是 —— 所以不会有 cranelift 那种 manifest feature-gate 崩溃 |
| `CARGO_BUILD_RUSTFLAGS` | **被 `[target.*] rustflags` 静默压制**，不可靠 |
| `RUSTFLAGS` / `CARGO_ENCODED_RUSTFLAGS` | 整体替换，会清掉 config 里的 rustflags |
| `cargo --config target.X.rustflags=[…]` | **与 config 合并**，唯一安全的追加途径 |

---

## 🔴 用户全局配置是"隐形"的

`~/.cargo/config.toml`（或 `$CARGO_HOME/config.toml`）里的 `[profile.*]`
会和项目配置**合并**，项目覆盖全局。本机就踩到了：

```toml
# C:\Users\liuqi\.cargo\config.toml
[profile.release]
lto = true
strip = true
opt-level = "z"     ← 于是"没配 profile"的项目 release 是 -C opt-level=z
```

一旦项目里写了 `[profile.release]`，`opt-level` 就变成 `3`，产物大小随之变化 ——
而用户完全看不到原因。所以 `setup` 会把这一类覆盖**逐条列出来**，
`doctor` 会把所有配置来源连同路径一起打出来。

（顺便：这台机器上 `CARGO_HOME` 指向 rustup 目录，但 cargo 实际吃的是
`~/.cargo/config.toml`。所以工具不猜，把两个候选都报出来。）

---

## 命令一览

| 命令 | 作用 |
|---|---|
| `cargo xmake <任意 cargo 子命令>` | 原样转发，共用缓存 |
| `cargo xmake setup` | 把剖面设置合进 `.cargo/config.toml`（不覆盖你已有的值） |
| `cargo xmake undo` | 精确撤销 —— 配置按 `# cargo-xmake` 标记删，源码按 `// cargo-xmake:dyn` 标记折叠回泛型 |
| `cargo xmake doctor` | 打印会生效的每一刀 + 所有配置来源 |
| `cargo xmake audit` | 单态化热点排序（三源降级：`-Zdump-mono-stats` → `-Zprint-mono-items` → 数 LLVM IR） |
| `cargo xmake dynify` | 去泛型改写，默认只预览；`--rewrite` 落盘并自动编译验证 |
| `cargo xmake slim` | **减编译量**：算各成员的独占包数并推荐 `default-members`（`--auto` 试算，`--drop=a,b --apply` 落盘） |
| `cargo xmake selftest` | 造个临时 crate 验证「config → rustc 命令行」整条链路 |

开关都带 `--xmk-` 前缀，放在命令行**任意位置**都行，cargo 永远看不到它们：

`--xmk-tier=safe|fast|extreme` · `--xmk-force` · `--xmk-trace` ·
`--xmk-cranelift` · `--xmk-no-link-flags` · `--xmk-package=` · `--xmk-target-dir=` ·
`--xmk-top=N` · `--xmk-mode=auto|stats|mono|ir` · `--xmk-switch=debug-assertions|feature`

### `undo` 怎么还原源码：靠标记，不靠备份

`dynify` 生成的每个函数上方都有一行 `// cargo-xmake:dyn`。
`undo` 找到这行，往后精确定位到「一对 `#[cfg(…)] / #[cfg(not(…))]` 分支」，
把第一条（dyn 版）整段删掉、只留第二条（泛型版）并去掉它的属性
—— 这是 `render()` 的**结构性逆变换**，可重复执行。

**为什么不用 `.xmake-bak` 备份还原？** 两条理由都站不住脚：

1. 备份只在「编译验证失败、需要回滚」的那一刻有意义；验证一通过它就删了，
   用户到那时已经**没有备份可用**。
2. 反过来，如果为了 `undo` 而把备份一直留着，`undo` 就变成
   **覆盖用户在 dynify 之后对源码做的所有编辑** —— 比不还原更危险。

标记法则把这两个问题都绕开了：只动我们生成的那一对分支，其它编辑一律不碰，且跨会话有效。
`undo` 写回前仍会**新复制**一份 `*.xmake-bak` 作为当次的安全垫（打印里会提示可删）。

还原是**逐字节**的：`render()` 的契约是输出不带尾随换行，替换区间的边界也按
「item 文本」对齐，所以行尾换行、空行、缩进都原样保留。
`src/dynify.rs` 里 `render_has_no_trailing_newline` 与
`render_then_revert_roundtrips_byte_for_byte` 两条测试把这个契约钉死了。

配置那边则是按 `# cargo-xmake` 标记删行；如果删完只剩下空行，
说明整份文件都是我们写的 → **连文件（和空的 `.cargo/` 目录）一起删掉**，
不留一个 1 字节的尸体。文件本来就不在时 `undo` 是**幂等空操作**，不报错。

### 一个小细节：`dynify` 会顺手把自己留下的缓存清掉

`dynify --rewrite` 落盘后跑 `cargo check` 验证。但 **check 与 build 的指纹不同**，
同一份源码会在 `target/debug/incremental/` 下多留一份**孤儿缓存**
（实测单文件项目 1.7 MB，大项目按比例放大）。不清理的话
`target/` 会从 4.06 MB 涨到 5.43 MB，看上去像"去泛型反而更占磁盘"。

所以工具会前后各快照一次 `incremental/` 子目录，**只删本次自己新建的** ——
和 `human/` 报告文件同一套纪律：不碰你原有的任何东西。
实测 `target/` 5,432,598 → **3,716,406 B**。

---

## 诚实的边界

* **`setup` 在单 crate 玩具上也能省时间，但主要不是省在"优化"上。**
  实测（单 crate、无依赖、N=200）dev **冷**构建：原样 1.07 s → `setup` 0.64 s（**−40%**）
  → 再加 `dynify` 0.59 s（再 −8%）；热构建 0.14 / 0.11 / 0.13 s（噪声级）。
  `setup` 那一截来自 `debug=false`（不生成调试信息）和 `codegen-units=16`；
  `dynify` 那一截才是"少编译 199 份函数体"。
  但这些杠杆真正的战场是**依赖树**（PDB、依赖的 opt-level、链接期），玩具项目里没东西可打。
* **`dynify` 的构建时间收益也不大**（dev 下 N=200 只快 5% 左右），
  因为 dev 的 `opt-level=0` 本来就不跑 LLVM 优化 pass。
  它真正赢的是**产物体积和 IR 规模**（−22.7% exe / −69% IR），
  以及在更高优化级别下的编译时间。
  （而 `measure.py` 那份单变量对照里 dev 冷构建快了 46.6% —— 那是因为它测的是
  两份**独立 crate**的冷构建，含 `cargo clean` 后的完整流程，口径不同。**同一实验内的数字才能相减。**）
* **`audit` 的 `ir` 兜底模式是启发式的。** v0 mangling 没有稳定可用的官方 demangler
  （`-C symbol-mangling-version=legacy` 需要 `-Z unstable-options`），所以工具里有一个
  ~150 行的 v0 子集解码器，只为"把同一函数的实例化归到一个根"服务，不做高保真还原。
  它由真实符号语料 + **跨模式交叉验证**（与 nightly 的排名必须一致）钉住；
  但 `ir` 模式**数的是 IR 符号不是单态化实例**（`main` 会数成 2、`lang_start` 数成 4），
  所以报告里会明确标注。想要权威数字请装 nightly。
* **cranelift 只能 dev 用**，而且**绝对不能写进 `Cargo.toml`** ——
  stable cargo 在**解析 manifest 时**就硬失败：
  `feature codegen-backend is required ... not stabilized in this version of Cargo`，
  不是"忽略这个键"，是整个 manifest 解析失败，CI 直接死。
  唯一可用途径是 rustc 级 `RUSTFLAGS`。它也**不支持 `lto = "fat"`**。
* **lld 对 lilyco 无效**（93.9 → 95.7 s）—— 它只影响链接，而链接在总时长里只占几秒。
* **`--jobs-frontend`（原 `-Zthreads`）是噪声级**，cargo 本来就在 crate 之间并行。
* **`cargo clean -p` 在新布局下不可靠** —— 实测 `Removed 0 files`。
  所以 `audit` 用 `-C metadata=<每次不同的 nonce>` 强制重跑，
  额外参数只作用于目标 crate，依赖不重编。

---

## 🔴🔴 真实项目上是什么样（这一节比上面的数字都重要）

上面所有数字都来自 `experiments/dynproof/` 那个 **N=200 的合成工程** ——
那个工程是**按"可 dyn 化"的形状设计出来的**，所以收益好看。
拿真实的 `lilyco-binfmt`（79 个 rs 文件的 workspace 成员）跑一遍，结论完全不同：

```
单态化份数 21623    不同的「根」 4448     平均复制 4.9×

#1   657   <# as std::iter::Iterator>::fold          vtable，每类型一份
#2   313   <# as std::vec::…::SpecFromIterNested>::…  vtable
#3   297   <# as std::iter::Iterator>::find           vtable
#5   149   std::vec::Vec::<#>::extend_desugared      被复制，但在标准库里
#10  548   std::option::Option::<#>::and_then        被复制，但在标准库里
#13    1   office_doc::run_office_doc                ← 本地代码，份数是 1
```

* **排名前 12 的热点全在 `std`/`alloc`/`core` 里**，`dynify` 一个都碰不到。
* **本地代码的份数基本全是 1** —— 它压根没被复制，也就没什么可省的。
* `dynify` 扫下来 84 个泛型函数，只有 **1 个**符合改写条件。
* 结论：9616 份「可回收」里，**只有 8 份在你自己的源码里（0.04%）**。

所以 `audit` 现在会把每个热点标成三类：

| 标注 | 含义 |
|---|---|
| `· 可 dyn 化（在你的源码里）` | dynify 真能改 |
| `· 被复制，但在标准库里，dynify 改不到` | 只能靠减少具体类型数量 / 换依赖 |
| `· vtable，每类型一份，dyn 去不掉` | 每个具体类型必须有一份，跟 dyn 无关 |

**本地份数不到 1% 时，它会直说"dynify 在这个项目上不值得做"**，
而不是继续推荐你跑 `dynify`。

> 教训：工具的收益**严重依赖代码形状**。合成工程证明"机制成立"，
> 但**不能**用来承诺"你的项目能省多少"。真实项目里泛型复制的绝大部分
> 发生在标准库和依赖里 —— 那是 `dynify` 结构上就够不着的地方。

真实项目还逼出了 5 个只有它们才会暴露的 bug（中文注释 panic、位置参数被忽略、
Git Bash 路径、报告写到 workspace 根、把库函数算成可改），
详见 `docs/06-cargo-xmake.md` §2.8。

---

## 实测记录

完整实验与数据在上级仓库：

* `experiments/dynproof/` —— 单变量对照工程（`gen.py` 生成 mono/dyn 两份，只差一行签名）
* `docs/06-cargo-xmake.md` —— 设计决策与本轮所有实测
* `docs/05-toolchain-cards.md` —— lld / 并行前端 / cranelift 三张牌
* `docs/04-rust-vs-xmake-speed.md` —— 与 xmake 的三层对比

关键量级（两个**不同**的实验，别混用）：

* **单变量对照**（`experiments/dynproof/`，两份源码只差 `drive` 的签名一行，N=200，
  `measure.py` 的冷构建、每项 5 次取中位）：
  dev 冷构建 **1.03 → 0.55 s（−46.6%）**、dev exe **229,888 → 177,664 B（−22.7%）**、
  release exe 143,872 → 143,360 B（**−512 B，0.36%** —— fat LTO 基本把 dyn 调用去虚化掉了）；
  `drive` 的单态化实例 **200 → 0**；release 运行 10.7 → 11.7 ms（噪声范围）。
* **端到端**（同一份源码 + `dynify`，见上表）：release 档四项数字与只 `setup` 时
  **逐项相同** ⇒ release 零代价。

两者结论一致：**去泛型只在 dev 阶段有价值，release 靠 LTO 自己就把它抹平了。**

## 许可

MIT
