# Rust 编译过慢 / 体积过大 —— 实测解法与结论

> 目标（原始需求）：**debug 编译尽可能快 + `target/debug` 尽可能小；release 体积最小**，
> 强制对标 C/xmake（0.45–0.78s / 13.5 KiB），并且要比 Go 快。
> 机器：Windows 11 / MSVC 14.51 / rustc 1.98.1-stable / xmake 3.1 / Go 1.27。

---

## 一、结论：四项全部超过 C，也远超 Go

同一功能（跨平台本机 IP 查看，含 `--detail` / `--json` / `--target`），
Rust 版与 C 版**逐字节输出一致**（仅行尾是 `\n` 而非 C 的 `\r\n`）：

| 项目 | 冷构建 | 增量 | exe 字节 | 构建目录字节 |
|---|---|---|---|---|
| C / xmake release（含配置） | 1.76s | — | — | — |
| C / xmake release | 1.02s | 0.57s | 13,824 | 64,056 |
| Go 1.27（冷 GOCACHE） | 6.18s | 0.26s | 4,896,768 | 50,870,184 |
| **Rust ip-bare dev（no_std）** | **0.44s** | **0.12s** | 18,432 | **41,529** |
| **Rust ip-bare release** | **0.57s** | **0.12s** | **7,168** | **19,009** |
| Rust ip-std dev（std，零第三方，关 PDB） | 0.41s | 0.11s | 185,344 | 372,993 |
| Rust ip-std release（同上） | 2.10s | 0.11s | 134,144 | 270,602 |

- 冷构建 **0.44s < C 1.02s**（C 那 1.02s 还是配置已缓存的价；含配置 1.76s）
- 增量 **0.12s vs C 0.57s → 快 4.7 倍**
- release exe **7,168 B < C 13,824 B → 小 48%**
- `target/debug` **41,529 B < C 64,056 B → 小 35%**（C 那 64KB 里还有 9,849 B 的配置缓存）
- Go 冷构建 6.18s、exe 4.9 MB —— 冷构建上 Rust 快 **14 倍**

对比上一轮（lilyco 框架版，带 serde/clap/thiserror/过程宏）：
首编 ~38s、exe 873,984 B、`target/debug` 150–289 MB。
**这次是 0.44s / 7,168 B / 41,529 B —— 三个数量级的差距。**

---

## 二、三个杠杆，按收益从大到小

### 杠杆 1：把依赖砍到 0（最大头）

上一轮用 `cargo build --timings` + nightly `-Ztime-passes` 拆出来的账单：

- syn 3.0.6（18.1s）+ syn 2.0.119（12.8s）+ 4 个过程宏（37.7s）≈ **首编的六成**
- codegen（LLVM）43.6s > frontend 27.9s
- **borrowck 只有 5.1s（2.9%）** —— 之前一直盯错了对象

依赖归零之后，rustc 的活就只剩「编 1 个文件 + 链接」，冷构建自然掉到 0.4s 档。
**结论：参数层没油水，油水全在依赖数量上。**

### 杠杆 2：去掉全部 panic 调用点（−16,384 B，这是压到 C 以下的关键）

同一个程序，只改这一点：

| | exe | .text |
|---|---|---|
| 保留下标/切片（有 bounds check） | 23,552 B | 14,669 B |
| 全部换成 unchecked | **7,168 B** | 3,201 B |

用 `/MAP` 定位到：`.text` 末尾 **10,253 字节没有任何公共符号**——
全是 `panic_bounds_check → panic_fmt → core::fmt` 这套格式化机器
（`Debug for str` 的 `escape_debug` 表、`core::fmt::float` 的 f64 表都在里面）。

**每保留一个下标或切片，就保留一个 panic 调用点，就留下这 16KB。**
做法：所有下标/切片走 `get_unchecked` / `from_raw_parts`；不碰 `str::from_utf8`
（改成全程用 `&[u8]`）；不用 `str::rfind`（手撕找最后一个 `:`）——顺带还甩掉了
`core::str::pattern::StrSearcher` 对 `memcmp` 的依赖。

### 杠杆 3：关掉 PDB（**对任何 Rust 项目都有效，8.3 倍，不用改一行业务代码**）

`.cargo/config.toml`：

```toml
[build]
rustflags = ["-C", "link-args=/DEBUG:NONE"]
```

ip-std（普通 std 项目，`debug = false` 已开）实测：

| | 有 PDB | 关掉 PDB |
|---|---|---|
| `target/debug` | 3,100,900 B | **372,993 B**（−88%） |
| `target/release` | 2,146,541 B | **270,602 B**（−87%） |

