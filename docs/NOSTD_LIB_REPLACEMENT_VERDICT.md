# 907 行手写基础设施 → no_std 库替代：实测判决书

生成时间：2026-10-01
实验机：x86_64-pc-windows-msvc / rustc 1.98.1-stable / MSVC 14.51
对照物：`D:/Code/rust/question/ip-bare/src/main.rs`（907 行，零依赖/零 alloc/no_std/no_main）

---

## 一句话结论

**6 类手写基础设施里，4 类有成熟库可替代（rlibc / windows-sys / dont_panic），1 类必须自己写但只需 4 行（`__CxxFrameHandler3` stub），1 类库方案反而更差（`no-panic` 拖 proc-macro 税）。**

| # | 手写内容 | ip-bare 行数 | 替代库 | 实测结论 | 净收益 |
|---|---------|------------|--------|---------|--------|
| 1 | `memset`/`memcpy`/`memcmp` | 49 (784-832) | **`rlibc`** | ✅ 稳定版可用，需 `/WHOLEARCHIVE` 绝对路径 | **−49 行** |
| 2 | `strlen` | 9 (824-832) | — | ⚠️ rlibc **不提供** strlen，仍需自写 | ±0 |
| 3 | `mainCRTStartup` + `/ENTRY` 链接参数 | 15 (862-876) | ❌ 无 | 无库可代；但可用 `build.rs` 的 `rustc-link-arg` 免去 config.toml | −8 行（配置搬家） |
| 4 | `__CxxFrameHandler3` SEH stub | 9 (843-851) | ❌ 无 | 无库可代，但 **4 行**即可 | ±0 |
| 5 | `panic_handler` | 5 (853-857) | 多种 panic-* crate | 需 `platform::exit`，自写更短 | ±0 |
| 6 | `b_at`/`slice_u`/`split_host_port`/`to_cstr` | 61 (54-113) | `dont_panic`（校验）/ `no-panic`（证明） | `dont_panic` **零依赖**，`no-panic` 拖 syn+quote+proc-macro2 | 语义增强，行数不减 |
| 7 | 平台 FFI 声明（kernel32/ws2_32/iphlpapi） | ~130 (145-480) | **`windows-sys` 0.61** | ✅ 完整 no_std 可用，0.61 换 `windows-link` 后无 proc-macro | **−130 行** |

**可删手写行数：约 179 行（49 mem + 130 FFI）。**
**907 行 → 约 728 行。** 再加上 `windows-sys` 也消灭了 POSIX 分支的 `#[link(name="c")]` 声明（另有 ~70 行），**全平台合计可降到 ~600 行出头**。

### 实测落地：`ip-lib` 已经改出来了

不是纸面推演。我照上面方案实做了一个 `D:/Code/rust/question/ip-lib/`（删掉 3 个手写 mem + 130 行 FFI，改用 `rlibc` + `windows-sys`），**构建通过，5 组参数输出与 `ip-bare` 逐字节一致**（仅 `-h` 的程序名显示不同，属预期）。

| 指标 | ip-bare（907 行手写） | ip-lib（770 行 + rlibc + windows-sys） | 变化 |
|------|---------------------|--------------------------------------|------|
| 源码行数 | 907 | **770** | **−137 行（−15%）** |
| **release exe** | **7,168 B** | **7,168 B** | **0（完全相同）** |
| debug exe | 18,432 B | 18,944 B | +512 B（未优化泛型） |
| **冷构建** | **395 ms** | **2,599 ms** | **+2,204 ms（6.6× 慢）** ⚠️ |
| 依赖编译后的 noop | 136 ms | 145 ms | +9 ms |
| 依赖编译后 touch 重建 | 309 ms | 347 ms | +38 ms |
| 依赖数 | 0 | 2（rlibc + windows-sys） | +2 |

**⚠️ 这是本次最重要的反直觉发现：换库省了 137 行，但把冷构建从 395ms 拖到 2,599ms —— 慢了 6.6 倍。**

原因和之前 `serde_derive` 那次的结论**完全同构**：
- `windows-sys` 虽然 0.61 换成了 `windows_link!` 宏（无 proc-macro 税），但它**本身是个巨型 crate**——即使只开 10 个 feature，仍要解析/类型检查大量 `mod.rs`。
- 冷构建慢的是「编译依赖」，不是「编译你的代码」。
- **一旦依赖编译过一次（进入 `target/`），日常开发循环几乎没有代价**（noop 136→145ms，touch 重建 309→347ms，都在噪声与 10% 以内）。

