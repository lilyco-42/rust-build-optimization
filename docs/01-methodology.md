# 优化思路：分层决策

> 本文讲**怎么想**，`00-experiment-log.md` 讲**测出了什么**。
> 所有数字都有原始数据支撑，出处见实验台账。

---

## 一句话方法论

**先分清你要优化的是「编译量」还是「编译产物」，再顺着五层阶梯往下走 ——
每层做完先验证，通过了才进下一层。**

大多数人一上来就调 `opt-level`，那其实是第 2 层的东西，
而且实测它连「速度 vs 体积」的权衡都经常是假的（见下文反直觉 1）。

---

## 认知前提：成本在依赖，不在你的代码

| 项目 | 业务代码 | 依赖包数 |
|---|---|---|
| Tauri 最小桌面项目 | ~30 行 | **291** |
| lilyco 工作区 | 24 个 crate | **865** |

**业务代码的规模对构建时间和 `target/` 体积几乎无影响。**
所以第一反应永远是「依赖树长什么样」，而不是「我能怎么改代码」。

---

## 🔴 最重要的一张表：四个目标，四套杠杆，互不重叠

优化失败最常见的原因不是手段错了，而是**目标没分清**。这四个目标看着像一件事，其实杠杆完全不同：

| 你想优化 | 真正的杠杆 | 在这里无效甚至有害的 |
|---|---|---|
| **开发内环速度**（改一行看结果） | `default-members`、feature gate、`incremental = true` | 调依赖 `opt-level` —— 依赖**不重编**，改了白改 |
| **`target/` 磁盘占用** | `/DEBUG:NONE`、`debug = false`、去掉 `staticlib`/`cdylib` | 加 `lto` —— `lto` 反而让 `target/` 更大 |
| **产物大小**（发给用户的 exe） | 依赖 `opt-level = "z"`、`lto = "fat"`、`strip` | `debug = false` —— 对 release 产物零影响 |
| **冷构建 / CI 时间** | `sccache`（上限 1.84×）、减少依赖数 | `/DEBUG:NONE` —— 对用时几乎无影响 |

**混着做会互相抵消。** 最典型的反例是 `dev-lean`（`opt-level="z"` + `lto`）：
它能把你自己的 exe 压到 2.9 MB，但 `target/` **反而更大**（1,522 MB > 1,511 MB），
因为它破坏了增量编译 —— 想省硬盘的人用了它会更难受。

---

## 核心区分：砍「编译量」 vs 砍「编译产物」

这是整套方法里最有用的一条，值得单独强调。

### 砍编译量 = 少编东西 → 快了按比例

- 手段：`default-members`、把重依赖标 `optional`、删掉不用的 crate
- 特征：**改一行配置，收益立刻按包数比例体现**
- 实测：lilyco `865 → 379` 包（−56%），一行 `default-members`

### 砍编译产物 = 编了但不留在磁盘 → 省空间，一秒钟都不快

- 手段：`/DEBUG:NONE`（PDB 归零）、`debug = false`、去 `staticlib`/`cdylib`
- 特征：**能省几百 MB，但对速度零贡献**（甚至因为要去符号而略慢）
- 实测：Tauri `4,135 → 1,014 MB`，冷构建反而 **+43 s**

**两者可以叠加，但必须分开测。** 混在一起测，你永远不知道是哪一刀起的作用，
也就无法在换项目时迁移经验。

---

## 五层阶梯

### 第 0 层 · 先量，别猜

不量就别改。这一步能定位 80% 的问题。

```bash
# ① 依赖图：谁把包数拉起来的
cargo metadata --format-version 1 > "$TEMP/meta.json"
# 然后做可达性分析 —— 算出「排除某个 crate 能省多少包」（脚本见 SKILL 或 scripts/）

# ② target 构成：到底谁在占地方
du -sm target
find target -name "*.rlib"  -printf "%s\n" | awk '{s+=$1} END {printf "rlib:  %.1f MB\n", s/1048576}'
find target -name "*.rmeta" -printf "%s\n" | awk '{s+=$1} END {printf "rmeta: %.1f MB\n", s/1048576}'
find target -name "*.pdb"   -printf "%s\n" | awk '{s+=$1} END {printf "pdb:   %.1f MB\n", s/1048576}'
find target -name "*.lib"   -printf "%s\n" | awk '{s+=$1} END {printf ".lib:  %.1f MB\n", s/1048576}'
```

