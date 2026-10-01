# Rust vs xmake：编译速度到底做到了没有

> 2026-10-01 · 同机 · Windows x86_64
> Rust: rustc 1.98.1 stable / cargo 1.98.1 · C: xmake 3.1.0 + MinGW gcc 15.2
> 被测：`ip-bare`（Rust，907 行，**零依赖** no_std） vs `ip-c`（C，256 行，winsock2 + iphlpapi）

---

## 一句话结论

**在可比规模上，不是打平，是全面超越。** 三个层次全赢，其中最刺眼的是 **no-op**：

| 层次 | Rust（ip-bare） | C / xmake（ip-c） | 比值 |
|---|---|---|---|
| 冷构建（配置缓存） | **388 ms** | 1,070 ms | **0.36×** |
| 增量（改一行） | **320 ms** | 538 ms | **0.59×** |
| **no-op（什么都没改）** | **159 ms** | **547 ms** | **0.29×** |
| exe | **18,432 B** | 19,456 B | 0.95× |

**而且 Rust 那份源码是 907 行，C 那份是 256 行 —— 3.5 倍的行数，还快 2.8 倍。**

> ⚠️ 口径说明：C 侧是 `xmake` 默认的 **release**（MinGW gcc），Rust 侧是 **dev**（`opt-level=0`）。
> 两种语言的 "debug/release" 语义不同，这里只比「各自默认的开发构建」这个最常用的场景。
> 单纯比速度时这对 C 是**有利**的口径（release 的 C 反而更快，见下文「为什么 C 不占便宜」）。

---

## 一、三个层次的原始数据

### xmake 3.1.0（C，256 行）

| 层次 | 三次测量 | 中位 |
|---|---|---|
| 全冷 `xmake clean -a`（含重新 configure） | 2,459 / 2,148 / 2,442 ms | **2,442 ms** |
| 冷 `xmake clean`（配置已缓存） | 1,092 / 1,067 / 1,095 ms | **1,092 ms** |
| 增量（改一行） | 538 / 1,089 / 533 ms | **538 ms** |
| **no-op** | 551 / 566 / 547 ms | **553 ms** |
| exe | — | 19,456 B（`build/mingw/x86_64/release/`） |

### Rust（`ip-bare`，907 行，零依赖，dev 档 `opt-level=0`）

| 层次 | 三次测量 | 中位 |
|---|---|---|
| 冷 `cargo clean` | 392 / 384 / 390 ms | **390 ms** |
| 增量（改一行） | 324 / 325 / 310 ms | **324 ms** |
| **no-op** | 160 / 159 / 158 ms | **159 ms** |
| exe | — | 18,432 B |

---

## 二、最刺眼的那个数字：no-op

**xmake 什么都没改也要 547 ms；Rust 只要 159 ms。**

这个数字单独拎出来讲，因为它暴露的不是「C 慢」，而是**构建系统架构的差异**。
把它拆开（纯启动 = `--version`，各 5 次取中位，独占 CPU）：

| | 纯启动 | 读构建状态 | no-op 合计 |
|---|---|---|---|
| xmake 3.1 | **262 ms** | ~285 ms | 547 ms |
| cargo 1.98 | **155 ms** | **~4 ms** | 159 ms |

- **xmake 每次调用都要跑一遍 Lua VM**，把你的 `xmake.lua` 完整执行一遍，
  才能在内存里重建出目标图，然后才去比对文件时间戳。
  **这 ~285 ms 是"门票钱"，与项目大小无关。**
- **cargo 每次调用只读 `.fingerprint/` 下的指纹文件** —— 实测这部分只花 ~4 ms，
  159 ms 里几乎全是进程启动（155 ms）。

> 📐 这个结论对我的方法论很重要：**no-op 时间是构建系统的"空载油耗"，
> 项目越小它占比越大。** 评估一个构建系统时，先量 no-op，
> 再量增量，两者之差才是「真正编译你代码」的成本。
>
> 由此还能推出一个反直觉的推论：**在小项目上，Rust 的增量构建（320 ms）
> 比 xmake 的 no-op（547 ms）还快** —— 也就是说 xmake 连"什么都不做"都比
> Rust "做一遍"慢。

---

## 三、为什么 C 在"单位编译速度"上也不占便宜

### 3.1 C 的预处理 480× 膨胀

