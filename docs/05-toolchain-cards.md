# 还能再快吗：四张没打过的牌

> 2026-10-01 · lilyco（24 成员 / 379 包）· 每轮单变量、隔离 target 目录
> 数据：`experiments/lilyco-measure/speed_cards*.json` + `speed_cards_round*.py`

---

## 结论摘要

| 牌 | 冷 `check` | 冷 `build` | 增量 `check` | 增量 `build` | `target/` | 判定 |
|---|---|---|---|---|---|---|
| **基准** stable 现行配置 | 46.5 s | 91.5 s | 2.98 s | 5.74 s | 871.6 MB | — |
| ① lld-link 链接器 | — | 95.7 s | — | — | ≈ 同 | ❌ **无效** |
| ② 并行前端 `-Zjobs-frontend=8` | 44.0 s（对照 45.4） | — | — | — | — | ❌ **噪声级** |
| ③ **cranelift codegen 后端** | **55.8 s** | **63.5 s** | **2.66 s** | **4.78 s** | 932.3 MB | ✅ **有条件可用** |

**cranelift 相对现行配置：**

```
冷 build   91.5 → 63.5 s   −28.0 s   (−31%)   ✅
增量 build  5.74 → 4.78 s  −0.96 s   (−17%)   ✅
增量 check  2.98 → 2.66 s  −0.32 s   (−11%)   ✅
冷 check   46.5 → 55.8 s   +9.3 s    (+20%)   ❌   ← 注意这是变慢
target     871.6 → 932.3 MB  +60.7 MB (+7%)   ⚠️
```

**一句话：cranelift 是这四张牌里唯一有肉的，但它把「冷 check」拖慢了 9 秒** ——
而冷 `check` 恰恰是我上一份文档里推荐的主力内环命令。所以它不是免费的。

---

## 一、三个失败/无效的牌（也要记下来）

### ① lld-link 链接器：无效

```
A stable（MSVC link.exe）   冷 build 93.9 s
B stable + linker="lld-link" 冷 build 95.7 s     ← +1.8 s，噪声
```

lld 确实生效了（构建成功、无 PDB、target 体积一致），但**没有收益**。
原因很直白：lld 只加速**链接阶段**，而 lilyco 的 91 s 里有 88 s 是编译 252 个 crate 的 codegen，
链接只占几秒。**分母太小，分子再大也没用。**

> 这一条对 Tauri 桌面项目要**分开看**：桌面项目在链接上花的比例更高（一个大二进制 + 资源），
> 但本次实测的 lilyco 是多二进制 CLI，链接占比低。

### ② 并行前端：噪声级

cargo 已经**跨 crate 并行**了，前端并行是**crate 内部**的并行，边际很小：

```
C2 nightly 对照         冷 check 45.4 s   Compiling 64 行 / Checking 188 行
D2 + --jobs-frontend=8  冷 check 44.0 s   Compiling 63 行 / Checking 189 行
```

−1.4 s（−3%），落在抖动里。**不值得开。**

### ③（附带）nightly 本身：与 stable 无差异

`nightly 基准` 冷 check 45.4–49.5 s vs `stable` 46.5–47.2 s。**工具链本身不是杠杆。**

---

## 二、🔴 必须坦白：第一轮测出的是假象

第一轮（**共用同一个 `target/` 目录**）的结果是：

| 变体 | 冷 check | 冷 build |
|---|---|---|
| A stable | 47.2 s | 93.9 s |
| E nightly + cranelift | **37.5 s** | **46.8 s** |
| 看起来的差值 | −9.7 s | **−47.1 s（−50%）** |

第四轮（隔离 `CARGO_TARGET_DIR`）的真实值是 **−28.0 s（−31%）**，而且冷 check 是 **+9.3 s（变慢）**。
**第一轮把 cranelift 的收益夸大了 1.7 倍，还把 check 变慢错读成了变快。**

### 根因：stable 与 nightly 的产物布局不同 → `cargo clean` 清不干净

实测（最小工程验证）：

```
stable  1.98.1 :  target/debug/deps/layout_probe.exe        ← deps/ 存在（旧布局）
nightly 1.100  :  target/debug/layout_probe.pdb            ← deps/ 不存在（新布局）
```

nightly cargo 1.100 已默认启用新的 build-dir 布局（`cargo -Z help` 里的
`-Z build-dir-new-layout`，在 nightly 上是默认行为）。于是：

1. A（stable）在旧布局下产出 379 个 crate 的产物，其中 **proc-macro 和 build script 是编译好的二进制**
2. 轮到 E 时，nightly 的 `cargo clean` 按**新布局**清理 —— **清不到 A 留在旧布局里的产物**
3. E 于是直接**复用**了 A 编译好的 proc-macro / build-script → 白捡几十秒

**教训：跨工具链做对照，必须给每个变体独立的 `target` 目录。**
「`cargo clean` 就干净了」这个假设，在工具链不同的前提下不成立。

---

## 三、🔴 另一个我要认的错：体积口径重复计数

旧布局下，cargo 会把 `deps/` 里的最终产物**以硬链接放到 `target/debug/` 顶层**：

```
target/debug/deps/liblilyco_core-<hash>.rlib     ← 真身
target/debug/liblilyco_core.rlib                 ← 硬链接，同一份数据
```

我此前按「遍历所有文件、累加 `st_size`」统计体积 —— **同一份硬链接被算了两遍**。

改用 `(st_dev, st_ino)` 去重后：

| 口径 | target/ |
|---|---|
| 朴素累加（此前发布的值） | 935.3 MB |
| **硬链接去重（真实值）** | **871.6 MB** |
| 虚高 | **+63.7 MB（+6.8%）** |

