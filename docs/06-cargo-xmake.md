# 06 · cargo-xmake：把「dev 快、release 猛」做成一个工具

本文记录 **cargo-xmake** 的决策依据与全部实测。工具本身在 `cargo-xmake/`。

---

## 一、决策记录（下笔写代码之前先记下来）

**需求（一句话）**
做一个兼容 cargo 的子命令，`cargo new` / `run` / `build` / `test` 照旧可用，
但 debug 构建有 C/xmake 的速度和体积；**dev 默认开虚函数（动态派发）少单态化，
release 关掉虚函数换性能**。

**已有方案（ACQUIRE）**

| 仓库 | 是什么 | 覆盖了我们要的哪一半 |
|---|---|---|
| `mew-sh/rustm`（3★，MIT） | cargo 包装器：Cranelift(dev) → mold/lld → LLVM(release) → PGO/BOLT + sccache | 「标志那一半」基本被做完了 |
| `MaxPer2005/crystal-build`（0★） | 透明构建缓存：`cargo clean && crystal build` 79s → 0.39s | 另一个问题（重复冷构建），不是我们的 |

`gh search repos` 搜 `cargo-xmake` / `cargo xmake` / `cargo subcommand build speed` /
`rust generic monomorphization reduce` —— **都没有**。
**「dev 开虚函数 / release 关」这一半是空地，没有任何先例。**

**冷知识（这一轮实测挖出来的，也是唯一真正的壁垒）**

1. **没有任何编译标志能把静态派发变成动态派发。** 见下方第二节的表格。
2. 所以这必须是一个**源码变换**；而它只有做成 `cfg` 双形态才能让
   「dev 一份 / release 每 T 一份」同时成立。
3. 收益**不均匀**，正比于「单态化份数 × 函数体大小」→ 工具必须先**排序**。
4. `<# as Trait>::method` 的份额 **dyn 化去不掉**（vtable 每类型必然一份），
   必须和「泛型函数体被复制」分开统计。

**决策**
做 `cargo-xmake`，分四层：

| 层 | 手段 | 为什么这么做 |
|---|---|---|
| 剖面设置 | 写 `.cargo/config.toml` 的 `[profile.*]` | 环境变量会引发全量重编（第二节） |
| 链接期 | `[target.*] rustflags` 追加 `/DEBUG:NONE` | 环境变量 `CARGO_BUILD_RUSTFLAGS` 会被静默压制 |
| 泛型热点排序 | `-Zprint-mono-items` / `--emit=llvm-ir` | 编译器自带的设施比解析 IR 可靠 |
| 去泛型改写 | cfg 双形态源码变换，`--rewrite` + 自动编译验证 + 失败回滚 | 唯一可行路径，且必须可逆 |

**下一个动作**：写工具 → 在 `experiments/dynproof` 上端到端验证。

---

## 二、八个把设计逼出来的实测

> 2.1~2.7 来自合成工程，2.8 来自**真实项目** —— 而 2.8 推翻了我对 2.1~2.7 的过度推广。

### 2.1 🔴 没有任何 `-Z` 开关能减少单态化

在 nightly 1.100.0 上，对同一个 N=200 的 mono 工程跑 `--emit=llvm-ir`，只换开关：

| 开关 | IR 字节 | 结论 |
|---|---|---|
| 基线 | 4,101,270 | — |
| `-Zshare-generics=yes` | 4,101,270 | **完全相同** |
| `-Zshare-generics=no` | 4,101,270 | **完全相同**（单 crate 没东西可共享） |
| `-Zmir-opt-level=0` | 4,129,667 | +0.7%（噪声） |
| `-Zinline-mir=no` | 4,125,785 | +0.6%（噪声） |
| `-Zpolymorphize` | — | **`error: unknown unstable option`** —— 已从 1.100 移除 |

> 附带纠正一条我自己的错误笔记：上一轮我写过「`-Zpolymorphize` 可用」。
> 它是错的。`rustc -Zhelp` 里根本没有这个选项。
> **又一次说明：`-Z` 开关要问编译器，不要问笔记，更不要问文章。**
> （同一类错误的另一个案例：有文章说 1.100 有 `-Zmonomorphization-stats`，
> 真实名字是 `-Zdump-mono-stats`。）
>
> ⚠️ **这条我自己也错判过一次，必须留下**：我曾在本文件里写
> 「`-Zdump-mono-stats` 什么都不输出、是死路径」。
> **错。** 它只是**不写进 `target/`** —— 报告写在 **crate 根目录的 `human/`**
> （`<crate>/human/<name>.mono_items.{json,md}`）。我当时只翻了 `target/`，
> 看不到文件就下了「死路径」的结论。
> **教训：判定一个开关"没输出"之前，先全仓 `find` 一遍同名产物；
> 「在预期位置没找到」不等于「没产生」。**
> 现在它是本工具**首选**的数据源，因为 rustc 顺带给了一个代价模型：
> ```json
> [{"name":"drive","instantiation_count":200,"size_estimate":40,"total_estimate":8000},
>  {"name":"<T152 as Work>::step","instantiation_count":1,"size_estimate":10,"total_estimate":10}]
> ```
> `total_estimate = count × size_estimate` 就是 rustc 自己的成本模型 ——
> 这正是"按收益排序"需要的那个排序键，比我手写的分组强得多。

