//! 单态化审计 —— cargo-xmake 的差异化功能。
//!
//! ## 为什么需要它
//!
//! 实测（`experiments/dynproof`）：把 `fn f<T: Trait>(t: &T)` 改成
//! `fn f(t: &dyn Trait)` 在 N=200 时能把 LLVM IR 砍掉 69%、`f` 的副本从 200 份
//! 变成 1 份。但收益**不均匀** —— 它正比于
//!
//! ```text
//! （单态化份数 × 函数体大小）
//! ```
//!
//! 一个只被实例化 2 次的小函数改了白改。工具真正该做的是**排序**。
//!
//! ## 数据来源（按优先级）
//!
//! 1. 🔴 **`-Zdump-mono-stats=human -Zdump-mono-stats-format=json`** —— 最好的来源。
//!    rustc 不但给出每个条目的实例化次数，还给出它自己的**代价估算**：
//!    ```json
//!    [{"name":"drive","instantiation_count":200,"size_estimate":40,"total_estimate":8000},
//!     {"name":"<T152 as Work>::step","instantiation_count":1,"size_estimate":10,"total_estimate":10}]
//!    ```
//!    `total_estimate = count × size`，正好就是我们想排的那个东西。
//!    ⚠️ 它把输出写到 **crate 根目录下的 `human/`**，不是 `target/`
//!    —— 我第一轮找错地方，差点以为这个开关是死的。
//! 2. `-Zprint-mono-items=yes` —— 一行一条 demangle 好的名字，稳定好解析，
//!    但需要自己分组（`drive::<T0>` → `drive`）。
//! 3. `--emit=llvm-ir` —— stable 也能跑的兜底，数 `^define`，名字自己拆。
//!
//! ## 关键技巧
//!
//! `cargo rustc -- <args>` 的额外参数**只作用于目标 crate**，依赖不会重编。
//! 再配一个每次都变的 `-C metadata=<nonce>` 强制它重跑 ——
//! 否则第二次调用会被 cargo 判为 fresh，一行输出都没有。
//! （`cargo clean -p` 在新布局下不可靠，实测 `Removed 0 files` 之后照样 `Finished`。）

use crate::cli::{AuditMode, Opts};
use crate::util;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Source {
    MonoStats,
    MonoItems,
    LlvmIr,
}

impl Source {
    pub fn label(self) -> &'static str {
        match self {
            Source::MonoStats => "-Zdump-mono-stats (nightly)",
            Source::MonoItems => "-Zprint-mono-items (nightly)",
            Source::LlvmIr => "--emit=llvm-ir (stable 兜底)",
        }
    }
    /// 有没有 rustc 自己给的代价估算
    pub fn has_cost(self) -> bool {
        self == Source::MonoStats
    }

    /// 数据源自身的局限。必须显示出来 —— 不然用户会以为三种数据源等价。
    /// 实测 IR 兜底：`main` 数成 2 份、`std::rt::lang_start` 数成 4 份，
    /// 而 rustc 的实例化计数都是 1 —— 它数的是 IR 符号，不是单态化实例。
    pub fn caveat(self) -> Option<&'static str> {
        match self {
            Source::MonoStats => None,
            Source::MonoItems => Some("没有代价模型，排名按份数而非收益；名字里的泛型实参会被归并成同一个根"),
            Source::LlvmIr => Some(
                "数的是 IR 符号，不是单态化实例 —— 个别条目会比真实份数偏多（实测 main 2 / lang_start 4）；想要精确份数与代价模型请装 nightly",
            ),
        }
    }
}

/// 一条原始记录。前两个来源由 rustc 给 count；IR 兜底每条都是 1。
pub struct Entry {
    pub name: String,
    pub kind: String,
    pub count: usize,
    pub size: Option<u64>,
    pub total: Option<u64>,
}

pub struct Group {
    pub key: String,
    pub count: usize,
    pub size: Option<u64>,
    pub total: Option<u64>,
    /// 这个根是"被泛型复制出来的"吗
    pub from_generic: bool,
}

impl Group {
    /// 这个「根」是否因**实例化**而重复 —— 也就是 dyn 化能回收的目标。
    ///
    /// 判据是 `份数 > 1 且不是 trait 方法`，**不能**依赖 `from_generic`：
    ///   * `-Zprint-mono-items` 给的名字形如 `drive::<T0>`，归一化时能看到泛型块，
    ///     `from_generic` 为 true；
    ///   * `-Zdump-mono-stats` 给的是**已 demangle 的裸名** `drive`，一个字面量都不带
    ///     泛型标记，`from_generic` 恒为 false。
    /// 早期版本用 `from_generic` 当开关，导致代价模型这条主路径上 `reclaimable()`
    /// 恒等于 0。rustc 说某个符号名有 N 份实例，那 N 份就是实例化复制。
    pub fn duplicated(&self) -> bool {
        !is_trait_method(&self.key) && self.count > 1
    }

    /// 每个具体类型必然一份的 impl / vtable 槽位 —— dyn 化消不掉。
    ///
    /// 与 `duplicated()` 互斥且互补（在 `count > 1` 的前提下，二者恰好覆盖全部情况），
    /// 所以输出层只能写成 `if is_vtable_slot() … else if duplicated() …`；
    /// 写成 `duplicated() && is_vtable_slot()` 会永远为假（曾经真的这么写过，
    /// 标签就消失了）。
    pub fn is_vtable_slot(&self) -> bool {
        is_trait_method(&self.key) && self.count > 1
    }
}

pub struct Report {
    pub package: String,
    pub profile: String,
    pub source: Source,
    pub entries: usize,
    pub groups: Vec<Group>,
    pub roots: usize,
}