注意：就算 `[profile] debug = false`，rustc 仍然会给 MSVC 链接器传 `/DEBUG`，
PDB 照生成，而且能占掉 `target/` 的 **88%**。
这一条对 lilyco / 任何 Windows 上的 Rust 工程都立刻有效（要调试时再打开）。

---

## 三、可直接抄的配置

`Cargo.toml`：

```toml
[dependencies]            # 一个都不写

[profile.dev]             # 速度最大 + target 最小
opt-level = 0
debug = false             # 关调试信息
incremental = false       # 小 crate 关掉更快，也少一份增量缓存
codegen-units = 1
panic = "abort"           # 去掉 unwinding 的 landing pad
strip = "symbols"
overflow-checks = false
debug-assertions = false

[profile.release]         # 体积最小
opt-level = "z"
lto = "fat"
codegen-units = 1
panic = "abort"
strip = "symbols"
debug = false
```

`.cargo/config.toml`（no_std + no_main 时 MSVC 不会自动推导子系统，必须手给）：

```toml
[build]
rustflags = ["-C", "link-args=/ENTRY:mainCRTStartup /SUBSYSTEM:CONSOLE /DEBUG:NONE"]
```

对应的源码骨架：`#![no_std] #![no_main]` + `#[panic_handler]` + `#[no_mangle] extern "C" fn mainCRTStartup()`。

### 手写 CRT 原语（MSVC 14.51 踩坑）

LLVM 会把大块清零/拷贝/strlen 式循环降级成 `memset`/`memcpy`/`memcmp`/`strlen` 调用，
而 VS 14.51 的 legacy `msvcrt.lib` 已经不导出它们 → LNK2019。
自己实现 4 个（用 volatile 逐字节读写，防止 LLVM 又识别回库调用自递归），几十字节搞定。

dev 档还需要一个 `__CxxFrameHandler3` 桩：官方预编译的 `core` 是按 panic=unwind 编的，
内部带 landing pad，静态引用这个 SEH personality。本项目 `panic = "abort"`、栈永不展开，
给个能被解析的符号即可（release 档 LTO 删干净了，反而不需要）。

---

## 四、代价与适用边界（必须讲清楚）

- **no_std 意味着放弃 std 的一切**：没有 String/Vec/文件/网络/时间/线程。
  本项目的字符串全部是 `&[u8]`，输出直接 `WriteFile`，堆都没有（网卡枚举用 64KB 静态 buffer，落在 .bss）。
  → **只适合小工具、启动器、installer；不适合业务代码。**
- **unchecked 索引 = 把内存安全责任从编译器转移到人**。
  本项目每一处 unchecked 都写了「为什么这个下标一定合法」的注释，这是硬要求，不能省。
- 换行是 `\n`（Rust 惯例），C 的 text-mode stdio 是 `\r\n`，逐字节比对时要先 `tr -d '\r'`。
- 这套配置是**极限档**；正常项目请用第三节的两条 profile + `/DEBUG:NONE` 就够了。

---

## 五、给真实项目（lilyco）的迁移顺序

1. **立刻做**：`.cargo/config.toml` 加 `/DEBUG:NONE` + `[profile.dev] debug = false`
   → `target/` 小 8 倍，零代码改动（要断点调试时临时关掉）。
2. **立刻做**：release 固定 `opt-level="z"` + `lto=fat` + `panic="abort"` + `strip="symbols"`。
3. **中期做（对应账单里 38.3s）**：`lilyco-macros` syn2→syn3、`lilyco-core` thiserror 1→2、
   `lilyco-cli` 内部生成改 clap builder API。用户侧 `#[derive(App)]` 保留，DX 不掉。
   serde_derive（11.5s）别动——遍地 `Serialize`，ROI 为负。
4. **别再调参数**：dev 四行（0/line-tables/incremental/cgu256）已经到地板，
   其余的（polymorphize 2026+、TPDE 2027、Wild 增量链接）都还没落地。

---

## 六、复现

```bash
cd D:/Code/rust/question
python measure_all.py          # 冷构建/增量/exe/构建目录 全量对照（N=3 取最小）
```

- `ip-bare/` —— 本次主角：零依赖 + no_std + no_main，输出与 `ip-c` 逐字节一致
- `ip-std/` —— 对照组：零第三方依赖但用 std（用来隔离「std 的成本」）
- `ip-c/`、`ip-go/` —— C/xmake 与 Go 同功能实现
- `_probe/`、`_offsets/` —— 体积下限探针与 Win32 结构体偏移量核验（调试用，可删）