### 2.2 🔴 环境变量剖面注入会引发全量重编

```
[1] cargo build              → Compiling
[2] cargo build              → Fresh (0.01s)   ← 同环境，缓存命中
[3] CARGO_PROFILE_DEV_DEBUG=false cargo build → Compiling  ← 指纹变了
[4] cargo build              → Compiling       ← 又变了
[5] CARGO_PROFILE_DEV_DEBUG=false cargo build → Compiling
```

cargo 会打印 `Dirty …: the profile configuration changed`，把整棵依赖树标脏。
379 个 crate 的项目每切一次就是 91 秒。

**而 `.cargo/config.toml` 里的 `[profile.*]` 实测生效，且优先级高于 `Cargo.toml`。**
于是 `cargo build` 和 `cargo xmake build` 共用同一份指纹、同一份缓存，
CI 也自动吃到（配置文件跟仓库走）。这是选 config 而不是环境变量的**唯一**理由。

### 2.3 🔴 四种 rustflags 注入途径，只有一种能追加

| 途径 | 实测行为 |
|---|---|
| `[target.X] rustflags`（config 文件） | 生效 |
| `CARGO_ENCODED_RUSTFLAGS` | **整体替换**，config 里的 rustflags 全部消失 |
| `RUSTFLAGS` | **整体替换**，同上 |
| `CARGO_BUILD_RUSTFLAGS` | **被 `[target.X] rustflags` 静默压制** —— 我们传的 `-Copt-level=0` 直接不见了，且不报任何错 |
| `cargo --config 'target.X.rustflags=[…]'` | **与 config 合并**，两边都在 ✅ |

所以 cargo-xmake 要加链接参数时走 `--config`（或直接写进 config 文件），
绝不碰 `CARGO_BUILD_RUSTFLAGS` —— 它是个会静默失败的陷阱。

### 2.4 🔴 用户全局配置是「隐形」的剖面来源

本机 `C:\Users\liuqi\.cargo\config.toml`：

```toml
[profile.release]
lto = true
strip = true
opt-level = "z"
```

它会被合并进每个项目。于是：
* 项目「什么都没配」→ release 是 `-C opt-level=z`，exe 135,168 B
* 项目「配了 `[profile.release]`」→ 变成 `-C opt-level=3`，exe 143,872 B

**产物大了 6.5%，而用户完全看不到原因。** 所以 `setup` 会把这类覆盖逐条列出，
`doctor` 会把所有配置来源连同路径打印出来。

还有个更细的坑：这台机器上 `CARGO_HOME=D:\…\rustup\.cargo`，
但 cargo 实际吃的是 `~/.cargo/config.toml`。**工具不猜，两个候选都报。**

**后来我把这条查到底了（三重确认）：**

```bash
# ① rustc 真正收到什么 —— 最硬的证据
cargo build --release -v | grep -oE '\-C opt-level=[^ ]+'
#   → -C opt-level=z    -C lto    -C strip=symbols

# ② 问 cargo 自己（nightly 才有 config 子命令）
cargo +nightly -Z unstable-options config get profile.release --show-origin
#   profile.release.opt-level = "z" # C:\Users\liuqi\.cargo\config.toml
```

**③ 它关不掉**：把 `CARGO_HOME` / `HOME` / `USERPROFILE` / `HOMEDRIVE` / `HOMEPATH`
全部指向空目录之后，cargo **仍然**从 `C:\Users\liuqi\.cargo\config.toml` 读配置 ——
Windows 上 cargo 的 home 是走 OS API（profile 文件夹）解析的，不看这几个环境变量。

> ⚠️ 我曾在别处的笔记里写「全局 config.toml 的 `[profile.*]` 完全无效」。
> **那是错的**，而且错得很有代表性：当时的验证方式是「若 `opt-level="z"` 生效，
> hello world 应该 ~10KB；实测 112,128 B ⇒ 没生效」。两处漏洞：
> ① Windows 上 strip 过的 hello world 本来就有 ~110 KB 的 PE + std 基线；
> ② **拿间接指标（产物大小）去反推直接事实（rustc 参数）**，中间任何别的因素都会污染结论。
> **教训：能用 `-v` 直接看到的，就不要用大小去猜。**

所以这个坑的真正危险不是"无效"，而是"**隐形生效**" ——
用户完全看不见，产物大小却变了。`setup` 因此会把这类覆盖逐条列出来。

### 2.5 🔴 `cargo clean -p` 在新布局下不可靠

`audit` 需要强制目标 crate 重编（否则第二次调用是 fresh，一行输出都没有）。
`cargo clean -p dynproof-mono` 实测输出 `Removed 0 files`，然后 cargo 依然 `Finished`。

解法：给 `cargo rustc --` 传一个每次都变的 `-C metadata=<nonce>`。
额外参数**只作用于目标 crate**，依赖不重编 —— 实测每次只有
`Compiling dynproof-mono v0.1.0`，没有任何依赖被重编。

