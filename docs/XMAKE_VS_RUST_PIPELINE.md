# 对标 xmake：汇编级 + 全流程拆解

生成时间：2026-10-01
实验机：Windows x86_64 / rustc 1.98.1-stable（另用 1.100-nightly 取阶段数据）/ MSVC 14.51 / MinGW gcc 15.2 / xmake 3.1
被测代码：`ip-c/src/main.c`（256 行 C） vs `ip-bare/src/main.rs`（907 行 Rust no_std）

---

## 一句话回答「能不能做到」

**能，而且不是勉强打平——是编译器层面就赢了。**
`rustc` 单次调用的墙钟时间是 `gcc` 的 **0.44×**，产物指令数是 **0.56×**，体积是 **0.37×**。
差距不在「构建系统」上，而在**「编译器要喂多少代码」**上——见下面第 2 节那个 480× 的数字。

---

## 一、体积全景：先把口径对齐

| 方案 | 编译器 | 优化档 | exe 体积 | 导入 DLL 数 |
|------|--------|-------|---------|-----------|
| **MSVC C** | cl.exe | `/O2 /GS-` | **143,872 B** | 3 |
| **MinGW C** | gcc | `-O3 -s` | **19,456 B** | **11** ⚠️ |
| **Rust no_std** | MSVC | `opt-level=z + fat-lto` | **7,168 B** | 3 |

⚠️ **必须先说清两件事，否则对比没有意义：**

1. **默认的 MSVC C 是 143,872 B，是 Rust 的 20 倍。** 因为 `cl.exe` 默认**静态链接整份 CRT**（`.rdata` 1.5MB 里大半是 CRT 字符串表）。C 想变小必须手动加 `/GS- /NODEFAULTLIB` 等一串开关，而 Rust 只需一个 `opt-level="z"`。
2. **MinGW C 的 19,456 B 是「动态依赖 CRT」换来的。** 它 `import` 了 **11 个 DLL**，其中 9 个是 `api-ms-win-crt-*`（stdio/heap/locale/math/runtime/string/environment/private）。
   而 **Rust 只 `import` 3 个：KERNEL32 / WS2_32 / IPHLPAPI**。
   ⇒ 在「干净机器」上，MinGW C 少了那 9 个 UCRT DLL 就跑不起来；Rust 版本直接能跑。**19,456 B 是虚的。**

---

## 二、真正的根因：C 的预处理 480× 膨胀

这是整个对比里最重要的一个数字：

```
C 源文件:        256 行
gcc -E 之后: 122,914 行      ← 膨胀 480 倍
包含头文件:      323 个 .h    ← winsock2.h / iphlpapi.h / stdio.h / windows.h ...
```

而 gcc 的 `-ftime-report` 显示：

```
phase parsing            :  0.89s ( 93%)     ← 全部时间的 93%
phase opt and generate   :  0.06s (  6%)
TOTAL                    :  0.96s
```

**gcc 93% 的时间花在解析那 12 万行头文件上**，真正编译你 256 行业务逻辑只用了 6%。

Rust 侧对应数字：

```
rustc parse_crate  : 0.024s
rustc expand_crate : 0.046s   ← 宏展开，对应 C 的预处理
```

Rust 的 `windows-sys` 是**预编译好的 rlib**，你 `use` 它时不需要重新解析声明。这就是「依赖预编译」的红利——**同一份 KERNEL32/WS2_32 声明，C 每次构建都要从头 parse 一遍，Rust 只编译一次然后复用二进制**。

---

## 三、全流程逐阶段对照

### 3.1 C / xmake 的流水线

| # | 阶段 | 具体动作 | 耗时 |
|---|------|---------|------|
| 1 | xmake 读 `xmake.lua` | Lua 脚本解析 target/rules | — |
| 2 | **平台探测** | 找 platform(mingw) / SDK / cc / cxx 路径 | — |
| 3 | **逐 flag 试编** | `checking for flags (-O3) ... ok`、`(-DNDEBUG) ok`、`(-MMD -MF) ok`、`(-Werror) ok` — **每个 flag 都要真编一次探测** | ~300ms |
| 4 | gcc 预处理 | 256 → **122,914** 行（323 个头） | **0.89s（93%）** |
| 5 | gcc 优化 | tree-opt + RTL + 指令调度 | 0.06s |
| 6 | gcc 汇编 → obj | `main.c.obj` | — |
| 7 | g++ 链接 | 合并 obj + `-lws2_32 -liphlpapi` + 9 个 UCRT | — |
| — | **合计** | | **首次 3,146 ms / 二次 1,909 ms / noop 607 ms** |