判读要点：
- `pdb` 大 → 第 2 层有效
- `.lib` 有 200 MB 上下 → 有 `staticlib`/`cdylib`，第 3 层有效
- `rlib + rmeta` 就是**硬地板**，别指望绕过
- 包数 500+ 且没有 `default-members` → 第 1 层收益最大

### 第 1 层 · 砍编译量：工作区结构（性价比最高）

- **`default-members`** —— 实测 lilyco 一行的收益是 486 个包（56%）
- **feature gate** —— 把 `wgpu`/`tauri`/编辑器内核这类重依赖标 `optional`，
  别让核心 crate 拖它们进来
- **拆工作区** —— 单个 crate 独占 200+ 包时，考虑把它独立出去

为什么放第 1 层：**改一行配置，直接省时间**，风险接近零（`-p` 和 `--workspace` 都不受影响）。

### 第 2 层 · 砍产物：零代码改动

- **`/DEBUG:NONE`** —— 单刀最大，`target/` −179 MB
- **`debug = false`** —— −82 MB
- **依赖 `opt-level = "z"`** —— −113 MB，且**比 `1` 更快**（见反直觉 1）

这一层完全不碰业务代码，且收益可逆、可测量。

### 第 3 层 · 结构取舍：需要判断

- **去掉 `staticlib` / `cdylib`** —— −183 MB。但**跑移动端要改回来**
- **拆工作区** —— 把重依赖变成独立仓库/工作区

这一层可能影响功能，改之前要确认。

### 第 4 层 · 改代码：最后手段，最贵

- 手写 `Serialize` 换掉 `serde(derive)` —— 实测冷编译 **−48%**，
  因为 proc-macro（`serde_derive` + `syn` + `proc-macro2`）**铺在串行关键路径上**
- 丢掉 `thiserror` —— 只给一个 `Display` impl，却拖进 `thiserror-impl` + build script。
  `core::error::Error` 自 Rust 1.81 已在 core 稳定，手写约 15 行替代
- 消灭 panic 调用点 —— 体积的终极杠杆，但必须逐处 `unsafe` 并注释清楚下标为何合法

**只有前面四层都做完了还不够，才动这一层。**

---

## 四条反直觉（都有实测数据）

**① `opt-level = "z"` 比 `opt-level = 1` 更快，同时更小。**
176.6 s vs 201.1 s，1,014 MB vs 1,069 MB。`z` 跳过向量化/循环展开等昂贵 pass。
**「体积优化必然更慢」这个直觉是错的。** 顺带：`opt-level = 2` 是双输（最慢 249.7 s 又不小）。

**② 即使 `[profile.dev] debug = false`，rustc 仍会给 MSVC 传 `/DEBUG`，PDB 照产。**
必须显式加 `link-args=/DEBUG:NONE`。这个坑不踩一次想不到。

**③ `[profile.dev.build-override]` 对 build script 产物基本无效。**
实测 1,132 → 1,135 MB，**反而略增**。真正的杠杆在链接器层。

**④ `#![no_std]` 不是性能特性。**
单变量实测（同源代码体只差一行）：冷构建 7673 vs 7786 ms（−1.4%，噪声级），
**exe 字节数完全相同**。它的价值在可移植性（裸机/嵌入式/无 libc），
**别为了速度或体积去做 no_std**。

---

## 验证闭环（必做，跳过一定会吃亏）

**「没报错」不等于「生效了」。** 本次实验踩过两次：

| 踩的坑 | 表现 |
|---|---|
| sccache 缓存上限写进配置文件 | sccache **不读**该文件，把值写成非法字符串它也不报错，照样用 10 GiB 启动 |
| `~/.cargo/config.toml` 里的 `[profile.release]` | 完全无效，cargo 只在**工作区根** `Cargo.toml` 读 profile |

所以每改一项，必须用工具**自身**的输出复核：

```bash
cargo build 2>&1 | tail -2 | grep -v debuginfo   # 看编译状态串
du -sm target                                     # 看体积
sccache --show-stats | grep -i "max cache"        # 看缓存上限
cargo metadata --no-deps --format-version 1 | grep default_members  # 看工作区默认成员
```

---

## 什么时候该停

`rlib + rmeta` 就是硬地板 —— 那是所有依赖的编译产物本身，删了就没法增量编译。

实测 Tauri（291 依赖）：`rlib 512 + rmeta 332 = 844 MB`。
4,135 MB 的优化空间里，最后 844 MB 谁也动不了。

**看到 `target/` 已经贴近 rlib+rmeta 之和，就该停手了**，
再往下只能靠「减少依赖数」或「拆工作区」，那是第 1 层的事，要回到阶梯顶部重新想。