### 2.6 🔴 stable 用户拿不到可读符号：v0 mangling 得自己解

`-Zdump-mono-stats` 与 `-Zprint-mono-items` 都是 nightly。stable 用户只剩
`--emit=llvm-ir` 一条路，而 IR 里的符号是 **v0 mangling**：

```
@_RINvCsj1Qc4EzirgF_13dynproof_mono5driveNtB2_2T0EB2_
@_RNvXCsj1Qc4EzirgF_13dynproof_monoNtB2_2T0NtB2_4Work4step
```

我原本用"抠 `<len><ident>` 片段"的土办法，实测**把名字解成垃圾**
（`cuyqEa_3::rt::lang_start::Q::Ezirg`），于是 416 个符号归出 415 个根，
报告说"只有 1 份重复、可回收 0%"——**比不给数据更坏，因为它看起来像结论**。

试过的两条捷径，都堵死：

| 捷径 | 实测 |
|---|---|
| `-C symbol-mangling-version=legacy` 换成好解的 legacy 形式 | **`requires -Z unstable-options`** —— stable 下不可用 |
| 用现成 demangler 库 | 打破"工具自身零依赖"这条（3 万行 v0 语法的替代品） |

最后写了一个**只求归一、不求保真**的 v0 子集解码器（约 150 行）。三条核心规则
全部由上面的真实符号反推，并有单测钉住：

1. **见到第二个 `C`（crate 根）就停** —— 那是实例化方，不进名字。
   （但 `N` 后面的 `C` 是"闭包命名空间"，不是 crate，得先吃掉命名空间字符。）
2. **`I` 之后先读值命名空间的定义路径，遇到 backref/基本类型就停** ——
   这一段是真路径，后面的都是泛型实参。
3. **`X` = trait impl 时改用"全部 ident 的最后两段"** —— 因为 trait 路径本身
   也长在 `NtB…_` 后面，规则 2 会把 `Work` 一起切掉。

还有个反直觉的坑：**v0 的 `<len>` 只能按十进制读**。
`10lang_start` 是"长度 10 + 名字 10 字符"，若贪心按 base62 读会一路吃到
后面第一个 `_`（把 `10lang` 当长度），全盘错位。

**验证办法是跨模式交叉对照**：IR 兜底算出的排名必须和 nightly 的
`-Zprint-mono-items` 一致。实测两边都是
`<# as Work>::step` 200 份（vtable）+ `drive` 200 份（可 dyn 化），排名一致。

> 但要诚实标注口径差异：**IR 数的是符号，不是单态化实例**。
> 实测 `main` 数成 2 份、`std::rt::lang_start` 数成 4 份，而 rustc 的实例化计数都是 1。
> 所以 `audit` 在 IR 模式下会打一行黄字说明这件事，不让用户以为三种数据源等价。

### 2.7 🔴 `dynify` 的编译验证会留下孤儿增量缓存

`dynify --rewrite` 落盘后跑 `cargo check` 验证。**check 与 build 的指纹不同**，
于是同一份源码在 `target/debug/incremental/` 下**多出一份孤儿缓存**：

```
s_cfg   → incremental/ 1 个目录   3,309,709 B
s_dyn   → incremental/ 2 个目录   3,069,216 + 1,716,202 B
                     ↑ 真实缓存（还变小了）   ↑ cargo check 留下的孤儿
```

实测 `target/` 因此从 4,062,616 B（只 setup）**涨到** 5,432,598 B，
看上去像"去泛型反而让磁盘变大"——数据自相矛盾，差点写进文档。

修法与 `human/` 那套同一纪律：**前后各快照一次 `incremental/` 子目录，
只 `remove_dir_all` 本次新建的**，用户原有缓存一个都不碰。
修完 `target/` 5,432,598 → **3,716,406 B**，比只 setup 的还小，数据自洽了。

> 教训（第二次犯同类错）：**读数不符合预期时，先怀疑测量脚本和自己的测量动作**。
> 上一轮是脚本删了 config 没恢复；这一轮是**测量动作本身污染了被测对象**
> （我额外跑的 `cargo rustc --emit=llvm-ir -Cmetadata=…` 往 `target/` 塞了产物）。
> 最终数字全部来自**干净副本 + 只跑 `build`/`build --release`**。

### 2.8 🔴🔴 拿真实项目一跑，五个 bug 同时现形（也是本章最重要的一节）

前面 2.1~2.7 全部来自 `experiments/dynproof/` 的 **N=200 合成工程**。
那个工程是**按"可 dyn 化"的形状造出来的**，所以机制验证得很好看。
换真实的 `lilyco-binfmt`（workspace 成员，79 个 rs 文件）跑，第一次运行是这样的：

```
扫描 0 个泛型函数：0 个可改，0 个不改      ← 84 个泛型函数一个都没扫到
```

一个 79 文件的真实 crate 不可能没有泛型函数。于是挨个查，五个问题全冒出来了。

#### (1) 中文注释直接把扫描器搞 panic

`dynify` 里按字节回溯、判断"上一个字符是不是标识符/空白"的地方用了
`(b[i] as char).is_alphanumeric()`。**这是个会崩的陷阱**：