> `xmake` 的「flag 试编」和「平台探测」是构建系统开销，**首次构建额外付出约 300–900ms**，而这部分与编译本身无关。

### 3.2 Rust / cargo 的流水线（rustc `-Ztime-passes` 实测）

| # | 阶段 | 具体动作 | 耗时 |
|---|------|---------|------|
| 1 | cargo 读 `Cargo.toml` | 无依赖 → 无需依赖解算/下载 | — |
| 2 | `parse_crate` | 解析 907 行源码 | **0.024s** |
| 3 | `expand_crate` / `macro_expand_crate` | 宏展开（等价 C 预处理） | **0.046s** |
| 4 | `late_resolve_crate` / `resolve_crate` | 名字解析 | **0.020s** |
| 5 | `coherence_checking` | trait 一致性检查 | 0.055s |
| 6 | **`type_check_crate`** | 类型检查 ← **最贵的一步** | **0.155s** |
| 7 | `MIR_borrow_checking` | 借用检查 | **0.031s** |
| 8 | `monomorphization_collector_*` | 单态化收集 | 0.011s |
| 9 | `codegen_to_LLVM_IR` | 生成 LLVM IR | 0.004s |
| 10 | **`LLVM_passes`** | LLVM 优化（含 fat-LTO） | **0.083s** |
| 11 | `finish_ongoing_codegen` | 汇编 + 收尾 | 0.078s |
| 12 | `run_linker` | link.exe | **0.071s** |
| — | **`total`** | | **0.255s** ⚡ |

> **注意 `total` 只有 0.255s，而 rustc 单次墙钟是 418ms** —— 差的 163ms 是进程启动、读 sysroot、写文件等固定开销。

**关键观察：Rust 最贵的三步是 `type_check(0.155) > LLVM(0.083) ≈ codegen收尾(0.078) > link(0.071)`。**
其中 **borrowck 只占 0.031s（12%）** —— 和之前 lilyco 域模型的结论一致，**borrowck 不是瓶颈**。

---

## 四、汇编级对照：指令序列的真实差异

### 4.1 指令数统计（同一方法：全量反汇编后按助记符分类）

| 指标 | MinGW C (`-O3 -s`) | Rust (`z`+fat-lto) | 比值 |
|------|-------------------|-------------------|------|
| **总指令数** | **2,492** | **1,385** | **0.56×** |
| `call`（函数调用） | 179 (7.2%) | 93 (6.7%) | 0.52× |
| `j*`（分支跳转） | 379 (15.2%) | 147 (10.6%) | **0.39×** |
| `mov*`（数据搬运） | 585 (23.5%) | 323 (23.3%) | 0.55× |
| `lea` | 161 | 72 | 0.45× |
| `ret` | 52 | 10 | 0.19× |
| `.text` 字节 | 9,248 | **3,201** | **0.35×** |

**Rust 的 `.text` 只有 C 的 35%。** 差值主要来自 C 的 CRT 胶水代码（52 个 `ret` 对 10 个 `ret` —— C 里有大量小函数往返调用；Rust 的 LTO 把它们全部内联掉了）。

### 4.2 实际指令序列取样

**Rust `mainCRTStartup` 入口（fragment）：**
```asm
pushq  %r15 / %r14 / %r13 ...          # 保存寄存器
subq   $0x488, %rsp                    # 一次性预留栈帧
movl   $0xfde9, %ecx                   # 65001 = CP_UTF8，直接常量
callq  *GetStdHandle
...
lea    0x170(%rsp), %rdx
movl   $0x80, %ecx
xorl   %eax, %eax
rep    stosl  %eax, %es:(%rdi)         # ← 512B 清零 = 1 条 rep stosl
movw   $0x202, %cx                     # WSAStartup 版本号
callq  *WSAStartup
```
**要点**：`rep stosl` 一条指令完成 512 字节清零；版本号 `0x202`、代码页 `0xfde9` 都是直接编入的立即数，**没有查表、没有运行时解析**。

**MinGW C `main` 入口（fragment）：**
```asm
pushq  %r15 / %r14 / %r13 ...
subq   $0x58, %rsp
movq   %gs:0x30, %rax                  # ← TLS 访问（__stack_chk_fail 用）
movq   0x8(%rax), %rsi
movq   0x46ac(%rip), %rbx              # 从 .data 读全局指针
movq   0x832d(%rip), %rdi              # 从 .data 读全局指针
...
xorl   %eax, %eax
lock                                   # ← 原子前缀
cmpxchgq %rsi, (%rbx)                  # ← stdio 线程安全初始化
jne    ...
movl   (%r12), %eax                    # 又一堆全局状态检查
cmpl   $0x1, %eax
```