⇒ **lilyco 的 `target/` 真实值是 871.6 MB，不是 935.3 MB。** 之前文档里的数字要按这个修正。
（同理，`硬地板 470 MB` 那个数也是朴素口径，真实下限更低。）

**这就是我自己反复讲的那条纪律：「读数不一致时，先怀疑自己的口径」。结果我自己的口径就是错的。**
正确写法：

```python
seen = set(); total = 0
for f in target.rglob("*"):
    if not f.is_file(): continue
    st = f.stat()
    key = (st.st_dev, st.st_ino)      # ← Windows 上 st_ino 是 NTFS file index，硬链接同值
    if key in seen: continue
    seen.add(key); total += st.st_size
```

---

## 四、cranelift 怎么用（以及一个会让 CI 挂掉的坑）

### ⛔ 不要写进 `Cargo.toml` 的 `[profile.dev]`

```toml
# 千万别这样写
[profile.dev]
codegen-backend = "cranelift"
```

实测 **stable cargo 1.98.1 会在解析 manifest 阶段硬报错**：

```
error: failed to parse manifest at `Cargo.toml`
Caused by:
  feature `codegen-backend` is required
  The package requires the Cargo feature called `codegen-backend`, but that feature
  is not stabilized in this version of Cargo (1.98.1)
```

**它不是"忽略这个键"，是"整个 manifest 解析失败"** ——
只要 `Cargo.toml` 里有这一行，任何用 stable 的人（包括你的 CI）都编不动这个项目。
`--config` 传入同理（它也会要求 `cargo-features = ["codegen-backend"]` 声明，那也是 manifest 改动）。

### ✅ 正确用法：调用时加 rustc 标志（已验证生效）

```bash
RUSTFLAGS="-Zcodegen-backend=cranelift -Clink-args=/DEBUG:NONE" cargo +nightly build
```

`/DEBUG:NONE` 必须**一起带上** —— 因为 `RUSTFLAGS` 环境变量的优先级高于
`.cargo/config.toml` 里的 `[target.*] rustflags`，带上它才不会把这项优化丢掉。

验证它真的生效（看 rustc 命令行）：

```bash
RUSTFLAGS="-Zcodegen-backend=cranelift -Clink-args=/DEBUG:NONE" \
  cargo +nightly build -p lilyco-core -v 2>&1 | grep -o "codegen-backend=[a-z]*"
# 输出：codegen-backend=cranelift
```

建议做成一个 alias / 包装脚本，**不要**改成默认配置 —— 理由见下。

### 为什么它只适合"手动 opt-in"

| 限制 | 后果 |
|---|---|
| **不支持 `lto = "fat"`** | release 必须留在 LLVM。所以它只能是 **dev-only** |
| 需要 nightly | 团队/CI 不能默认依赖 |
| 写进 manifest 会让 stable 解析失败 | ⛔ 会把 CI 直接搞挂 |
| dev 二进制运行性能下降 | cranelift 的 codegen 质量低于 LLVM，运行时更慢 |
| `target/` +60.7 MB | 磁盘紧张的话要权衡 |

### 值不值得用：看你的痛点在哪

| 你的痛点 | cranelift 的收益 | 结论 |
|---|---|---|
| 「`cargo clean` 后等 90 秒」 | −28 s（−31%） | ✅ 值得 |
| 「改一行等 3 秒」 | −0.3 s（−11%） | ⚠️ 只有 0.3 秒，但要上 nightly，不划算 |
| 「CI 太慢」 | 无（release 用不了） | ❌ 帮不上 |

**我的建议：给 lilyco 保留现状。** 因为：
① 主力内环是 `cargo check`（2.98 s），cranelift 只省 0.3 s，而且要付 nightly 的代价；
② 冷构建 91 s 一个月才几次，为它换 nightly 不划算；
③ 真正每天受益的那一刀（`default-members` 865→379 包）已经落地了。
**但如果你哪天要连续做很多次全量冷构建，这条命令值得贴在手边。**

---

## 五、顺带补齐：xmake 的 no-op 那半秒到底是什么

之前测出「xmake 什么都没改也要 547 ms，Rust 只要 159 ms」，现在把它的构成拆开了：

| | 纯启动 | 剩余（读构建状态） | no-op 合计 |
|---|---|---|---|
| xmake 3.1 | **262 ms** | ~285 ms | 547 ms |
| cargo 1.98 | **155 ms** | ~4 ms | 159 ms |

（纯启动 = `xmake --version` / `cargo --version`，各 5 次取中位，独占 CPU）

⇒ **cargo 的"空载"几乎只有进程启动成本；xmake 每次要跑一遍 Lua VM 把 `xmake.lua` 执行完
才能得出构建图。** 这半秒与项目大小无关，是小项目上 xmake 永远甩不掉的门票钱。

---

## 六、复盘：这一轮我错在哪

| # | 错误 | 纠正 |
|---|---|---|
| 1 | 三个工具链共用一个 `target/`，而稳定版/夜间版产物布局不同 | 每个变体给独立 `CARGO_TARGET_DIR` |
| 2 | 体积在 6 轮增量之后才测，把 `incremental/` 历史算进去了 | 只在冷构建刚结束时测 |
| 3 | 按文件大小累加统计体积，硬链接被算两遍 | 用 `(st_dev, st_ino)` 去重 |
| 4 | 在后台构建期间测 `--version` 启动开销，出现 2.4× 抖动 | 同机测量必须独占 CPU，作废重测 |
| 5 | 一度以为「nightly 比 stable 省 237 MB 磁盘」 | 那是布局差异，不是真的省 —— 不能算作收益 |

**这五条里有四条是同一个病根：把一个"看起来对"的对照当成结论，
没有先用最小实验去验证前提（布局是否相同 / 有没有残留 / 计时是否受干扰）。**