* `b` 是**单个字节**；`0xBA as char` 是 `'º'`，而 `'º'.is_alphanumeric() == true`；
* 中文 UTF-8 的续字节里 `0xBA` 极常见 —— `示` = `E7 A4 BA`；
* 于是回溯一头扎进多字节字符中间，紧接着 `&text[k..end]` 直接
  `byte index 858 is not a char boundary; it is inside '示'` panic。

同一模式共 5 处，`is_whitespace` 那条也一样有毒
（`0xA0 as char` 是 NBSP、`0x85 as char` 是 NEL，都算空白）。
**修法：全部换成 `is_ascii_whitespace()` / `is_ascii_alphanumeric()`** ——
ASCII 字节永远不会出现在多字节序列内部，所以 ASCII 判断天然安全。
`next_word()` 也补上按整字符步进，并对**非 ASCII 函数名显式拒绝**
（不靠"恰好解析对了"）。

> 合成工程里全是 ASCII，所以这条**永远测不出来**。真实代码里中文注释是常态。

#### (2) `opts.paths` 从来没被赋值 → 位置参数被静默忽略

`Opts` 里有 `paths: Vec<String>`，`scan_paths` / `rs_files` 都在用它，
但 CLI 解析里**没有任何一处给它赋值**。于是 `cargo xmake dynify <path>`
静默忽略路径，永远退回 `root/src`。

#### (3) Git Bash 的 `/d/Code/...` 在 Windows 上不是绝对路径

`Path::is_absolute()` 对 `/d/Code/x` 返回 **false**（缺盘符前缀），
于是被当相对路径拼到 `root` 后面 → 路径不存在 → 又静默变成"0 个"。
现在会认出 `/x/` 这种 MSYS 形态并还原成 `D:\…`。

#### (4) 报告写到了 **workspace 根**，工具只在 crate 目录里找

`-Zdump-mono-stats=human` 的路径相对 **rustc 的 cwd，而 cargo 把 rustc 的 cwd
设成 workspace 根**。单 crate 项目里"包根 == workspace 根"，看不出差别；
一到 workspace 报告就落到别处，工具于是稳定地报"没读到报告"。

顺带纠正两条我此前写错的细节（对 cargo 1.100）：

| 我原来写的 | 实际 |
|---|---|
| `-Zdump-mono-stats` 是**裸开关** | 它**带值，值就是输出目录**：`-Zdump-mono-stats=human`；不写值就落在 cwd |
| 它产出 **JSON** | **默认 markdown**，要 JSON 得再加 `-Zdump-mono-stats-format=json` |

还有一条更隐蔽的：报告文件名由 crate 名决定，**重跑是覆盖同名文件而不是新建**，
所以"按路径差集找新文件"会漏掉刚生成的那份、进而误读上次的陈旧数据。
**必须按 `(路径, mtime)` 比对。**

#### (5) 🔴 把"库函数被复制"算成了"可 dyn 化"

这是最严重的一条，因为它不是崩溃，是**一个会被当成结论的错数字**：

```
老输出： 有 9616 份是「同一个泛型函数体被复制」（黄色那些），估算代价 163565（33%）。
        把那些函数改成 &dyn Trait，预期能少掉约 44% 的单态化份数。
```

而那 9616 份里，`std::vec::Vec::<#>::extend_desugared`、`Option::<#>::and_then`、
`map_fold::{closure#0}` 这类**标准库函数**占了 9608 份 ——
`dynify` 只能改本 workspace 的源码，**它一份都碰不到**。

真实分布是：

```
9616 份「被复制」  →  本地源码 8 份（0.04%） + 依赖/标准库 9608 份
```

也就是说"预期少掉 44%"是**根本做不到**的。修法：给每个热点判归属
（`Local` / `Dep` / `Std`），结论段分开报，并在本地占比 < 1% 时**直说
"dynify 在这个项目上不值得做"**。

归属判据（利用 mono-stats 的命名习惯）：

* 本地 item 的名字是**从 crate 根起的模块路径、不带 crate 名** ——
  所以首段既不是 `std`/`core`/`alloc`、也不在任何包名集合里 → 本地；
* 首段命中依赖包名 → `Dep`；命中 workspace 成员名 → 本地（dynify 会扫所有成员）；
* 首段是 `std`/`core`/`alloc`/`proc_macro`/`test` → `Std`。

顺带修掉 `type_inherent()` 的口径：它算的是 `<# as Trait>::` 的总份数，
但实现写成了 `count - 1`，而它自己的措辞是"另有 N 份"。vtable 槽位
**每一份都真实存在、一个也省不掉**，所以口径该是**全量** `count`
（`reclaimable()` 用 `count - 1` 是因为"合并后至少留 1 份"，两边本就不同）。

#### 这一节真正的教训

**合成工程只能证明"机制成立"，不能用来承诺"你的项目能省多少"。**
五个 bug 里有四个（(1)(3)(4) 和部分 (2)）都是"在玩具上恰好成立"的假设，
而 (5) 更根本：玩具工程的形状本身就是按工具的能力设计的。