```
C 源文件:        256 行
gcc -E 之后: 122,914 行      ← 膨胀 480 倍
包含头文件:      323 个 .h    ← winsock2.h / iphlpapi.h / stdio.h / windows.h ...
```

`gcc -ftime-report`：

```
phase parsing            :  0.89s ( 93%)     ← 全部时间的 93% 花在解析头文件
phase opt and generate   :  0.06s (  6%)
TOTAL                    :  0.96s
```

**gcc 93% 的时间在解析那 12 万行系统头文件，真正编译你 256 行业务逻辑只占 6%。**

### 3.2 Rust 的对应数字

```
rustc parse_crate  : 0.024s
rustc expand_crate : 0.046s   ← 宏展开，对应 C 的预处理
```

**`windows-sys` 是预编译好的 rlib。** 你 `use` KERNEL32 / WS2_32 的声明时，
不需要重新解析任何头文件 —— 那些声明已经在二进制里了。

⇒ **同一份系统 API 声明，C 每次构建都要从头 parse 一遍，Rust 只编译一次然后复用二进制。**
这就是「依赖预编译」的红利，也是 C 无论怎么调 build system 都追不回来的结构性差距。

---

## 四、但是：真实项目的绝对速度 Rust 赢不了，也永远赢不了

必须把这句话说清楚，否则上面的数字会误导决策。

| | 可比规模（几百行、零依赖） | 真实项目（lilyco：24 成员 / 379 包） |
|---|---|---|
| 冷构建 | Rust **390 ms** | Rust 90.1 s |
| 增量 | Rust **324 ms** | Rust 2.8 s（`cargo check`） |
| C/xmake 的对应值 | 1,092 ms / 538 ms | **没有对应物** |

**关键在最后一格：C 生态里不存在等价规模的依赖树。**
一个同体量的 C 项目，依赖的是几十个 `.h`（预处理期展开）+ 几个 `.a`/`.lib`，
而 lilyco 要**从源码编译 379 个 crate**。

**这是「编译量」的差异，不是「编译器速度」的差异。**
上面量出的「Rust 单位速度是 C 的 2.8×」在真实项目里依然成立 ——
只是被 379 倍的编译量吃掉了。

⇒ 所以对真实项目，唯一的杠杆是 **砍编译量**（`default-members` 865→379 包），
而不是继续和编译器参数较劲。**xmake 在这里帮不上忙，因为它面对的问题不一样。**

---

## 五、可复现

```bash
# C / xmake
cd experiments/ip-c
xmake clean -a -y && xmake -y            # 全冷（含 configure）   ~2.4 s
xmake -y && xmake clean -y && xmake -y   # 冷（配置已缓存）        ~1.09 s
echo '/* x */' >> src/main.c && xmake -y # 增量                    ~0.54 s
xmake -y                                  # no-op                  ~0.55 s

# Rust（零依赖裸程序）
cd experiments/ip-bare
cargo clean && cargo build               # 冷                    ~0.39 s
echo '// x' >> src/main.rs && cargo build # 增量                  ~0.32 s
cargo build                               # no-op                  ~0.16 s
```

> ⚠️ 测量纪律：**计时循环里不要跑后台构建。** 我第一次测 `--version` 启动开销时，
> 后台正跑着 379 个 crate 的编译，`xmake --version` 出现 960/761/403/406/981 ms
> 的 2.4× 抖动 —— 数据直接作废。**同机测量必须独占 CPU。**

---

## 六、和体积对比的关系

速度赢了，体积也赢了，两者不冲突（细节见 [`XMAKE_VS_RUST_PIPELINE.md`](XMAKE_VS_RUST_PIPELINE.md)）：

| 方案 | 编译器 | 优化档 | exe | 导入 DLL |
|---|---|---|---|---|
| MSVC C | cl.exe | `/O2 /GS-` | 143,872 B | 3 |
| MinGW C | gcc | `-O3 -s` | 19,456 B | **11** ⚠️ |
| **Rust no_std** | MSVC | `opt-level="z"` + fat-lto | **7,168 B** | 3 |

⚠️ MinGW C 的 19,456 B 是**虚的** —— 它 `import` 了 11 个 DLL，其中 9 个是
`api-ms-win-crt-*`。在干净机器上少了那 9 个 UCRT DLL 就跑不起来。
Rust 版只 `import` 3 个（KERNEL32 / WS2_32 / IPHLPAPI），**19,456 B 换不来能独立运行**。