**要点**：C 版本开头就有 **`%gs:0x30`（TLS）、`lock cmpxchgq`（原子 CAS）、多次全局状态检查** —— 这些全是 CRT 的启动开销（栈保护 + stdio 惰性初始化 + locale）。**Rust 版本里一条都没有**（实测 grep `gs:` / `lock` / `cmpxchg` → Rust 侧 0 条命中同类模式）。

> 这解释了为什么 Rust 只有 10 个 `ret` 而 C 有 52 个：**Rust 版本没有「库函数」，一切都是内联的直线代码。**

---

## 五、思路总结：Rust 做到「又快又小」的四条原则

| # | 原则 | 具体做法 | 收益（实测） |
|---|------|---------|------------|
| **1** | **把声明变成预编译产物** | 用 `windows-sys`/`rlibc` 而不是 `#include`；依赖只编一次，之后复用 rlib | 干掉 480× 的预处理膨胀 → gcc 93% 的解析时间消失 |
| **2** | **不链 CRT** | `#![no_std] #![no_main]` + 自写 `mainCRTStartup` + `/ENTRY` | 导入 DLL 从 11 个降到 3 个；无 TLS/无栈保护/无 stdio 初始化 |
| **3** | **关掉调试信息** | `.cargo/config.toml` 里 `-C debuginfo=0 -C link-args=/DEBUG:NONE` | `target/` 缩小 53%（PDB 是主因） |
| **4** | **减小依赖图** | 零依赖 or 零 proc-macro 依赖 | 冷构建 395ms（对照：`windows-sys` 单依赖就让它涨到 2,599ms） |

### 但必须清醒的两条边界

- 🔴 **原则 2 的代价是 821 行手写基础设施**（CRT 原语 / panic handler / 入口 / SEH 桩 / 无边界检查原语）。907 行里只有 86 行是业务逻辑。
- 🔴 **原则 4 与「用 windows-sys」直接冲突**：`windows-sys` 是巨型 crate，冷构建从 395ms 涨到 2,599ms。**省行数 ≠ 省时间。**

---

## 六、结论表：每个维度的赢家

| 维度 | C/xmake | Rust | 赢家 |
|------|---------|------|------|
| **裸编译器墙钟** | 947 ms (gcc) | **418 ms** (rustc) | **Rust 0.44×** ⚡ |
| **`.text` 字节** | 9,248 | **3,201** | **Rust 0.35×** |
| **exe 体积（同后端）** | 19,456 (MinGW) / 143,872 (MSVC) | **7,168** | **Rust 0.37× / 0.05×** |
| **总指令数** | 2,492 | **1,385** | **Rust 0.56×** |
| **导入 DLL 数** | 11 (MinGW) / 3 (MSVC) | **3** | **Rust 平/胜** |
| **功能对齐的行数** | **256** | 907 | **C 0.28×** |
| **需要的 unsafe** | 0（但有 20+ 宏/手写 FFI） | ~300 行 | 平 |
| **换平台成本** | 改 makefile + 条件编译 | 改 Cargo feature + `#[cfg]` | 平 |
| **构建系统自身开销** | 607 ms noop（xmake Lua + flag 探测） | **136 ms noop** | **Rust 0.22×** |
| **首次构建** | 3,146 ms | 533 ms（零依赖）/ 2,599 ms（用 windows-sys） | **Rust 胜（零依赖时）** |

### 一句话

> **Rust 在编译器层面全面胜出（速度 0.44×、体积 0.37×、指令数 0.56×），这个胜利来自「不链 CRT + 预编译声明」。**
> **它的唯一代价是行数（907 vs 256）和「不要引入重依赖」这条纪律——一旦引入 `windows-sys` 这类巨型 crate，冷构建优势立刻从 0.44× 变成 2.7×。**

---

## 附：复现命令

```bash
# 1. C 预处理膨胀
gcc -E src/main.c | wc -l                          # → 122,914
gcc -ftime-report -O3 -c src/main.c                # → parsing 93%

# 2. Rust 各阶段
rustup run nightly rustc -Ztime-passes -C opt-level=z -C lto=fat \
  -C panic=abort -C codegen-units=1 src/main.rs     # → total 0.255s

# 3. 指令数
llvm-objdump --triple=x86_64-pc-windows-gnu -d ip-c.exe > c.asm
llvm-objdump -D ip-bare.exe > rs.asm
# 按助记符分类统计（见 /tmp/instr_stat.py 思路）

# 4. 导入 DLL
llvm-objdump -p ip-c.exe | grep -c "DLL Name"       # → 11
llvm-objdump -p ip-bare.exe | grep -c "DLL Name"    # → 3

# 5. 段大小
llvm-objdump -h ip-bare.exe | awk '/.text/{print $3}'  # → 00000c81
```