**只要还没在真实项目上跑过，就不能说"优化 OK"。**

---

## 三、工具的设计落点

### 3.1 对 cargo 透明

自己的开关一律 `--xmk-` 前缀，从 argv **任意位置**摘掉；
摘完剩下的就是纯 `cargo <sub> <args…>`，原样转发。
`--xmk-` 在 cargo 里不存在，所以永远不会误吞。

cargo 以 `cargo xmake …` 调用时 `argv[1] == "xmake"`，跳过即可；
直接敲 `cargo-xmake build` 也认。

### 3.2 dev 档的每一刀

| 键 | 值 | 理由 |
|---|---|---|
| `debug` | `false` | 不生成 DWARF，dev 产物体积的绝对大头 |
| `strip` | `"debuginfo"` | 双保险 |
| `incremental` | `true` | 内环速度的命根子 |
| `opt-level` | `0` | 不跑 LLVM pass，codegen 最快 |
| `debug-assertions` | `true` | **见下方耦合说明** |
| `overflow-checks` | `false` | C 的默认就是静默回绕 |
| `codegen-units` | `16` | cargo dev 默认 256 |
| `[target.*] rustflags` | `-C link-args=/DEBUG:NONE` | MSVC 上 rustc 即使 `debug=false` 也照传 `/DEBUG`，PDB 照生成 |

三档：`safe`（只砍调试信息，语义零改动）/ `fast`（默认）/ `extreme`（加 `panic=abort`）。

### 3.3 release 档 = 什么都不用「关」

用户要求「release 关闭虚函数增加性能」。但实际上 **release 不需要这个动作**：
Rust 默认就是静态派发，`opt-level=3` + `lto="fat"` + `codegen-units=1`
本来就会把 dev 里留下的 dyn 调用去虚化并内联。

所以 release 档只做一件事：把优化开关推到顶。

### 3.4 🔴 一个刻意的、必须公开的耦合

`dynify` 生成的默认开关是 `cfg(debug_assertions)`，因为它**零接线**：
dev 天然是 true，release 天然是 false。

但这意味着 dev 档**必须**保留 `debug-assertions = true` ——
和「关掉 debug_assert 更快」直接冲突。工具不能默默选一个，所以：

* `setup` 会专门打一段 `!` 提示解释这件事；
* `doctor` 打印的是 `build_config_plan()` 的结果（也就是**真正会写下去的**那份），
  而不是 `dev_plan()` 的原始表 —— 否则打印的和写下去的不一致；
* 不接受耦合的人可以 `dynify --switch=feature` 换成
  `#[cfg(feature = "xmake-dyn")]`。

### 3.5 生成的形态与「调用点零改动」

```rust
#[cfg(debug_assertions)]                 // dev  → 1 份
pub fn drive(t: &dyn Work, n: u64) -> u64 { /* 原函数体 */ }

#[cfg(not(debug_assertions))]            // release → 每 T 一份，能内联
pub fn drive<T: Work>(t: &T, n: u64) -> u64 { /* 原函数体 */ }
```

`drive(&x, n)` 在两种签名下**都能编译** —— `&Concrete → &dyn Trait` 的
unsize 强转是自动发生的。这是整件事成立的关键，也让改写不需要碰调用点。

改写安全措施：
* 默认 `dry-run`，`--rewrite` 才落盘；
* 落盘前先写 `<文件>.xmake-bak` 备份；
* 落盘后立刻 `cargo check`，**失败自动回滚**；
* 不在安全模式内的候选直接拒绝并给出原因（多约束 / `?Sized` /
  返回类型含 T / 用了 `T::` 关联函数 / T 没以 `&T` 出现）。

### 3.6 只碰 `.cargo/config.toml`，绝不碰 `Cargo.toml`

* config 里的 `[profile.*]` 优先级更高，效果一样；
* config **不是 manifest**，所以不会触发 manifest 的 feature gate ——
  这正是 cranelift 在 stable 上把整个构建搞崩的原因（见 `05-toolchain-cards.md`）；
* 我们写的每一行都以 `# cargo-xmake` 结尾，`undo` 能精确还原；
* 用户已有的同名 key 默认**不动**（`--xmk-force` 才覆盖）。

### 3.7 🔴 `undo` 必须真的能「回去」——两半都要能回

配置好撤（删掉带标记的行就行），**源码难撤**：
`dynify` 生成的是两个同名函数，撤掉得把其中一段整段删掉。
我一开始只做了配置那一半，`undo` 的 help 却写着"还原"，
于是 `src/main.rs` 里的 `#[cfg(not(debug_assertions))]` 留在那儿 —— 一个说谎的接口。

这里有个岔路，两条都不通：

| 方案 | 为什么不成立 |
|---|---|
| 用 `.xmake-bak` 备份还原 | 备份在**编译验证成功后就删了**，用户真想撤的时候已经没有；反过来为了 `undo` 而长期保留备份，`undo` 就变成**覆盖用户在 dynify 之后做的所有编辑**，比不还原更危险 |
| 靠"记得原文" | 跨会话就没了 |