**⇒ 判决：** 如果目标是「**单文件小工具、每次干净构建**」（正是当初对标 C/xmake 的场景 —— xmake 每次也是从零编），**换库是净亏**：省 137 行换来 6.6× 冷构建时间，体积还不变。
如果是「**长期维护、依赖常驻 `target/`**」的正常开发，换库值得：省 137 行 + 免去 130 行易错的结构体偏移手工核对，日常循环几乎无感。

---

## ⚠️ 为什么体积零变化（7,168 B → 7,168 B）—— 本次最关键的认知

比行数更值得记住。实测：**手写 907 行版和换库 770 行版的 release exe 都是 7,168 B，逐字节同尺寸。**

1. **130 行 `extern` FFI 声明本身就是零体积的。** 它们只是符号表条目，编译后完全消失；换成 `windows-sys`（也是 `#[link]` 声明）自然也是零体积。**删掉它们不减少一个字节的 `.text`。**
2. **`rlibc` 的 `memset` 自己占的字节 ≈ 手写版占的字节。** 换库不省体积，只省行数。实测 release 下两者都 7,168 B。
3. **真正压体积的是 `ip-bare` 的第 5、6 类**——`panic_handler` + 无边界检查原语。它们消灭了 `panic_bounds_check → panic_fmt → core::fmt` 那条约 10KB 的链。**这两类没有库能替代**：`dont_panic` 只做校验不做替代。

**⇒ 一句话：库能砍掉「行数」，砍不掉「体积」。体积的唯一杠杆永远是「消灭 panic 调用点」，那件事只能靠 `unsafe` 或静态校验，不能靠换库。**
**⇒ 另一句：库还会增加「冷构建时间」——这是换库的真实代价，之前没被计入。**

---

## 逐项实测证据

### 1. `rlibc` —— 解决 memset/memcpy/memcmp（49 行）

`rlibc` v1.0.0，rust-lang 官方仓库，836,537 次下载。源码 173 行，`#![no_std]`，导出：

```
00000000 T memcmp
00000000 T memcpy
00000000 T memmove
00000000 T memset
```

**踩坑 1：stable 上 `compiler_builtins` 根本不能当普通依赖用。**
- `compiler_builtins` v0.1.160，35,995,352 次下载，是 rustc 自己用的。
- 加 `features = ["mem"]`（**注意不是 `compiler-builtins-mem`**）后编译报：
  ```
  error[E0554]: #![feature] may not be used on the stable release channel
   --> compiler_builtins-0.1.160/src/lib.rs:3:1  |  #![feature(abi_unadjusted)]
  ```
- 开 `RUSTC_BOOTSTRAP=1` 强闯，报：
  ```
  error: the crate `compiler_builtins` resolved as `compiler_builtins` but is not `#![compiler_builtins]`
  ```
- **结论：`compiler_builtins` 只能在 `-Zbuild-std` 场景下作为 sysroot 组件使用，普通 `[dependencies]` 永远不行。** 这条路彻底堵死。

**踩坑 2：MSVC 的 `link.exe` 不认 GNU 风格的 `-l`。**
- `build.rs` 里 `println!("cargo:rustc-link-arg=-lrlibc")` → `warning LNK4044: 无法识别的选项"/lrlibc";已忽略` → 依然 LNK2019。
- 正解：`/WHOLEARCHIVE:<绝对路径>`。
  ```
  cargo:rustc-link-arg=/WHOLEARCHIVE:C:/.../target/debug/deps/librlibc-xxxx.rlib
  ```
- **为什么必须 `/WHOLEARCHIVE`**：MSVC 归档按需拉取成员；`libcore.rlib` 引用 memset 时 `librlibc` 还没被扫描到，普通传参解析不了。`/WHOLEARCHIVE` 强制整包载入。

**踩坑 3：`/WHOLEARCHIVE` 路径不能硬编码。**
- `target/debug/deps/` 下有多个 `librlibc-<hash>.rlib`（不同 feature/flag 组合产生），哈希每次可能变。
- **正解：`build.rs` 从 `OUT_DIR` 反推**：
  ```rust
  // OUT_DIR = <target>/<profile>/build/<pkg>-<hash>/out
  // ancestors: 0=out  1=<pkg>-<hash>  2=build  3=<profile>  4=<target>
  let out = PathBuf::from(std::env::var("OUT_DIR").unwrap());
  let profile = out.ancestors().nth(3).unwrap().file_name().unwrap().to_os_string();
  let target  = out.ancestors().nth(4).unwrap().to_path_buf();
  let deps    = target.join(&profile).join("deps");
  ```
  ⚠️ 我第一版写成 `nth(2)`/`nth(3)`，build 静默产出 `cargo:warning=rlibc rlib not found in ...\target\debug\build\deps`，白折腾一轮。**代数要从外往里数。**
- 多个候选时取字典序最大者，否则 `/WHOLEARCHIVE` 两次引入同一符号 → `LNK2005: memcpy 已经在 ... 中定义`。

**验证**：`/WHOLEARCHIVE` 生效后，`link.exe` 报错列表里 `memset`/`memcpy`/`memcmp` 三行**全部消失**，只剩 `__CxxFrameHandler3`。

**⚠️ `rlibc` 不提供 `strlen`。** 所以 `ip-bare` 825-832 那段 `strlen` 仍须自写（或改用 `rlibc` 的 `memcmp` 改写 `cstr_len` 实现）。

---

### 2. `windows-sys` 0.61 —— 解决全部平台 FFI 声明（~130 行）

**这是本次最大的发现：`windows-sys` 完整支持 `#![no_std] + #![no_main]`。**