impl Report {
    pub fn duplication(&self) -> f64 {
        if self.roots == 0 {
            1.0
        } else {
            self.entries as f64 / self.roots as f64
        }
    }

    /// 能靠"去泛型"消掉的条目数。
    ///
    /// ⚠️ 必须把 `<# as Trait>::method` 排除掉。
    /// 实测：`drive` 的 200 份变 1 份，但 `<Tn as Work>::step` 还是 200 份 ——
    /// 每个具体类型都必须有一个 vtable 槽指向它。那不是 dyn 化能消掉的，
    /// 那是"你有 200 个具体类型"造成的。
    /// 早期版本把两者都算进"可回收"，报出 95%，而实测只降 47%。
    pub fn reclaimable(&self) -> usize {
        self.groups
            .iter()
            .filter(|g| g.duplicated())
            .map(|g| g.count - 1)
            .sum()
    }

    /// 可回收的**估算代价**（只在有 rustc 代价模型时可用）
    pub fn reclaimable_cost(&self) -> Option<u64> {
        let mut any = false;
        let mut sum = 0u64;
        for g in &self.groups {
            if g.duplicated() {
                if let Some(t) = g.total {
                    any = true;
                    sum += t.saturating_sub(g.size.unwrap_or(0));
                }
            }
        }
        any.then_some(sum)
    }

    pub fn total_cost(&self) -> Option<u64> {
        let mut any = false;
        let mut sum = 0u64;
        for g in &self.groups {
            if let Some(t) = g.total {
                any = true;
                sum += t;
            }
        }
        any.then_some(sum)
    }

    /// 每个具体类型必然一份的量（vtable / impl），dyn 化动不了它。
    pub fn type_inherent(&self) -> usize {
        self.groups
            .iter()
            .filter(|g| is_trait_method(&g.key) && g.count > 1)
            .map(|g| g.count - 1)
            .sum()
    }
}

/// `<# as Trait>::method` —— 说明这个条目来自"每个具体类型一份"的 impl/vtable，
/// 不是可以被 dyn 化合并的泛型函数体。
pub fn is_trait_method(key: &str) -> bool {
    key.starts_with("<# as ")
}

// ---------------------------------------------------------------- 名字归一化

/// 把 `drive::<T0>`、`core::num::<impl u32>::pow`、`<T0 as Work>::step`
/// 都归一到同一个"根"。
///
///   * `foo::<T0, T1>`             —— 泛型实参在**末尾**，整段砍掉   → `foo`
///   * `core::num::<impl u32>::pow` —— 泛型块在**中间**，后面还有路径，
///                                    只把块内换成 `#` 继续走      → `core::num::<#>::pow`
///   * `<T0 as Work>::step`        —— 开头是 `<具体类型 as Trait>`，
///                                    把 `as` 之前换成 `#`          → `<# as Work>::step`
pub fn normalize(name: &str) -> (String, bool) {
    let mut generic = false;
    let mut out = String::with_capacity(name.len());
    let b = name.as_bytes();
    let mut i = 0;

    while i < b.len() {
        if b[i] == b':' && b.get(i + 1) == Some(&b':') && b.get(i + 2) == Some(&b'<') {
            let Some(close) = match_angle(name, i + 2) else {
                out.push_str(&name[i..]);
                break;
            };
            let after = close + 1;
            generic = true;
            if after >= b.len() {
                break; // 末尾泛型实参 = 实例化，丢掉
            }
            out.push_str("::<#>"); // 中间泛型块 = 路径的一部分
            i = after;
            continue;
        }
        out.push(b[i] as char);
        i += 1;
    }

    if out.starts_with('<') {
        if let Some(close) = match_angle(&out, 0) {
            let inner = out[1..close].to_string();
            if let Some(pos) = find_as(&inner) {
                generic = true;
                out = format!("<#{}>{}", &inner[pos..], &out[close + 1..]);
            }
        }
    }

    (out, generic)
}

fn match_angle(s: &str, start: usize) -> Option<usize> {
    let b = s.as_bytes();
    if b.get(start) != Some(&b'<') {
        return None;
    }
    let mut d = 0i32;
    for (i, c) in b.iter().enumerate().skip(start) {
        match c {
            b'<' => d += 1,
            b'>' => {
                d -= 1;
                if d == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
    }
    None
}

/// 找到顶层 ` as `，返回它自己（含前导空格）的下标
fn find_as(inner: &str) -> Option<usize> {
    let b = inner.as_bytes();
    let mut d = 0i32;
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'<' => d += 1,
            b'>' => d -= 1,
            b' ' if d == 0 && inner[i..].starts_with(" as ") => return Some(i),
            _ => {}
        }
        i += 1;
    }
    None
}

// ------------------------------------------------ v0 mangling 的「够用」解码器

fn is_b62(c: u8) -> bool {
    c.is_ascii_digit() || c.is_ascii_lowercase() || c.is_ascii_uppercase()
}

/// 读一个 `_` 结尾的 base62 数（消歧器、backref、生命周期都长这样）。
/// 返回新下标；调用方负责跳过分隔用的 `_` 与否，这里统一吞掉。
fn skip_b62_underscore(s: &str, mut i: usize) -> usize {
    let b = s.as_bytes();
    while i < b.len() && is_b62(b[i]) {
        i += 1;
    }
    if b.get(i) == Some(&b'_') {
        i += 1;
    }
    i
}