所以走**标记驱动的结构性逆变换**：`dynify` 在每个生成函数上方留一行
`// cargo-xmake:dyn`（稳定、唯一、可 grep），`undo` 靠它定位到
「一对 `#[cfg(…)] / #[cfg(not(…))]` 分支」，删掉第一条、留下第二条并去掉其属性。
只动我们生成的那一对分支，其它编辑一概不碰，跨会话有效。
写回前仍会新复制一份 `*.xmake-bak` 作为**当次**的安全垫。

**逐字节精确**这件事比看上去脆弱，我在这上面栽过：
一开始 `revert` 还额外吃掉一个换行，用来"补偿" `render()` 末尾的 `\n`。
那是在**补偿错误的东西** —— 真正的错配是 `apply()` 调了 `rendered.trim_end()` 而测试没调，
即测试没镜像真实路径。把 `render()` 的契约改成"**输出不带尾随换行**"、
让替换区间严格按「item 文本」对齐之后，行尾换行/空行/缩进全部原样保留，
`revert` 也就退回成 `i = e2 + 1`（什么都不额外消费）。
两条测试把这个契约钉死：`render_has_no_trailing_newline`（前置契约）与
`render_then_revert_roundtrips_byte_for_byte`（端到端）。

配置文件还有两个收尾细节：
* 删完只剩空行 → 整份文件都是我们写的 → **连文件（及空的 `.cargo/` 目录）一起删掉**，
  否则会留一个 1 字节的尸体；
* 文件本来就不在 → **幂等空操作**（连续敲两次 `undo` 的第二次），返回 `changed: false`
  而不是让 `fs::read_to_string` 抛 `os error 3`。

整条链路固化成 `scripts/smoke_cargo_xmake.sh`：建临时工程走完
`setup → dynify --rewrite → build → undo → 再 undo`，断言 27 项
（源码 md5 逐字节还原、二次 undo 幂等、release 零代价、
**同一份 config 下 dev 产物确实更小**、多约束函数被预期拒绝等）。

---

## 四、端到端验证

工程：`experiments/dynproof/mono`（N=200 个具体类型，`gen.py` 生成），
`.cargo/config.toml` 由 `cargo xmake setup` 写入。

### 4.1 单态化

| | 单态化条目 | 不同的「根」 | `drive` 副本 |
|---|---|---|---|
| 改写前 | 416 | 18 | 200 |
| 改写后 | **218（−47.6%）** | 19 | **1** |

`<# as Work>::step` 前后都是 200 —— 那是每个具体类型必然一份的 vtable 槽位，
dyn 化动不了。**我第一版把两者都算进"可回收"，报出 95%，与实测的 47% 差了一倍。**
现在 `reclaimable()` 显式排除 `<# as Trait>::` 形态，并单独统计 `type_inherent()`。

### 4.2 构建时间与产物体积（单 crate 玩具，干净副本、只跑 build）

三份独立副本，各自有独立 `target/`；体积数字来自**只跑 `build` / `build --release`**
的干净目录（吸取 2.7 的教训：测量动作本身会污染被测对象）。

| | dev 单态化 | `target/` | dev exe | release exe | release 单态化 | PDB |
|---|---|---|---|---|---|---|
| A 原代码 / 无 config | 416 | 5,042,395 B | 229,888 B | 135,168 B ※ | 416 | 4 个 |
| B 原代码 / `setup` | 420 | 4,062,616 B（−19.4%） | 230,912 B | 143,872 B | 406 | **0 个** |
| C `setup` + `dynify` | **218（−47.6%）** | **3,716,406 B（−26.3%）** | **177,664 B（−22.7%）** | **143,872 B** | **406** | **0 个** |

※ A 的 release 之所以更小，是因为它吃到了用户全局的 `opt-level="z"`（见 2.4），
不是我们的功劳。**B 和 C 是 143,872 B，两者逐字节相同。**

**C 与 B 的 release 四项数字完全一致**（406 份 / 8 个根 / `drive` 8000 / `step` 1998），
`drive` 恢复成 200 份单态化 —— 这就是「dev 开虚函数、release 关」被验证的证据：

* dev：`drive` 200 份 → **1 份**
* release：`drive` **仍是 200 份**，产物字节不变 ⇒ 运行时零代价

一个必须说清的点：**B 的 dev 单态化是 420 份，比 A 的 416 份还多 4 份。**
`setup` 本身**不减少单态化**（它改的是 debug 信息、opt-level、链接参数）。
减少单态化的只有 `dynify` 这一步。把两者混为一谈就会得到一个错误的因果。

### 4.3 诚实的边界

* **`setup` 在单 crate 玩具上几乎不动 dev exe**（229,888 → 230,912 B，噪声级）。
  这些杠杆打的是**依赖树**（PDB、依赖 opt-level、链接期），玩具里没东西可打。
  它在玩具上唯一确定拿到的是：**PDB 从 4 个变 0 个**、以及 release 档可控。
* **`dynify` 的构建时间收益也小**（dev 下 N=200 约 −5%），
  因为 dev 的 `opt-level=0` 本来就不跑 LLVM 优化 pass。
  它真正赢的是**产物体积与 IR 规模**（−22.7% exe / −69% IR），
  以及在更高优化级别下的编译时间。