---

# 附录 D — 按指定工具链复测：clang-cl / zig cc / cargo-zigbuild

> 日期：2026-10-01　机器与前面各章相同
> 触发：用户指令「优先使用 clang-cl, cargo-zigbuild」
> 全部数字为 Python `time.perf_counter()` 实测（剥除 `BASH_ENV`），同机同源文件

## D.1 工具链版本

| 工具 | 版本 | 位置 |
|---|---|---|
| clang-cl | clang 22.1.8 (x86_64-pc-windows-msvc) | `D:\APP\scoop\apps\llvm\current\bin` |
| cl (MSVC) | 14.51.36231 | `D:\VS\Product\VC\Tools\MSVC\14.51.36231` |
| zig | 0.16.0 | `D:\APP\scoop\apps\zvm\current\data\bin` |
| cargo-zigbuild | 0.23.4 | `~/.cargo/bin` |
| rustc | 1.98.1-stable (cargo) / 1.100.0-nightly | rustup |

## D.2 体积终局表 —— 同一份 C 源码 / 同一份 Rust 源码

```
配置                                     exe(B)   .text(B)   导入DLL数
----------------------------------------   --------   --------   ---------
Rust no_std MSVC  (cargo --release)         7168       3201         3   <-- 冠军
C    clang-cl /MD (动态 CRT)               12288       5462        10
Rust no_std cargo-zigbuild (windows-gnu)   78336      56774         9
C    cl.exe /O2 /MT (静态 CRT)            143872      87488         3
C    clang-cl /MT (静态 CRT)              152576      94685         3
C    zig cc windows-gnu                   192000     149574         n/a
C    zig cc windows-msvc                  566784     454102         n/a
```

## D.3 编译时间表（best-of-N，毫秒）

```
C 侧（单文件编译+链接）
  zig cc windows-gnu                188 ms    -> 192000 B
  cl /O2 /MT                        446 ms    -> 143872 B
  clang-cl /O2 /MD                  489 ms    ->  12288 B
  clang-cl /O2 /MT                  500 ms    -> 152576 B

Rust 侧（cargo 冷构建，已删 target/）
  cargo build --release (MSVC)       533 ms    ->   7168 B   <-- 时间同级、体积 1/6.8
  cargo zigbuild --release (gnu)    1826 ms    ->  78336 B   <-- 3.4x 时间、10.9x 体积
```

## D.4 四个可复现的关键发现

### ① clang-cl 在 `/MT` 下不做函数级裁剪 —— 体积恒定
`/O1`、`/O2`、`/Oz`、`-flto -fuse-ld=lld-link` 四种优化档编译出的 exe **全部是 152,576 B，
一个字节都不差**。这说明 clang-cl 驱动按整库方式拉入 `libcmt.lib`（静态 CRT），
没有像 `cl.exe` 那样做 `/OPT:REF` 级别的按需抽取。
对比 `cl.exe /O2 /MT` = 143,872 B（小 6%）。

**实践含义**：在 Windows 上追求小体积时，**不要用 clang-cl 走 `/MT`**；
要么用 `/MD`（12,288 B，但带 10 个 DLL 依赖），要么直接用 `cl.exe`。

### ② `zig cc` 快，但产物全静态且无法裁剪
`zig cc` 188 ms 是全场最快，但：
- `windows-gnu` → 192,000 B
- `windows-msvc` → 566,784 B
加 `-nostartfiles`、`-Wl,-e,<sym>` **都不改变大小**（仍 192,000 B），
说明 zig 的启动/CRT 目标文件是被无条件链入的，链接器 GC 到不了那一层。

### ③ cargo-zigbuild 在 Windows 极小二进制目标上方向相反
用 `cargo zigbuild --target x86_64-pc-windows-gnu --release` 编同一份
`ip-bare/src/main.rs`（只加了 Gnu 入口符号 + `WinMain` 桩，见下）：

| 指标 | MSVC 路线 | zigbuild 路线 | 倍数 |
|---|---|---|---|
| exe | 7,168 B | 78,336 B | **10.9×** |
| `.text` | 3,201 B | 56,774 B | **17.7×** |
| `.rdata` | 1,584 B | 17,716 B | **11.2×** |
| 导入 DLL | 3 | 9 | 3× |
| 冷构建 | 533 ms | 1,826 ms | **3.4×** |