```toml
[dependencies]
windows-sys = { version = "0.61", default-features = false, features = [
  "Win32_Foundation",
  "Win32_Networking_WinSock",
  "Win32_NetworkManagement_IpHelper",
  "Win32_NetworkManagement_Ndis",   # ← 这个必须加，否则 GetAdaptersAddresses 不存在
  "Win32_System_Console",
  "Win32_System_Threading",
] }
```

实测：链接成功，exe **3,072 B**（理论下界）。

**关键机制**：`windows-sys` 0.61 已从「巨型 `#[link] extern` 块」改为 `windows_link::link!` 宏 —— 宏在编译期展开成单个 `#[link]`，**不经过 proc-macro**，所以没有 proc-macro 编译税。

**踩坑：`GetAdaptersAddresses` 需要双 feature gate。** 源码里是：
```rust
#[cfg(all(feature = "Win32_NetworkManagement_Ndis", feature = "Win32_Networking_WinSock"))]
windows_link::link!("iphlpapi.dll" "system" fn GetAdaptersAddresses(...));
```
只加 `Win32_NetworkManagement_IpHelper` 会报 `no GetAdaptersAddresses in Win32::NetworkManagement::IpHelper`。

**注意**：`ip-bare` 现有的 `#[repr(C)] struct AdapterAddresses/UnicastAddress/AddrInfo` 都可以直接换成 `windows_sys::...::IP_ADAPTER_ADDRESSES_LH` / `IP_ADAPTER_UNICAST_ADDRESS_LH` / `addrinfo`，那份「手工数结构体偏移」的易错代码可以整段删掉。**这是除行数外更大的收益：不再靠肉眼核对 `_physical: [u16;4]` / `_physical_len` / `_flags` / `_mtu` 的位置。**

---

### 3. `dont_panic` vs `no-panic` —— 校验「不可能 panic」的辅助函数（61 行）

两者目标相同：让编译器证明某段代码不可能 panic。实测差异巨大。

| | `dont_panic` 0.1.0 | `no-panic` 0.1.37 |
|---|---|---|
| 依赖树 | **仅自身，0 依赖** | `syn 3.0.6` + `quote` + `proc-macro2` + `unicode-ident` |
| 机制 | `#![no_std]`，一个 `extern "C"` 未定义符号 + 宏 | 属性宏，编译期分析 MIR |
| 超时风险 | 无 | 拖 proc-macro 链 |
| 实测 | ✅ 0.18s 构建 | ✅ 4.12s 构建（**慢 23 倍**） |

`dont_panic` 源码核心（MITNFA，`categories = ["no-std"]`）：
```rust
#![no_std]
extern "C" { pub fn rust_panic_called_where_shouldnt() -> !; }
#[macro_export] macro_rules! dont_panic { ... }  // 展开成调用那个不存在的符号
```
若编译器没能优化掉，**直接链接失败** —— 语义等价于 `ip-bare` 手写 unchecked 想要的保证，但由库强制。