* **`dynify` 会改源码**（存 `.xmake-bak` 备份，`cargo xmake undo` 可撤），
  且只对**能被 dyn 化**的泛型函数生效 —— 用了关联类型/关联函数的会被跳过并给出原因。
* 收益正比于「单态化份数 × 函数体大小」，所以 `audit` 的排序比开关更重要。

### 4.4 🔴 真实项目验证：`lilyco-binfmt`（workspace 成员，79 个 rs 文件）

这是唯一一次"离开玩具"的验证，也是把 §2.8 那五个 bug 全逼出来的地方。

| 项 | 数字 |
|---|---|
| 单态化份数 | 21623 |
| 不同的「根」 | 4448 |
| 平均复制 | 4.9× |
| 报告条数 | 5058（`-Zdump-mono-stats` 只列被复制的） |
| **`dynify` 扫到的泛型函数** | **84 个** |
| **其中符合改写条件的** | **1 个** |
| 被复制的份数里属于**本地源码**的 | **8 / 9616 = 0.04%** |

排名前 12 的热点**全部**是 `std`/`alloc`/`core` 里的泛型
（`Iterator::fold` 657、`Iterator::find` 297、`Vec::extend_desugared` 149…），
本地代码要到 #13 才出现，而且份数是 **1**（压根没被复制）。

**结论：在这类项目上 `dynify` 不值得做。**
它没有失效 —— 是它的作用域（本 workspace 的源码）和真实项目的
泛型热点分布（标准库 + 依赖）本来就不重叠。

> 所以 README 里那张 `416 → 218（−47.6%）` 的表，**必须**配上
> 「那是合成工程」这个前提才有意义。它证明的是**机制成立**，
> 不是"你的项目也能省 47%"。

---

## 五、复盘：这一轮我错在哪

| # | 我的做法 | 实际是 | 教训 |
|---|---|---|---|
| 1 | 上一轮笔记写「`-Zpolymorphize` 可用」 | 1.100 已移除 | **笔记也会错。`-Z` 开关每次都要问 `rustc -Zhelp`。** |
| 2 | 用 `tr ' ' '\n'` + `grep '^-Copt'` 找 rustc 参数 | rustc 打印的是 `-C opt-level=3`（有空格），grep 全部漏掉，一度以为环境变量注入失效 | 验证脚本本身也会骗人；**先用一个已知会出现的 flag 校准 grep** |
| 3 | `restore()` 用 `with_extension("rs")` 反推备份原名 | `main.rs.xmake-bak` 的 stem 是 `main.rs` → 写到 `main.rs.rs`，**原文件保持坏状态且不报错** | **回滚路径必须写测试。** 已加 `backup_naming_roundtrips` |
| 4 | 替换区间从 `fn` 关键字开始 | 生成 `pub #[cfg(…)] fn`，编译器报 `visibility pub is not followed by an item` | 改写要覆盖**整个 item 头**（`pub` / `unsafe` / `async` / `#[attr]`） |
| 5 | `match_brace` 把 `->` 里的 `>` 当泛型闭合 | `<T: Fn() -> u64>` 和 `fn f(…) -> R {` 都算错，整个函数被漏掉（扫描到 0 个） | 已加 `arrow_does_not_break_brace_matching` 测试 |
| 6 | `reclaimable()` 把 `<# as Trait>::` 也算进去 | 报 95%，实测 47% | 分母要分清「泛型函数体」和「每类型必然一份」 |
| 7 | 测量脚本里 `[ "$cfg" = off ] && rm -f config` 写过一次就再没恢复 | B 组等于没配置，测出"setup 无收益"，其实是脚本 bug | **A/B 分组的准备工作要在每组开头显式做，不能靠"上一组留下的状态"** |
| 8 | 写「`-Zdump-mono-stats` 是死路径」 | 它写在 **crate 根的 `human/`**，我只翻了 `target/` | **"在预期位置没找到" ≠ "没产生"。下结论前先全仓 find 一遍。** |
| 9 | 用 `from_generic`（看名字里有没有 `::<T0>`）当"可 dyn 化"的开关 | rustc 代价模型给的是**裸名** `drive`，这个开关恒为 false → `reclaimable()` 恒等于 0 | 判据要跨数据源成立；**一条主路径上的开关必须有测试覆盖** |
| 10 | `duplicated && is_vtable_slot()` 判断 vtable 标签 | `duplicated()` 定义上已排除 trait 方法，表达式恒假，标签整个消失 | 互斥的两个谓词要**显式写成 if/else if**，或用 `^` 断言互斥（已加测试） |
| 11 | 最终体积测量前，先用手工 `cargo rustc --emit=llvm-ir -Cmetadata=…` 探过路 | 那些调用往 `target/` 塞了 nonce 命名的产物，`target/` 从 4.06 MB 虚涨到 16 MB | **测量动作本身会污染被测对象。体积数字只能来自干净副本。** |
| 12 | `undo` 只撤了配置，help 却写着"还原" | `src/main.rs` 里的 `#[cfg(not(debug_assertions))]` 留在原地 —— 一个说谎的接口 | **承诺可逆就要把两半都做完**，否则不如不写 |
| 13 | `undo` 多了/少了一个换行，我先去"补偿" `render()` 末尾的 `\n` | 真正的错配是 `apply()` 调了 `trim_end()` 而测试没调 —— **测试没镜像真实路径**；补偿改在了错的地方，反而把真实路径弄坏（冒烟里少一个空行） | 偏差出现时先问「我测的和跑的是同一条路径吗」，**别急着改被怀疑的那一方** |
| 14 | 第二次 `undo` 崩在 `读 …\.cargo\config.toml 失败: os error 3` | 第一次 `undo` 刚把整份（全是我们写的）config 删掉，第二次自然读不到 | **幂等**是 `undo` 的必修项：文件缺失要当"无事发生"，不是报错 |
| 15 | 用 `(b[i] as char).is_alphanumeric()` 按字节回溯找标识符 | `0xBA as char` 是 `'º'`（**是**字母），而 `示` = `E7 A4 BA` → 回溯扎进多字节字符中间，真实项目上直接 panic | **`as char` 不能用来判字节属性**；一律用 `is_ascii_*`。玩具里全 ASCII，永远测不出来 |
| 16 | `Opts.paths` 声明了却从没被赋值 | `cargo xmake dynify <path>` **静默忽略**路径，永远退回 `root/src` | 声明了没接线的字段不会被编译器发现；**位置参数要有端到端测试** |
| 17 | 以为 `Path::is_absolute()` 认得 Git Bash 的 `/d/Code/...` | Windows 上返回 `false` → 被当相对路径拼到 root 后 → 静默 0 个文件 | 跨 shell 的路径形态要显式转换 + 测试 |
| 18 | 只在 crate 目录找 mono-stats 报告 | cargo 把 rustc 的 cwd 设成 **workspace 根**，报告落在那里；单 crate 时两者重合才看不出问题 | **"在我以为的位置没找到" 要先去别处找**（同第 8 条） |
| 19 | 把 `std::vec::Vec::<#>::extend_desugared` 这类**库函数**算进"可 dyn 化"，报"预期少掉 44%" | 9616 份里本地只有 **8 份（0.04%）**，其余 9608 份 dynify 结构上就够不着 | **"理论上可优化" ≠ "你能优化"**。给建议前先判"这块归谁管" |
| 20 | 文档写「`-Zdump-mono-stats` 是裸开关，产出 JSON」 | 1.100 上它**带值（值=输出目录）**，且**默认 markdown**，JSON 要再加 `-Zdump-mono-stats-format` | 工具链版本升级会改 `-Z` 的语义；**`-Zhelp` 每次都要重查**（同第 1 条） |