/// 读一个 v0 `<identifier>`：`<len><chars>`。
///
/// ⚠️ 长度**只按十进制数字读**，不能贪心吃 base62 字母。
/// v0 的 `<len>` 是没有结束符的（`10lang_start` = 长度 10 + 名字 10 个字符），
/// 若贪心按 base62 读会一路吃到后面第一个 `_`，把 `10lang` 当成长度，全盘错位。
/// `_` 和 `0` 都表示空标识符（`uE0I`、`uE0C` 这类里的 `0`）。
fn read_ident(s: &str, i: usize) -> Option<(String, usize)> {
    let b = s.as_bytes();
    if i >= b.len() {
        return None;
    }
    if b[i] == b'_' || b[i] == b'0' {
        return Some((String::new(), i + 1));
    }
    if !b[i].is_ascii_digit() {
        return None;
    }
    let start = i;
    let mut j = i;
    while j < b.len() && b[j].is_ascii_digit() {
        j += 1;
    }
    let len: usize = s[start..j].parse().ok()?;
    if len == 0 || j + len > b.len() {
        return None;
    }
    let seg = &s[j..j + len];
    // 名字里至少得有一个字母，否则更可能是被误读的数字
    if !seg.chars().any(|c| c.is_ascii_alphabetic()) {
        return None;
    }
    Some((seg.to_string(), j + len))
}

/// 基本类型单字母（`u` = `()`、`i`/`j` = 整数、`b` = bool……）。
/// 见到它们说明已经进入"类型区"，定义路径到此为止。
fn is_basic_type(c: u8) -> bool {
    matches!(
        c,
        b'a' | b'b' | b'c' | b'd' | b'e' | b'f' | b'h' | b'i' | b'j' | b'l' | b'm' | b'n' | b'o'
            | b'p' | b's' | b't' | b'u' | b'v' | b'x' | b'y' | b'z'
    )
}

/// v0 mangling 的"够用"解码器 —— 目标是**把同一个函数的所有实例化归到同一个根**，
/// 不是高保真还原类型。稳定版用户没有 nightly（用不了 `-Zdump-mono-stats` /
/// `-Zprint-mono-items`），只能从 `--emit=llvm-ir` 的 `define` 行里读符号，
/// 而那些符号是 v0 mangling。实测语料（全部来自 dynproof-mono）：
///
/// ```text
/// _RINvCsj1Qc4EzirgF_13dynproof_mono5driveNtB2_2T0EB2_
/// _RINvCsj1Qc4EzirgF_13dynproof_mono5driveNtB2_2T1EB2_
///     → 都是 dynproof_mono::drive            （200 份归一）
/// _RNvXCsj1Qc4EzirgF_13dynproof_monoNtB2_2T0NtB2_4Work4step
/// _RNvXs0_Csj1Qc4EzirgF_13dynproof_monoNtB5_2T2NtB5_4Work4step
///     → 都是 <# as Work>::step
/// _RINvNtCse0R8cuyqEa_3std2rt10lang_startuECsj1Qc4EzirgF_13dynproof_mono
///     → std::rt::lang_start                  （末尾那个 C… 是实例化方，要丢掉）
/// ```
///
/// 规则（都由上面这些真实符号反推、并有单测钉住）：
///   1. 遇到 `C`（crate 根）先记下来；**再遇到第二个 `C` 就停** —— 那是实例化方。
///   2. `N` 后面跟着一个命名空间字符（`v` 值 / `t` 类型 / `C` 闭包 / `S` shim），吃掉它；
///      若紧接着是 backref（`B…_`）或基本类型，说明进入了类型区，定义路径到此为止。
///   3. `X` = trait impl。此时取"所有 ident 的最后两段"当 `<# as Trait>::method`。
///      这一步不能靠"遇到类型区就停"—— trait 路径本身也长在 `NtB…_` 后面。
///   4. backref `B<base62>_`、消歧器 `s<base62>_`、生命周期 `L<base62>_` 都只是脚手架。
pub fn demangle_v0(sym: &str, self_crate: &str) -> Option<String> {
    let body = sym.strip_prefix("_R")?;
    let b = body.as_bytes();
    let mut i = 0usize;
    let mut path: Vec<String> = Vec::new(); // 定义路径（值命名空间）
    let mut all: Vec<String> = Vec::new(); // 全部 ident，trait impl 分支要用
    let mut has_x = false;
    let mut type_zone = false;
    let mut pushed_crate = false;

    while i < b.len() {
        let c = b[i];
        match c {
            b'C' => {
                i = skip_b62_underscore(body, i + 1);
                if pushed_crate {
                    break; // 第二个 crate = 实例化方
                }
                pushed_crate = true;
                if let Some((s, ni)) = read_ident(body, i) {
                    if !s.is_empty() {
                        all.push(s.clone());
                        if !type_zone {
                            path.push(s);
                        }
                    }
                    i = ni;
                }
            }
            b'N' => {
                i += 2; // N + 命名空间字符
                if let Some(&n) = b.get(i) {
                    if n == b'B' || is_basic_type(n) {
                        type_zone = true;
                    }
                }
            }
            b'X' => {
                has_x = true;
                i += 1;
                if b.get(i) == Some(&b's') {
                    // 消歧器
                    i = skip_b62_underscore(body, i + 1);
                }
            }
            b'B' => {
                type_zone = true;
                i = skip_b62_underscore(body, i + 1);
            }
            b'L' => {
                i = skip_b62_underscore(body, i + 1);
            }
            b'I' | b'E' | b'M' | b'Y' => i += 1,
            _ => {
                if is_basic_type(c) {
                    type_zone = true;
                    i += 1;
                } else if let Some((s, ni)) = read_ident(body, i) {
                    if !s.is_empty() {
                        all.push(s.clone());
                        if !type_zone {
                            path.push(s);
                        }
                    }
                    i = ni;
                } else {
                    i += 1;
                }
            }
        }
    }

    let strip_self = |mut v: Vec<String>| -> Vec<String> {
        if let Some(first) = v.first() {
            if first == self_crate && v.len() > 1 {
                v.remove(0);
            }
        }
        v
    };

    let key = if has_x {
        let a = strip_self(all);
        if a.len() < 2 {
            return None;
        }
        let n = a.len();
        format!("<# as {}>::{}", a[n - 2], a[n - 1])
    } else {
        let p = strip_self(path);
        if p.is_empty() {
            return None;
        }
        p.join("::")
    };
    Some(key)
}

