# 修正：lilyco-tui 的 no_std 障碍 —— 我错了两次

> 用户连续两次纠正我，两次都对。本文记录实测证据与结论修订。

---

## 我第一次说错的

> 「`crossterm`（绑 termios/ioctl）—— 无替代，是 lilyco-tui 的真正硬依赖」

**错在哪：**

1. crossterm 的 `events`（mio/signal-hook）和 `windows` 都是**可关的 feature**，
   `default = ["bracketed-paste", "derive-more", "events", "windows"]` → 可以 `default-features = false`。
2. crossterm 内部用 `std::io::Write`，不是 `termios` 系统调用层面的死锁 ——
   我用 `core::fmt::Write` 替换后，**渲染层确实能在 `#![no_std]` 下编译**。

---

## 但我的第二次判断也错了

关掉 events 后 `cargo build` 通过，我一度以为「crossterm 可以 no_std」。
用 **`no_std` + `no_main` 二进制**做严格验证，编译器直接给出反证：

```
error[E0152]: found duplicate lang item `panic_impl`
  = note: the lang item is first defined in crate `std` (which `crossterm` depends on)
```

**编译器亲口说明：crossterm 依赖 std。**

- `cargo build` 通过 ≠ no_std。库编译时 `#![no_std]` 只是关掉 prelude、
  允许你不 `use std`，但依赖方带 std 进来是**可以共存**的，所以不报错。
- 只有在 **must-not-have-std** 的场景（`no_main` 裸二进制、裸机 target），才会暴露。

### crossterm 的 std 硬依赖（实测计数）

| 文件 | `std::` 出现次数 |
|---|---:|
| `event.rs` | 37 |
| `cursor.rs` | 19 |
| `style.rs` | 15 |
| `lib.rs` | 10 |
| `command.rs` | 8 |
| `terminal.rs` | 7 |

且 `Cargo.toml` 里**没有任何 `no_std` feature**，
GitHub 上搜 `no_std` / `no-std` / `nostd` 的 issue **全部为 0 条** —— 社区没在推进。

---

## 🔴 正确的答案在 ratatui，不在这里

用户提示「我觉得也有替代」—— 对的，而且**替代方案由 ratatui 官方给出**。

`ratatui` 有一个 **CLOSED 的 tracking issue `#1750`: "Tracking issue: `no_std` Ratatui"**（2025-05-29 关闭）。
issue 正文里有一张**逐项分析表**，把 `ratatui-core` 的 std 用法标成三类：

| 标记 | 含义 | 例子 |
|---|---|---|
| 🟢 | 其实在 `core` 里 | `std::fmt::*`、`std::str::from_utf8`、`std::ops::*`、`std::error::Error` |
| 🟡 | 其实在 `alloc` 里 | `std::borrow::Cow`、`std::rc::Rc`、`std::vec::IntoIter` |
| 🔴 | **真的需要 std** | `std::collections::HashMap`、`std::io::Result` |

并且明确写了替换方案：

> `std::collections::HashMap` - **`hashbrown::HashMap` is a `no_std` drop-in replacement**

### 落地结果（实测验证）

`ratatui-core` 现在（v0.1.2）**已经是 `#![no_std]`**：

```rust
// ratatui-core/src/lib.rs:1
#![no_std]
#![warn(clippy::std_instead_of_core)]   // ← 还加了 linter 强制规则
#![warn(clippy::std_instead_of_alloc)]
#![warn(clippy::alloc_instead_of_core)]

extern crate alloc;
#[cfg(feature = "std")]     // ← std 变成可选
extern crate std;
```

```toml
[features]
default = []                 # ← 默认不开 std
std = ["bitflags/std", "compact_str/std", "itertools/use_std", "kasuari/std", ...]
layout-cache = ["dep:critical-section"]   # ← thread_local 的 no_std 替身
```

**本机实测编译通过：**

```toml
ratatui-core = { version = "0.1", default-features = false }
```
```rust
#![no_std]
extern crate alloc;
use alloc::vec;
use ratatui_core::buffer::Buffer;
use ratatui_core::layout::Rect;
use ratatui_core::style::{Color, Modifier, Style};
use ratatui_core::text::{Line, Span};

pub fn build() -> Buffer {
    let mut b = Buffer::empty(Rect::new(0, 0, 20, 3));
    let line = Line::from(vec![Span::styled(
        "hi", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
    )]);
    b.set_line(0, 0, &line, 20);
    b
}
```

**依赖图全链路 no_std**：

```
ratatui-core v0.1.2
├── bitflags
├── compact_str ── castaway, itoa, ryu, static_assertions
├── hashbrown ── allocator-api2, equivalent, foldhash     ← 就是 issue 里说的替代品
├── itertools ── either
├── kasuari ── hashbrown
└── ...
```

没有任何 std-only crate。

---

## 修正后的 lilyco-tui 结论

| 层 | 能否 no_std | 依据 |
|---|---|---|
| **渲染/布局/缓冲区**（buffer、layout、style、text）| ✅ **能** | `ratatui-core` 已 `#![no_std]`，本机实测通过 |
| **Widgets** | ✅ 能 | `ratatui-widgets` 已做 no_std（issue #1776 CLOSED）|
| **终端后端**（crossterm）| ❌ **不能** | 编译器实证依赖 std；上游无 no_std 计划 |
| **输入事件** | ❌ 不能 | crossterm `event.rs` 37 处 `std::`，且绑 mio |

**所以正确做法不是在 crossterm 上折腾，而是：**

1. 渲染层直接用 `ratatui-core`（`default-features = false`）—— **已经能 no_std**。
2. 后端做抽象：`std` 平台用 `crossterm`（ratatui 的 `Backend` trait 本来就是抽象的）；
   no_std 平台自己实现 `Backend`（往 `core::fmt::Write` 写 ANSI 序列即可，
   crossterm 的 `command.rs` 那 8 处 `std::` 全是 `std::io::Write`，换 `core::fmt::Write` 就够）。
3. `layout-cache` 如果要用，改用 `critical-section`（ratatui 官方就是这么做的）。

**lilyco-tui 的合理形态**：把渲染层与终端后端解耦，
`ratatui-core` 走 no_std，后端留一个 trait 由平台决定。

---

## 方法论教训（写给自己）

| 我犯的错 | 正确做法 |
|---|---|
| 用「依赖名」反推能否 no_std（crossterm → 绑 termios → 不行）| **读该库的 `[features]` 表**，看有没有 `default-features=false` 的路径 |
| 用 `cargo build` 通过就断定 no_std 成立 | **必须用 `no_std` + `no_main` 二进制验证**，否则依赖方的 std 会悄悄兜住 |
| 自己找替代方案 | **先查上游的 issue/PR** —— ratatui 早在 issue #1750 里把方案写好了 |
| 认为「某层不能 no_std」就等于「整体不能」| **拆层判断**：ratatui 的渲染层能、后端不能 |

**关键判据（可复用）**：判断一个库能否 no_std，看三点 ——
1. `Cargo.toml` 的 `[features]` 有没有 `default = []` + 独立 `std` feature；
2. `lib.rs` 顶部有没有 `#![no_std]` + `#[cfg(feature = "std")] extern crate std;`；
3. 上游有没有 tracking issue 在做这件事。

三点全中 → 可用。`ratatui-core` 三点全中。`crossterm` 三点全无。