第 7、9、10、11 条是同一条：**读数符合预期时最容易漏掉检查**，
而"看起来像结论"的错数据比没有数据更坏。
第 13 条是它的镜像：**偏差出现时最容易改错地方**，
所以先确认「测试路径 == 真实路径」，再去动那个真正错配的点。
第 15~20 条是第三条：**玩具工程只能证明"机制成立"**。
六条里有五条在合成工程上永远不会出现，而第 19 条更根本 ——
合成工程的**形状本身就是按工具能力设计的**，
所以它给出的收益数字里天然不含"这块地你根本够不着"的那部分。

---

## 六、复现

```bash
# ---- A. 合成对照工程（证明机制成立）----
# 生成对照工程（N=200）
cd experiments/dynproof && DYNPROOF_N=200 python gen.py

# ---- B. 真实项目（证明它到底值不值得用）----
# 一定要在真实项目上跑一遍。真实代码会带中文注释、workspace、多层模块，
# 而这三样恰好是合成工程里没有的，也正是 2.8 那五个 bug 的藏身处。
cd <任意真实项目>
cargo xmake dynify            # 只读预览：能改到你几个函数？（真实项目常常是 84 里只有 1）
cargo xmake audit             # 热点归谁：本地 / 依赖 / 标准库？
                              #   若本地占比 < 1%，工具会直说"不值得做"

# 单态化基线
cd mono && cargo xmake audit

# 数据源选择（默认 auto：stats → mono → ir 逐级降级）
cargo xmake audit --xmk-mode=stats    # nightly，带 rustc 代价模型（首选）
cargo xmake audit --xmk-mode=mono     # nightly，-Zprint-mono-items
cargo xmake audit --xmk-mode=ir       # stable 兜底，--emit=llvm-ir

# 写配置 → 看覆盖率
cargo xmake setup
cargo xmake doctor

# 预览 / 落盘改写
cargo xmake dynify
cargo xmake dynify --rewrite
cargo xmake audit          # drive 应从 200 份变 1 份

# 后悔（配置删行 + 源码折叠回泛型，逐字节还原）
cargo xmake undo
```

工具自身的测试：`cd cargo-xmake && cargo test`（30 项）。
端到端自检：`cargo xmake selftest`。
整条链路回归：`bash scripts/smoke_cargo_xmake.sh`（建临时工程，
`setup → dynify --rewrite → build → undo → 再 undo`，27 项断言）。