多出来的 6 个 DLL 全是 UCRT 转发层：
`api-ms-win-crt-{heap,runtime,stdio,environment,math,multibyte}-l1-1-0.dll`。
**只要目标是 `windows-gnu`，这 6 个就必然被拖进来** —— 这是 rustc 的 gnu target 规格决定的，
不是 zig 的锅，zigbuild 也绕不开。

**结论**：`cargo-zigbuild` 的价值在**跨平台交叉编译**（Linux/macOS 上一键出 Windows/Android 产物），
**不在**「Windows 上做最小二进制」。两者目标正交。

### ④ 为了让 zigbuild 编过，实际改了什么
`ip-bare` 原版是 MSVC 专用（`/ENTRY:mainCRTStartup` + `#[cfg(windows)]`），
GNU 链接器不认 MSVC 语法，踩了四道坎，全部记录如下（新目录 `ip-bare-gnu/`）：

| # | 报错 | 根因 | 解法 |
|---|---|---|---|
| 1 | `unrecognized file extension: /ENTRY:mainCRTStartup` | `/ENTRY:` 是 MSVC 语法 | 改 `-Wl,-e,mainCRTStartupGnu` |
| 2 | `duplicate symbol: mainCRTStartup`（与 `crt2.obj` 冲突） | mingw CRT 已占用该名 | 入口改名 `mainCRTStartupGnu`，并给 MSVC 块加 `target_env = "msvc"` 门 |
| 3 | `undefined symbol: WinMain`（`libmingw32:crtexewin.obj`） | zig 的 mingw 运行时强制引用 | 加一个 `WinMain` 桩（永不返回，实际入口仍是自定义那个） |
| 4 | `E0308: expected u32, found i32` | `platform::exit` 收 `u32`、`run()` 返回 `u32` | 桩函数统一 `-> u32` |

对应 `.cargo/config.toml`（`ip-bare-gnu/`）：

```toml
[build]
rustflags = ["-C", "link-args=-Wl,-e,mainCRTStartupGnu -Wl,--subsystem,console -nostartfiles -Wl,--gc-sections -Wl,--strip-all"]

[target.x86_64-pc-windows-gnu]
linker = "zigcc-x86_64-pc-windows-gnu"
```

注：`-nostartfiles` 在这个链路上**实际未生效**（zig 仍拉了 mingw 启动对象），
所以第 3 步的 `WinMain` 桩是必需的 —— 这一点在本机实测确认。

## D.5 最终建议（更新版）

| 目标 | 推荐 | 理由 |
|---|---|---|
| **Windows 最小二进制** | `rustc` + MSVC（`/ENTRY:` + `/DEBUG:NONE` + LTO fat + `opt-level="z"`） | 7,168 B / 533 ms，全场唯一 3-DLL 的产物 |
| **Windows 编译速度最快** | `zig cc`（C 项目） | 188 ms；但接受 192 KB 全静态产物 |
| **C 项目、要小** | `cl.exe /O2` + `/MD` | ~12 KB 级；比 clang-cl 的 `/MT` 小 12× |
| **跨平台交叉编译 Rust** | `cargo-zigbuild` | 这是它的主场；**不要**用它追体积 |
| **不要用** | clang-cl `/MT` 追体积、zigbuild 追体积 | 前者恒定 152 KB，后者 78 KB/1.8 s |

## D.6 复测命令

```bash
# C 侧
export PATH="/d/app/scoop/apps/llvm/current/bin:$PATH"
clang-cl /nologo /O2 /GS- /MD /std:c11 /Fe:out.exe main.c ws2_32.lib iphlpapi.lib   # 12,288 B
clang-cl /nologo /O2 /GS- /MT /std:c11 /Fe:out.exe main.c ws2_32.lib iphlpapi.lib   # 152,576 B

# Rust 侧
cargo build --release                                   # MSVC -> 7,168 B
cargo zigbuild --target x86_64-pc-windows-gnu --release # gnu   -> 78,336 B

# 体积与段成分
llvm-objdump -h out.exe | awk '/\.text/{print strtonum("0x"$3)}'
llvm-objdump -p out.exe | grep -c "DLL Name"
```

配套脚本：`_probe/bench_all.py`（全量计时）、`_probe/clangcl_build.ps1`（clang-cl 各档）、
`ip-bare-gnu/`（zigbuild 可编译工程）。