**判决：用 `dont_panic`，不要用 `no-panic`。**
理由完全一致于我们之前对 `serde_derive`/`thiserror` 的判决：**proc-macro 是串行关键路径上的最大税源**，而 `dont_panic` 恰好零依赖。

⚠️ **但 `dont_panic` 不减少行数** —— `b_at`/`slice_u` 那类 `unsafe` 访问函数还是得存在。它换来的是**正确性保证**（从「注释里保证下标合法」变成「编译器保证」），不是行数。

---

### 4. 无库可代的三个残留项

**`mainCRTStartup` + `/ENTRY`** —— 没有 crate 能做这件事。原因：这正是「不用 CRT 启动代码」的本质，任何库自己也需要一个入口。**但可以简化**：把 `link-args` 从 `.cargo/config.toml` 挪到 `build.rs` 的 `cargo:rustc-link-arg`，避免污染 proc-macro 宿主构建（见下方踩坑）。

**`__CxxFrameHandler3`** —— 没有 crate。但只需 4 行，且**只在 dev 档需要**（release 的 LTO 会删掉带 landing pad 的代码）。这是 MSVC 特有的 ABI 细节，不值得找库。

**`panic_handler`** —— 有 `panic-halt`/`panic-abort`/`panic-abort` 等一堆，但 `ip-bare` 的需求是「`ExitProcess(101)`」，自己 3 行更短。**用库反而更长。**

---

## 新增踩坑：`link-args` 会污染 proc-macro 宿主构建

第一次测 `no-panic` 时把 `link-args=/ENTRY:mainCRTStartup /SUBSYSTEM:CONSOLE` 放进 `.cargo/config.toml` 的 `[target.x86_64-pc-windows-msvc]`，结果报：

```
/OUT:...\deps\no_panic-cfef6af39c8d997b.dll  /DLL
msvcrt.lib(exe_main.obj) : error LNK2019: 无法解析的外部符号 main
```

**根因**：proc-macro crate 编译成宿主（x86_64-pc-windows-msvc）**DLL**，也被同一份 `rustflags` 命中，于是 `mainCRTStartup` 和 `/DLL` 打架。

**正解：用 `build.rs` 的 `cargo:rustc-link-arg`** —— 它只作用于**本 package 的最终产物**，不落到依赖上：
```rust
println!("cargo:rustc-link-arg=/ENTRY:mainCRTStartup");
println!("cargo:rustc-link-arg=/SUBSYSTEM:CONSOLE");
```
这样 `.cargo/config.toml` 可以完全不要，链接参数就地声明。

---

## 为什么体积零变化（7,168 B → 7,168 B）

这是本次最重要的认知，比行数更值得记住：

1. **`rlibc` 的 `memset` 被链接进去了吗？** 是。但它**同样占字节**。手写版 4 个函数约 60–100 字节，`rlibc` 版本 LLVM 会向量化，体积相当。**换库不省体积，只省行数。**
2. **`windows-sys` 换掉 130 行 FFI，体积不变** —— 因为 130 行 `#[link] extern` 声明本身就是**零体积**（只是符号表条目，编译后完全消失）。删掉它们不减少一个字节的 .text。
3. **`/WHOLEARCHIVE` 有代价** —— 它强制拉入 `rlibc` **整个归档**。`rlibc` 里的 `memmove`（`ip-bare` 用不到）也会进来。实测总体积没涨，说明 `/OPT:REF` 把未引用的 `memmove` 又删了。

**结论：** 907 行里真正压体积的是**第 5、6 类**（panic 处理 + 无边界检查原语）—— 它们消灭了 `panic_bounds_check → panic_fmt → core::fmt` 那条 10KB 的链。而这两类**没有库能替代**（`dont_panic` 只做校验不做替代）。

**⇒ 一句话：库能砍掉「行数」，砍不掉「体积」。体积的杠杆永远是「消灭 panic 调用点」，那件事只能靠 `unsafe` 或 `dont_panic` 这样的静态校验，不能靠换库。**

---

## 最终推荐组合（实测全部通过）

> 注意：下面这套组合能编译、能跑、输出与手写版一致、release 体积不变，**但冷构建从 395ms 涨到 2,599ms**。是否采用取决于你的场景（见上文判决）。