/// 先试 v0，失败再退回"抠 `<len><ident>`"的老办法。
pub fn demangle_any(sym: &str, self_crate: &str) -> String {
    if let Some(k) = demangle_v0(sym, self_crate) {
        return k;
    }
    // legacy mangling：`_ZN…17h<hash>E` —— 去掉结尾的 hash 才能把实例化归并
    let s = demangle_crude(sym);
    match s.rfind("::") {
        Some(p) if s[p + 2..].starts_with('h') && s[p + 2..].len() >= 17 => s[..p].to_string(),
        _ => s,
    }
}

/// 极端兜底：自己从 mangled 名里抠 `<len><ident>` 片段。
/// 只在没有 nightly、走 IR 路径时用。
pub fn demangle_crude(sym: &str) -> String {
    let b = sym.as_bytes();
    let mut out: Vec<&str> = Vec::new();
    let mut i = 0;
    while i < b.len() {
        if b[i].is_ascii_digit() {
            let s = i;
            while i < b.len() && b[i].is_ascii_digit() {
                i += 1;
            }
            if let Ok(n) = sym[s..i].parse::<usize>() {
                if n > 0 && n < 200 && i + n <= b.len() {
                    let seg = &sym[i..i + n];
                    // 必须同时满足：字符集合法 **且至少含一个字母**。
                    // 少了后半条，v0 mangling 的 backref 如 `B2_2T0E` 会被切成
                    // "_2" 这种垃圾片段，还把真正的 `T0` 一起吞掉。
                    if seg.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '$')
                        && seg.chars().any(|c| c.is_ascii_alphabetic())
                    {
                        out.push(seg);
                        i += n;
                        continue;
                    }
                }
            }
            i = s + 1;
            continue;
        }
        i += 1;
    }
    if out.is_empty() {
        sym.to_string()
    } else {
        out.join("::")
    }
}

// ---------------------------------------------------------------- 采集

fn nonce() -> String {
    let n = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("xmk{n:x}")
}

fn crate_name(root: &Path) -> Option<String> {
    let txt = std::fs::read_to_string(root.join("Cargo.toml")).ok()?;
    let mut in_pkg = false;
    for line in txt.lines() {
        let t = line.trim();
        if t.starts_with('[') {
            in_pkg = t == "[package]";
            continue;
        }
        if in_pkg {
            if let Some((k, v)) = t.split_once('=') {
                if k.trim() == "name" {
                    return Some(v.trim().trim_matches('"').to_string());
                }
            }
        }
    }
    None
}

struct RunOut {
    text: String,
    code: i32,
}

fn cargo_rustc(
    root: &Path,
    pkg: &Option<String>,
    release: bool,
    extra: &[String],
    opts: &Opts,
    nightly: bool,
) -> Result<RunOut, String> {
    let mut cmd = if nightly {
        // `rustup run nightly cargo` —— 不依赖 cargo 是不是 rustup 的 shim
        let mut c = Command::new("rustup");
        c.arg("run").arg("nightly").arg(util::cargo_bin());
        c
    } else {
        Command::new(util::cargo_bin())
    };
    cmd.current_dir(root);
    cmd.arg("rustc");
    if release {
        cmd.arg("--release");
    }
    if let Some(p) = pkg {
        cmd.arg("-p").arg(p);
    }
    if let Some(td) = &opts.target_dir {
        cmd.arg("--target-dir").arg(td);
    }
    cmd.arg("--");
    for a in extra {
        cmd.arg(a);
    }
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
    let out = cmd
        .output()
        .map_err(|e| format!("调用 cargo rustc 失败: {e}（cargo/rustup 在 PATH 里吗？）"))?;
    let code = out.status.code().unwrap_or(-1);
    let mut text = String::from_utf8_lossy(&out.stderr).to_string();
    text.push_str(&String::from_utf8_lossy(&out.stdout));
    Ok(RunOut { text, code })
}

/// `-Zdump-mono-stats` 会把报告写进 **crate 根目录的 `human/`**，不是 target。
/// 这里列出目录内容，好在读完之后把"我们刚生成的那些"精确删掉。
fn list_mono_stats(root: &Path) -> Vec<PathBuf> {
    let mut v = Vec::new();
    let Ok(rd) = std::fs::read_dir(root.join("human")) else {
        return v;
    };
    for e in rd.flatten() {
        let p = e.path();
        let n = p.file_name().and_then(|s| s.to_str()).unwrap_or("");
        if n.ends_with(".mono_items.json") || n.ends_with(".mono_items.md") {
            v.push(p);
        }
    }
    v
}