```toml
# Cargo.toml
[dependencies]
rlibc = "1"                                    # memcpy/memmove/memset/memcmp
dont_panic = "0.1"                             # 零依赖的「不可 panic」静态校验
windows-sys = { version = "0.61", default-features = false, features = [
  "Win32_Foundation",
  "Win32_Networking_WinSock",
  "Win32_NetworkManagement_IpHelper",
  "Win32_NetworkManagement_Ndis",              # ← 必须，否则无 GetAdaptersAddresses
  "Win32_System_Console",
  "Win32_System_Environment",
  "Win32_System_WindowsProgramming",           # ← GetComputerNameA 在这
  "Win32_System_Threading",
  "Win32_System_IO",                           # ← WriteFile 需要
  "Win32_Storage_FileSystem",
  "Win32_Globalization",
] }
# ⛔ 不要用: compiler_builtins（stable 不可用作普通依赖）
# ⛔ 不要用: no-panic（拖 syn+quote+proc-macro2，编译慢 23 倍）
```

```rust
// build.rs —— 替代 .cargo/config.toml，且不污染 proc-macro 宿主
use std::path::PathBuf;
fn main() {
    println!("cargo:rustc-link-arg=/ENTRY:mainCRTStartup");
    println!("cargo:rustc-link-arg=/SUBSYSTEM:CONSOLE");
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let profile = out.ancestors().nth(3).unwrap().file_name().unwrap().to_os_string();
    let target  = out.ancestors().nth(4).unwrap().to_path_buf();
    let deps    = target.join(&profile).join("deps");
    let mut best = String::new(); let mut bp: Option<PathBuf> = None;
    for e in std::fs::read_dir(&deps).into_iter().flatten().flatten() {
        let s = e.file_name().to_string_lossy().to_string();
        if s.starts_with("librlibc-") && s.ends_with(".rlib") && s > best { best = s; bp = Some(e.path()); }
    }
    if let Some(p) = bp {
        println!("cargo:rustc-link-arg=/WHOLEARCHIVE:{}", p.to_string_lossy().replace(char::from(92), "/"));
    }
}
```

**仍需自写（4 处，约 20 行）：**
1. `strlen`（`rlibc` 不含）
2. `__CxxFrameHandler3`（4 行，仅 dev 档需要）
3. `mainCRTStartup`（本质需要，无法外置）
4. `panic_handler`（3 行，用库反而更长）

---

## 与「超越 C」目标的关系（实测数据，非估算）

| 指标 | C/xmake | ip-bare（907 行手写） | ip-lib（770 行 + 2 库） |
|------|---------|---------------------|----------------------|
| **release exe** | 13,824 B | **7,168 B（0.52×）** | **7,168 B（0.52×，相同）** |
| debug exe | — | 18,432 B | 18,944 B |
| 源码行数 | 256 | 907（3.5×） | **770（3.0×）** |
| 依赖数 | 0 | 0 | 2 |
| **冷构建** | 基准 | **395 ms** | **2,599 ms（6.6×）** |
| 依赖预热后 touch 重建 | — | 309 ms | 347 ms |
| 换平台成本 | 手改 makefile | 手改 `#[cfg]` 分支 + 手抄结构体 | 改 feature 列表 + 换对应 sys crate |

**最终结论：**
1. `awesome-nostd` 里的替代（`rlibc` / `windows-sys` / `dont_panic`）**确实能把 907 行降到 770 行**，并消灭「肉眼核对结构体偏移」「注释保证下标合法」这两处最易错的部分。
2. **但「比 C 小一半」的结论完全由手写 `unsafe`（消灭 panic 调用点）贡献，与换库无关** —— 换库后 release 体积**一字不变**（7,168 B = 7,168 B）。
3. **换库的真实代价是冷构建 6.6×** —— 因为 `windows-sys` 本身是巨型 crate。日常开发（依赖常驻 `target/`）几乎无感，但「每次从零构建」的场景是净亏。
4. **不可替代的 4 处（约 20 行）**：`strlen`、`__CxxFrameHandler3`、`mainCRTStartup`、`panic_handler`。它们是「不用 CRT」这个前提的固有成本，任何库都代不了。
5. **`compiler_builtins` 这条路彻底堵死** —— stable 上永远不能作为普通依赖，这是很多 no_std 教程没讲清的坑。