/// 返回 (条目, 数据源, 诊断文本)
pub fn collect(root: &Path, opts: &Opts) -> Result<(Vec<Entry>, Source, String), String> {
    let release = opts.profile.as_deref() == Some("release");
    let pkg = opts.package.clone().or_else(|| crate_name(root));
    let nightly_ok = util::has_nightly();

    let want_stats = matches!(opts.mode, AuditMode::Stats | AuditMode::Auto) && nightly_ok;
    let want_items =
        matches!(opts.mode, AuditMode::MonoItems | AuditMode::Auto) && nightly_ok && !want_stats;

    // ---- 1. -Zdump-mono-stats（信息最全） ----
    if want_stats {
        let before = list_mono_stats(root);
        let extra = vec![
            "-Zdump-mono-stats=human".to_string(),
            "-Zdump-mono-stats-format=json".to_string(),
            format!("-Cmetadata={}", nonce()),
        ];
        let r = cargo_rustc(root, &pkg, release, &extra, opts, true)?;
        let after = list_mono_stats(root);
        let fresh: Vec<PathBuf> = after.iter().filter(|p| !before.contains(p)).cloned().collect();
        let target = fresh
            .iter()
            .find(|p| p.to_string_lossy().ends_with(".json"))
            .cloned()
            .or_else(|| {
                after
                    .iter()
                    .filter(|p| p.to_string_lossy().ends_with(".json"))
                    .max_by_key(|p| std::fs::metadata(p).and_then(|m| m.modified()).ok())
                    .cloned()
            });
        if let Some(f) = target {
            if let Ok(txt) = std::fs::read_to_string(&f) {
                let parsed = parse_mono_stats_json(&txt);
                if !parsed.is_empty() {
                    // 精确清理：只删本次新出现的报告文件，外加我们读的那个
                    let mut removed = 0usize;
                    for p in fresh.iter().chain(std::iter::once(&f)) {
                        if std::fs::remove_file(p).is_ok() {
                            removed += 1;
                        }
                    }
                    let _ = std::fs::remove_dir(root.join("human")); // 目录空了才会成功
                    let diag = format!(
                        "读到 {} 条；已清理 rustc 生成的 {removed} 个 human/ 报告文件",
                        parsed.len()
                    );
                    return Ok((parsed, Source::MonoStats, diag));
                }
            }
        }
        if opts.mode == AuditMode::Stats {
            return Err(format!(
                "没读到 mono-stats 报告（找过 {}）。cargo 输出：\n{}",
                root.join("human").display(),
                tail(&r.text, 12)
            ));
        }
        // Auto 模式：继续降级
    }

    // ---- 2. -Zprint-mono-items ----
    if want_items {
        let extra = vec![
            "-Zprint-mono-items=yes".to_string(),
            format!("-Cmetadata={}", nonce()),
        ];
        let r = cargo_rustc(root, &pkg, release, &extra, opts, true)?;
        let items = parse_mono_items(&r.text);
        if !items.is_empty() {
            return Ok((items, Source::MonoItems, String::new()));
        }
        if opts.mode == AuditMode::MonoItems {
            return Err(format!(
                "nightly 没有吐出 MONO_ITEM。原始输出：\n{}",
                tail(&r.text, 12)
            ));
        }
    }

    if opts.mode == AuditMode::MonoItems {
        return Err("--xmk-mode=mono 需要 nightly（rustup toolchain install nightly）".into());
    }
    if opts.mode == AuditMode::Stats {
        return Err("--xmk-mode=stats 需要 nightly".into());
    }

    // ---- 3. IR 兜底（stable 可用） ----
    let extra = vec![
        "--emit=llvm-ir".to_string(),
        "-Cdebuginfo=0".to_string(),
        format!("-Cmetadata={}", nonce()),
    ];
    let started = SystemTime::now();
    let r = cargo_rustc(root, &pkg, release, &extra, opts, false)?;
    if r.code != 0 {
        return Err(format!(
            "cargo rustc 失败（exit {}）:\n{}",
            r.code,
            tail(&r.text, 20)
        ));
    }
    let tdir = opts
        .target_dir
        .clone()
        .unwrap_or_else(|| util::target_dir(root));
    let ll = newest_ll(&tdir, started)?
        .ok_or_else(|| format!("没找到 .ll 文件（看过 {}）", tdir.display()))?;
    let src = std::fs::read_to_string(&ll).map_err(|e| format!("读 {} 失败: {e}", ll.display()))?;
    // 本 crate 名要拿去把 v0 符号里的"实例化方"前缀剥掉，才能和 mono 模式的
    // 观感一致（`drive` 而不是 `dynproof_mono::drive`）
    let this = pkg.as_deref().unwrap_or("").replace('-', "_");
    let items = parse_ll(&src, &this);
    if items.is_empty() {
        return Err(format!("{} 里没有 define？", ll.display()));
    }
    let diag = format!("IR: {}", ll.display());
    Ok((items, Source::LlvmIr, diag))
}

fn tail(s: &str, n: usize) -> String {
    let lines: Vec<&str> = s.lines().filter(|l| !l.trim().is_empty()).collect();
    let start = lines.len().saturating_sub(n);
    lines[start..].join("\n")
}

fn newest_ll(tdir: &Path, since: SystemTime) -> Result<Option<PathBuf>, String> {
    let mut best: Option<(SystemTime, PathBuf)> = None;
    let mut stack = vec![tdir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else {
            continue;
        };
        for e in rd.flatten() {
            let p = e.path();
            let Ok(ft) = e.file_type() else { continue };
            if ft.is_dir() {
                stack.push(p);
            } else if p.extension().map(|x| x == "ll").unwrap_or(false) {
                let mt = e.metadata().and_then(|m| m.modified()).unwrap_or(UNIX_EPOCH);
                if mt >= since - std::time::Duration::from_secs(2)
                    && best.as_ref().map(|(t, _)| mt > *t).unwrap_or(true)
                {
                    best = Some((mt, p));
                }
            }
        }
    }
    Ok(best.map(|(_, p)| p))
}

// ---------------------------------------------------------------- 解析

/// 极简 JSON 解析：只认 `[{"k":"v","k":123,…}, …]` 这一种扁平形状。
/// 引 serde_json 会打破"工具自身零依赖"这条，而这里解析的是 rustc
/// 机器生成、形状固定的东西，手写足够而且可测。
fn parse_mono_stats_json(text: &str) -> Vec<Entry> {
    let mut out = Vec::new();
    let b = text.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i] != b'{' {
            i += 1;
            continue;
        }
        // 找到对象结尾（字符串里的括号不算）
        let start = i;
        let mut d = 0i32;
        let mut in_str = false;
        let mut esc = false;
        let mut j = i;
        while j < b.len() {
            let c = b[j];
            if in_str {
                if esc {
                    esc = false;
                } else if c == b'\\' {
                    esc = true;
                } else if c == b'"' {
                    in_str = false;
                }
            } else if c == b'"' {
                in_str = true;
            } else if c == b'{' {
                d += 1;
            } else if c == b'}' {
                d -= 1;
                if d == 0 {
                    break;
                }
            }
            j += 1;
        }
        if j >= b.len() {
            break;
        }
        let obj = &text[start..=j];
        if let (Some(name), Some(count)) =
            (json_str(obj, "name"), json_num(obj, "instantiation_count"))
        {
            if !name.is_empty() {
                out.push(Entry {
                    name,
                    kind: "item".into(),
                    count: count as usize,
                    size: json_num(obj, "size_estimate"),
                    total: json_num(obj, "total_estimate"),
                });
            }
        }
        i = j + 1;
    }
    out
}

fn json_str(obj: &str, key: &str) -> Option<String> {
    let pat = format!("\"{key}\"");
    let k = obj.find(&pat)? + pat.len();
    let rest = &obj[k..];
    let rest = rest[rest.find(':')? + 1..].trim_start();
    let rest = rest.strip_prefix('"')?;
    let mut out = String::new();
    let mut it = rest.chars();
    while let Some(c) = it.next() {
        match c {
            '"' => return Some(out),
            '\\' => match it.next() {
                Some('n') => out.push('\n'),
                Some('t') => out.push('\t'),
                Some('r') => out.push('\r'),
                Some('u') => {
                    let hex: String = it.by_ref().take(4).collect();
                    if let Ok(n) = u32::from_str_radix(&hex, 16) {
                        if let Some(ch) = char::from_u32(n) {
                            out.push(ch);
                        }
                    }
                }
                Some(o) => out.push(o),
                None => break,
            },
            _ => out.push(c),
        }
    }
    None
}

fn json_num(obj: &str, key: &str) -> Option<u64> {
    let pat = format!("\"{key}\"");
    let k = obj.find(&pat)? + pat.len();
    let rest = &obj[k..];
    let rest = rest[rest.find(':')? + 1..].trim_start();
    let end = rest
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(rest.len());
    if end == 0 {
        return None;
    }
    rest[..end].parse().ok()
}

fn parse_mono_items(text: &str) -> Vec<Entry> {
    let mut v = Vec::new();
    for line in text.lines() {
        let t = line.trim();
        let Some(rest) = t.strip_prefix("MONO_ITEM ") else {
            continue;
        };
        let Some((kind, rest)) = rest.split_once(' ') else {
            continue;
        };
        let name = rest.split(" @@ ").next().unwrap_or(rest).trim();
        if name.is_empty() {
            continue;
        }
        v.push(Entry {
            name: name.to_string(),
            kind: kind.to_string(),
            count: 1,
            size: None,
            total: None,
        });
    }
    v
}

fn parse_ll(src: &str, self_crate: &str) -> Vec<Entry> {
    let mut v = Vec::new();
    for line in src.lines() {
        let Some(rest) = line.strip_prefix("define ") else {
            continue;
        };
        let Some(at) = rest.find('@') else { continue };
        let after = &rest[at + 1..];
        let end = after
            .find(|c: char| c == '(' || c == ' ')
            .unwrap_or(after.len());
        let sym = after[..end].trim_matches('"');
        v.push(Entry {
            name: demangle_any(sym, self_crate),
            kind: "define".into(),
            count: 1,
            size: None,
            total: None,
        });
    }
    v
}

// ---------------------------------------------------------------- 分组

/// 把条目按"根"聚合，`count` / `total` 相加。
///
/// 即使 `-Zdump-mono-stats` 已经把 `drive` 聚合好了，
/// `<T152 as Work>::step` 这类仍然是每个类型一条 —— 归一到 `<# as Work>::step`
/// 才能在报告里看清"有 200 份其实是 vtable 槽位"。
pub fn group(items: &[Entry]) -> (Vec<Group>, usize) {
    let mut map: HashMap<String, (usize, Option<u64>, Option<u64>, bool)> = HashMap::new();
    for it in items {
        let (key, generic) = normalize(&it.name);
        let e = map.entry(key).or_insert((0, Some(0), Some(0), false));
        e.0 += it.count;
        match (e.1.as_mut(), it.size) {
            (Some(s), Some(x)) => *s += x,
            (Some(_), None) => e.1 = None,
            _ => {}
        }
        match (e.2.as_mut(), it.total) {
            (Some(s), Some(x)) => *s += x,
            (Some(_), None) => e.2 = None,
            _ => {}
        }
        e.3 |= generic;
    }
    let roots = map.len();
    let mut groups: Vec<Group> = map
        .into_iter()
        .map(|(key, (count, size, total, from_generic))| Group {
            key,
            count,
            size,
            total,
            from_generic,
        })
        .collect();
    // 有代价模型时按代价排（这才是真正的收益排序），否则按份数
    if groups.iter().any(|g| g.total.is_some()) {
        groups.sort_by(|a, b| {
            b.total
                .unwrap_or(0)
                .cmp(&a.total.unwrap_or(0))
                .then_with(|| a.key.cmp(&b.key))
        });
    } else {
        groups.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.key.cmp(&b.key)));
    }
    (groups, roots)
}

// ---------------------------------------------------------------- 测试

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_trailing_generic_args() {
        assert_eq!(normalize("drive::<T0>"), ("drive".into(), true));
        assert_eq!(normalize("foo::<T0, T1>"), ("foo".into(), true));
    }

    #[test]
    fn normalize_trait_impl_path() {
        assert_eq!(
            normalize("<T0 as Work>::step"),
            ("<# as Work>::step".into(), true)
        );
        assert_eq!(
            normalize("<alloc::vec::Vec<u8> as Trait>::m"),
            ("<# as Trait>::m".into(), true)
        );
    }

    #[test]
    fn normalize_mid_path_generic_block() {
        assert_eq!(
            normalize("core::num::<impl u32>::pow"),
            ("core::num::<#>::pow".into(), true)
        );
    }

    #[test]
    fn normalize_plain_name_is_untouched() {
        assert_eq!(normalize("main"), ("main".into(), false));
        assert_eq!(
            normalize("std::rt::lang_start"),
            ("std::rt::lang_start".into(), false)
        );
    }

    // ---------------- v0 mangling 解码：语料全部是真实符号 ----------

    const SELF_CRATE: &str = "dynproof_mono";

    #[test]
    fn v0_collapses_every_instantiation_to_one_root() {
        // 200 个实例化只差 `2T0` / `2T1` / `3T10` … 这一处
        for suffix in ["2T0", "2T1", "2T9", "3T10", "3T99"] {
            let sym = format!("_RINvCsj1Qc4EzirgF_13dynproof_mono5driveNtB2_{suffix}EB2_");
            assert_eq!(
                demangle_v0(&sym, SELF_CRATE).as_deref(),
                Some("drive"),
                "{sym} 没归一到 drive"
            );
        }
    }

    #[test]
    fn v0_strips_trailing_instantiating_crate() {
        // 末尾那个 `C…13dynproof_mono` 是实例化方，不能进名字
        let sym = "_RINvNtCse0R8cuyqEa_3std2rt10lang_startuECsj1Qc4EzirgF_13dynproof_mono";
        assert_eq!(
            demangle_v0(sym, SELF_CRATE).as_deref(),
            Some("std::rt::lang_start")
        );
    }

    #[test]
    fn v0_trait_impl_becomes_dyn_form() {
        // 同一 trait 的不同具体类型 + 不同消歧器，都必须归到一起
        for sym in [
            "_RNvXCsj1Qc4EzirgF_13dynproof_monoNtB2_2T0NtB2_4Work4step",
            "_RNvXs0_Csj1Qc4EzirgF_13dynproof_monoNtB5_2T2NtB5_4Work4step",
            "_RNvXs10_Csj1Qc4EzirgF_13dynproof_monoNtB6_3T64NtB6_4Work4step",
        ] {
            let got = demangle_v0(sym, SELF_CRATE).unwrap();
            assert_eq!(got, "<# as Work>::step", "{sym} → {got}");
            assert!(is_trait_method(&got), "必须被认成 trait impl 形态");
        }
        // 带实例化方后缀的也要去掉后缀再取最后两段
        assert_eq!(
            demangle_v0(
                "_RNvXsU_NtCse0R8cuyqEa_3std7processuNtB5_11Termination6reportCsj1Qc4EzirgF_13dynproof_mono",
                SELF_CRATE
            )
            .as_deref(),
            Some("<# as Termination>::report")
        );
    }

    #[test]
    fn v0_plain_fn_and_main() {
        assert_eq!(
            demangle_v0("_RNvCsj1Qc4EzirgF_13dynproof_mono4main", SELF_CRATE).as_deref(),
            Some("main")
        );
    }

    #[test]
    fn v0_does_not_treat_nested_closure_ns_as_crate() {
        // `NC` 是"闭包命名空间"，不是 crate 根 —— 按"见到 C 就当 crate"会错位
        let sym = "_RNSNvYNCINvNtCse0R8cuyqEa_3std2rt10lang_startuE0INtNtNtCs8xEFJqa6dYS_4core3ops8function6FnOnceuE9call_once6vtableCsj1Qc4EzirgF_13dynproof_mono";
        assert_eq!(
            demangle_v0(sym, SELF_CRATE).as_deref(),
            Some("std::rt::lang_start")
        );
    }

    #[test]
    fn v0_rejects_non_v0_symbols() {
        // legacy mangling / 普通名字不该被 v0 解码器接住
        assert!(demangle_v0("_ZN13dynproof_mono5drive17h0123456789abcdefE", SELF_CRATE).is_none());
        assert!(demangle_v0("main", SELF_CRATE).is_none());
        // 认不出来时退回老办法，且不 panic
        assert!(!demangle_any("_ZN13dynproof_mono5drive17h0123456789abcdefE", SELF_CRATE).is_empty());
    }

    #[test]
    fn v0_ir_agrees_with_mono_ranking() {
        // 跨模式一致性：IR 兜底路径算出的排名，应该和 nightly 的
        // -Zprint-mono-items 一致（这是这套启发式规则唯一的保证手段）
        let mut ir: Vec<Entry> = Vec::new();
        for k in 0..200 {
            ir.push(Entry {
                name: demangle_any(
                    &format!("_RINvCsj1Qc4EzirgF_13dynproof_mono5driveNtB2_2T{k}EB2_"),
                    SELF_CRATE,
                ),
                kind: "define".into(),
                count: 1,
                size: None,
                total: None,
            });
        }
        for ty in ["T1", "T2"] {
            ir.push(Entry {
                name: demangle_any(
                    &format!("_RNvXCsj1Qc4EzirgF_13dynproof_monoNtB2_2{ty}NtB2_4Work4step"),
                    SELF_CRATE,
                ),
                kind: "define".into(),
                count: 1,
                size: None,
                total: None,
            });
        }
        let (groups, roots) = group(&ir);
        assert_eq!(roots, 2, "202 个符号只应归出 2 个根");
        let drive = groups.iter().find(|g| g.key == "drive").unwrap();
        assert_eq!(drive.count, 200);
        assert!(drive.duplicated());
        let step = groups.iter().find(|g| g.key == "<# as Work>::step").unwrap();
        assert_eq!(step.count, 2);
        assert!(step.is_vtable_slot());
    }

    #[test]
    fn trait_method_and_free_fn_are_distinguished() {
        assert!(is_trait_method("<# as Work>::step"));
        assert!(!is_trait_method("drive"));
    }

    #[test]
    fn crude_demangler_pulls_len_prefixed_idents() {
        let s = "_RINvCs5ZQerB2FtIq_13dynproof_mono5driveNtB2_2T0EB2_";
        let d = demangle_crude(s);
        assert!(d.contains("dynproof_mono"), "{d}");
        assert!(d.contains("drive"), "{d}");
        assert!(d.contains("T0"), "{d}");
    }

    #[test]
    fn parses_rustc_mono_stats_json() {
        // 真实格式（来自 -Zdump-mono-stats-format=json）
        let txt = r#"[
            {"name": "drive", "instantiation_count": 200, "size_estimate": 40, "total_estimate": 8000},
            {"name": "<T152 as Work>::step", "instantiation_count": 1, "size_estimate": 10, "total_estimate": 10},
            {"name": "main", "instantiation_count": 1, "size_estimate": 815, "total_estimate": 815}
        ]"#;
        let e = parse_mono_stats_json(txt);
        assert_eq!(e.len(), 3);
        assert_eq!(e[0].name, "drive");
        assert_eq!(e[0].count, 200);
        assert_eq!(e[0].total, Some(8000));
        assert_eq!(e[1].name, "<T152 as Work>::step");
    }

    #[test]
    fn groups_use_rustc_cost_and_split_vtable_slots() {
        let txt = r#"[
            {"name": "drive", "instantiation_count": 200, "size_estimate": 40, "total_estimate": 8000},
            {"name": "<T1 as Work>::step", "instantiation_count": 1, "size_estimate": 10, "total_estimate": 10},
            {"name": "<T2 as Work>::step", "instantiation_count": 1, "size_estimate": 10, "total_estimate": 10}
        ]"#;
        let e = parse_mono_stats_json(txt);
        let (g, roots) = group(&e);
        assert_eq!(roots, 2);
        let drive = g.iter().find(|x| x.key == "drive").unwrap();
        assert_eq!(drive.count, 200);
        assert_eq!(drive.total, Some(8000));
        let step = g.iter().find(|x| x.key == "<# as Work>::step").unwrap();
        assert_eq!(step.count, 2, "两个具体类型应归到同一个根");
        assert_eq!(g[0].key, "drive", "排序应按代价：8000 > 20");
    }

    #[test]
    fn reclaimable_excludes_vtable_slots() {
        // 回归测试：曾经把 vtable 槽位也算进去，报 95% 而实测 47%
        let txt = r#"[
            {"name": "drive", "instantiation_count": 200, "size_estimate": 40, "total_estimate": 8000},
            {"name": "<T1 as Work>::step", "instantiation_count": 1, "size_estimate": 10, "total_estimate": 10},
            {"name": "<T2 as Work>::step", "instantiation_count": 1, "size_estimate": 10, "total_estimate": 10}
        ]"#;
        let e = parse_mono_stats_json(txt);
        let (groups, roots) = group(&e);
        let entries: usize = e.iter().map(|x| x.count).sum();
        let rep = Report {
            package: "t".into(),
            profile: "dev".into(),
            source: Source::MonoStats,
            entries,
            groups,
            roots,
        };
        // 只有 drive 的 199 份重复可回收；step 的那份是 vtable 槽位，不算
        assert_eq!(rep.reclaimable(), 199);
        assert_eq!(rep.type_inherent(), 1);
        assert_eq!(rep.reclaimable_cost(), Some(8000 - 40));
    }

    #[test]
    fn duplicated_and_vtable_are_complementary() {
        // 回归测试：`duplicated()` 排除了 trait 方法，所以输出层若写成
        // `duplicated() && is_vtable_slot()` 会永远为假，「vtable」标签消失。
        let txt = r#"[
            {"name": "drive", "instantiation_count": 200, "size_estimate": 40, "total_estimate": 8000},
            {"name": "<T1 as Work>::step", "instantiation_count": 1, "size_estimate": 10, "total_estimate": 10},
            {"name": "<T2 as Work>::step", "instantiation_count": 1, "size_estimate": 10, "total_estimate": 10}
        ]"#;
        let e = parse_mono_stats_json(txt);
        let (groups, _) = group(&e);
        let drive = groups.iter().find(|g| g.key == "drive").unwrap();
        let step = groups.iter().find(|g| g.key == "<# as Work>::step").unwrap();

        assert!(drive.duplicated(), "drive 是实例化复制");
        assert!(!drive.is_vtable_slot());

        assert!(step.is_vtable_slot(), "step 是每类型一份的 vtable 槽位");
        assert!(!step.duplicated(), "vtable 槽位不属于「可 dyn 化」");

        // 在 count > 1 的前提下两者恰好互斥且互补
        for g in &groups {
            if g.count > 1 {
                assert!(g.duplicated() ^ g.is_vtable_slot(), "{} 判定不互斥", g.key);
            }
        }
    }
}
