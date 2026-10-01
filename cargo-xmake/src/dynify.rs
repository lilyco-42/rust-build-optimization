//! `dynify` —— 把「泛型函数」变成「dev 用 dyn / release 用泛型」的双形态。
//!
//! ⚠️ 为什么这件事必须动源码：
//! 我实测过 nightly 上所有相关的 `-Z` 开关：
//!   * `-Zshare-generics=yes` 与 `=no` 产出的 LLVM IR **字节数完全相同**（单 crate 没东西可共享）
//!   * `-Zmir-opt-level=0` / `-Zinline-mir=no` 只在 ±0.7% 里晃
//!   * `-Zpolymorphize` 已经从 nightly 1.100 移除（`unknown unstable option`）
//! 也就是说：**不存在一个能把静态派发变成动态派发的编译标志。**
//! 只能改源码，所以我们把它做成一个有审计、有开关、可回滚的显式动作，
//! 而不是偷偷摸摸的默认行为。
//!
//! 生成的形态：
//! ```ignore
//! #[cfg(debug_assertions)]                 // dev  -> 1 份
//! pub fn name(t: &dyn Trait, n: u64) -> u64 { BODY }
//!
//! #[cfg(not(debug_assertions))]            // release -> 每 T 一份，但能内联
//! pub fn name<T: Trait>(t: &T, n: u64) -> u64 { BODY }
//! ```
//! 调用点**一个字都不用改** —— `name(&x, n)` 在两种签名下都能编译，
//! 因为 `&Concrete -> &dyn Trait` 的 unsize 强转是自动发生的。
//!
//! `--switch=feature` 则用 `#[cfg(feature = "xmake-dyn")]`，由 `cargo xmake`
//! 在 dev 时自动加上 `--features xmake-dyn`。默认用 `debug_assertions`，
//! 因为它零接线：dev 默认就是 on，release 默认就是 off。

use crate::cli::Switch;
use std::path::{Path, PathBuf};

pub struct Candidate {
    pub file: PathBuf,
    pub line: usize,
    pub name: String,
    pub param: String,
    pub bound: String,
    /// `fn` 之前 + `fn name`（不含泛型表），例如 `pub fn drive`
    pub head: String,
    /// 泛型表**不含尖括号**，例如 `T: Work`
    pub generics: String,
    /// 参数表 + 返回类型，例如 `(t: &T, n: u64) -> u64`
    pub tail: String,
    /// head + <generics> + tail（原样）
    pub sig: String,
    pub body: String,
    pub start: usize,
    pub end: usize,
    /// 为什么不能改（能改则为 None）
    pub reject: Option<String>,
}

/// 字节 `b` 起的 UTF-8 序列长度（1–4）。
/// 传进来续字节（`0x80..=0xBF`）时返回 1 —— 保守，只用于"别越界地往前走"，
/// 不用于判断字符是否合法。
#[inline]
fn utf8_len(b: u8) -> usize {
    if b < 0x80 {
        1
    } else if b >= 0xF0 {
        4
    } else if b >= 0xE0 {
        3
    } else if b >= 0xC0 {
        2
    } else {
        1
    }
}

/// 按顶层逗号切分（跳过 `<>` / `()` / `[]` 里的逗号）
/// —— `T: Into<Vec<u8>>, U` 必须切成两段而不是三段。
fn split_top_commas(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut d = 0i32;
    for c in s.chars() {
        match c {
            '<' | '(' | '[' => {
                d += 1;
                cur.push(c);
            }
            '>' | ')' | ']' => {
                d -= 1;
                cur.push(c);
            }
            ',' if d == 0 => {
                out.push(cur.trim().to_string());
                cur = String::new();
            }
            _ => cur.push(c),
        }
    }
    if !cur.trim().is_empty() {
        out.push(cur.trim().to_string());
    }
    out.retain(|x| !x.is_empty());
    out
}

impl Candidate {
    pub fn ok(&self) -> bool {
        self.reject.is_none()
    }
    /// 泛型版本（release）
    pub fn generic_form(&self) -> String {
        self.sig.clone()
    }
    /// dyn 版本（dev）—— 要把被 dyn 化的那个类型参数从泛型表里摘掉，
    /// 否则 `T` 变成未使用参数，编译直接报错。
    pub fn dyn_form(&self) -> Option<String> {
        let trait_name = self.bound.split('+').next()?.trim();
        if trait_name.is_empty() {
            return None;
        }
        // 保留生命周期等其它参数，只摘掉 T
        let kept: Vec<String> = split_top_commas(&self.generics)
            .into_iter()
            .filter(|g| {
                let name = g.split(':').next().unwrap_or("").trim();
                name != self.param
            })
            .collect();
        let gen_part = if kept.is_empty() {
            String::new()
        } else {
            format!("<{}>", kept.join(", "))
        };

        let mut tail = self.tail.clone();
        for pat in [format!("&mut {}", self.param), format!("&{}", self.param)] {
            let rep = if pat.starts_with("&mut") {
                format!("&mut dyn {trait_name}")
            } else {
                format!("&dyn {trait_name}")
            };
            tail = tail.replace(&pat, &rep);
        }
        if tail == self.tail {
            return None;
        }
        Some(format!("{}{}{}", self.head, gen_part, tail))
    }

    pub fn render(&self, switch: Switch) -> String {
        let Some(dyn_sig) = self.dyn_form() else {
            return String::new();
        };
        let on = format!("#[cfg({})]", switch.cfg());
        let off = format!("#[cfg(not({}))]", switch.cfg());
        // body 自带前导换行与缩进，所以这里不要再插一个 \n。
        // 结尾**也不带**换行：`apply()` 是把它当作「item 文本」做的区间替换
        // （还会再 `trim_end()` 一次），不带尾随换行才能让原文替换区间之外的
        // 空白 —— 包括那个行尾换行 —— 一个字都不动，`revert()` 才能精确还原。
        format!(
            "{mark}\n{on}\n{dyn_sig} {{{body}}}\n\n{off}\n{generic_sig} {{{body}}}",
            mark = MARKER_DYN,
            on = on,
            off = off,
            dyn_sig = dyn_sig,
            generic_sig = self.sig,
            body = self.body
        )
    }
}

/// 我们在生成的代码里留的标记。`undo` 靠它精确找到"一对 cfg 分支"并折叠回原状 ——
/// 所以它必须**稳定、唯一、可 grep**。
pub const MARKER_DYN: &str = "// cargo-xmake:dyn";

/// `render()` 的**逆变换**：把
///
/// ```ignore
/// // cargo-xmake:dyn
/// #[cfg(debug_assertions)]
/// pub fn f(t: &dyn Trait, n: u64) -> u64 { … }
///
/// #[cfg(not(debug_assertions))]
/// pub fn f<T: Trait>(t: &T, n: u64) -> u64 { … }
/// ```
///
/// 折叠回后面那个泛型版本（并去掉它的 `#[cfg]` 属性）。
/// 返回 (新内容, 折叠了几处)。
///
/// 为什么**不**用 `.xmake-bak` 备份来还原源码：
///   * 备份只在"验证失败回滚"期间有意义；成功后清掉，用户就再也回不去了；
///   * 若把备份留着，`undo` 就会覆盖用户在 dynify **之后**对源码做的编辑 —— 那更危险。
/// 用标记做结构性逆变换没有这两个问题：只动我们生成的那一对分支，
/// 其它编辑一概不碰，而且跨会话有效。
pub fn revert(src: &str) -> (String, usize) {
    let mut out = String::new();
    let mut i = 0usize;
    let mut n = 0usize;
    loop {
        let Some(rel) = src[i..].find(MARKER_DYN) else {
            break;
        };
        let at = i + rel;
        let line_start = src[..at].rfind('\n').map(|x| x + 1).unwrap_or(0);
        let sc = Scanner::new(src);

        // 第一个分支（dev / dyn）
        let Some(c1r) = src[at..].find("#[cfg(") else { break };
        let Some(nl1) = src[at + c1r..].find('\n') else { break };
        let i1 = at + c1r + nl1 + 1;
        let Some(o1) = src[i1..].find('{') else { break };
        let Some(e1) = sc.match_brace(i1 + o1, b'{', b'}') else { break };

        // 第二个分支（release / 静态分发）—— 就是我们要留下的那份
        let Some(c2r) = src[e1 + 1..].find("#[cfg(not(") else { break };
        let c2 = e1 + 1 + c2r;
        let Some(nl2) = src[c2..].find('\n') else { break };
        let i2 = c2 + nl2 + 1;
        let Some(o2) = src[i2..].find('{') else { break };
        let Some(e2) = sc.match_brace(i2 + o2, b'{', b'}') else { break };

        out.push_str(&src[i..line_start]);
        out.push_str(&src[i2..=e2]);
        n += 1;
        // 什么都不额外消费：`render()` 的契约就是**输出不带尾随换行**
        // （见其实现与 `render_has_no_trailing_newline` 测试），
        // 所以 `i2..=e2` 之后正好落在原文替换区间结束处，退一个字符就是错位。
        i = e2 + 1;
    }
    out.push_str(&src[i..]);
    (out, n)
}

/// `revert` 该扫哪些文件：与 `scan_paths` 完全一致（共用 `collect_from_roots`），
/// 否则会出现"扫的时候改了、撤的时候没撤到"的漏还原。
pub fn rs_files(paths: &[String], root: &Path) -> Vec<PathBuf> {
    collect_from_roots(paths, root)
}

// ---------------------------------------------------------------- 扫描

struct Scanner<'a> {
    s: &'a [u8],
    i: usize,
}

impl<'a> Scanner<'a> {
    fn new(s: &'a str) -> Self {
        Scanner {
            s: s.as_bytes(),
            i: 0,
        }
    }
    fn line_of(&self, pos: usize) -> usize {
        self.s[..pos.min(self.s.len())]
            .iter()
            .filter(|&&c| c == b'\n')
            .count()
            + 1
    }
    /// 跳到下一个 ident 边界。
    ///
    /// ⚠️ 两个循环都必须**按整个 UTF-8 字符**步进，不能 `i += 1` 乱走：
    /// 否则 `i` 会停在多字节字符中间，`start` 就不再是字符边界，
    /// 后面的 `from_utf8_lossy(&s[start..i])` 或任何 `&text[..]` 切片都会崩。
    fn next_word(&mut self) -> Option<(usize, String)> {
        while self.i < self.s.len() {
            let c = self.s[self.i];
            // 首字符：ASCII 字母 / `_` / 任何非 ASCII（Rust 允许 Unicode 标识符）
            if c.is_ascii_alphabetic() || c == b'_' || c >= 0x80 {
                let start = self.i;
                while self.i < self.s.len() {
                    let d = self.s[self.i];
                    if d.is_ascii_alphanumeric() || d == b'_' {
                        self.i += 1;
                    } else if d >= 0x80 {
                        self.i = (self.i + utf8_len(d)).min(self.s.len());
                    } else {
                        break;
                    }
                }
                // start..self.i 一定落在字符边界上，所以这里能安全地切片
                let w = String::from_utf8_lossy(&self.s[start..self.i]).to_string();
                return Some((start, w));
            }
            self.i += utf8_len(c);
        }
        None
    }
    fn skip_ws(&mut self) {
        while self.i < self.s.len() && self.s[self.i].is_ascii_whitespace() {
            self.i += 1;
        }
    }
    /// 从当前 `i`（应为某个开括号）匹配到对应闭括号，返回闭括号位置。
    /// ⚠️ 必须跳过 `->` —— 否则返回类型的箭头里的 `>` 会被当成 `>` 的闭合，
    /// 于是 `<T: Fn() -> u64>` 这种约束和 `fn f<T>(..) -> R {` 都会算错。
    fn match_brace(&self, from: usize, open: u8, close: u8) -> Option<usize> {
        let mut d = 0i32;
        let mut i = from;
        while i < self.s.len() {
            let c = self.s[i];
            if c == b'-' && self.s.get(i + 1) == Some(&b'>') {
                i += 2;
                continue;
            }
            if c == open {
                d += 1;
            } else if c == close {
                d -= 1;
                if d == 0 {
                    return Some(i);
                }
            } else if c == b'"' {
                // 跳过字符串字面量
                i += 1;
                while i < self.s.len() && self.s[i] != b'"' {
                    if self.s[i] == b'\\' {
                        i += 1;
                    }
                    i += 1;
                }
            } else if c == b'/' && self.s.get(i + 1) == Some(&b'/') {
                while i < self.s.len() && self.s[i] != b'\n' {
                    i += 1;
                }
            }
            i += 1;
        }
        None
    }
}

pub fn scan_file(path: &Path) -> Result<Vec<Candidate>, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("读 {} 失败: {e}", path.display()))?;
    Ok(scan_text(&text, path))
}

pub fn scan_text(text: &str, path: &Path) -> Vec<Candidate> {
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mut i = 0usize;
    while i < bytes.len() {
        // 找 `fn`（前面不能是 ident 字符）
        if bytes[i..].starts_with(b"fn")
            && (i == 0 || !(bytes[i - 1].is_ascii_alphanumeric() || bytes[i - 1] == b'_'))
        {
            let after = i + 2;
            if !bytes
                .get(after)
                .map(|c| c.is_ascii_whitespace())
                .unwrap_or(false)
            {
                i += 1;
                continue;
            }
            // 往前吃掉 `pub` / `pub(crate)` / `unsafe` / `async` / `const` / 属性，
            // ⚠️ 这一步必须做：否则替换区间从 `fn` 开始，`pub` 会被留在外面，
            // 生成 `pub #[cfg(...)] fn ...`，编译器直接报
            // "visibility `pub` is not followed by an item"。
            let istart = item_start(text, i);
            if let Some(c) = parse_fn(text, i, istart, path) {
                i = c.end.max(i + 1);
                out.push(c);
                continue;
            }
        }
        i += 1;
    }
    out
}

/// 从 `fn` 关键字位置向前扩展，覆盖可见性修饰符、`unsafe`/`async`/`const`/`extern`
/// 以及 `#[...]` 属性，返回整个 item 的起点。
///
/// ⚠️ 这里所有按字节回溯的判断**必须用 `is_ascii_whitespace` / `is_ascii_alphanumeric`**，
/// 绝不能用 `(b as char).is_whitespace()` —— 那是个会 panic 的陷阱：
/// `b` 是单个字节，`0xBA as char` 是 `'º'`，而 `'º'.is_alphanumeric() == true`；
/// 中文 UTF-8 的续字节里 0xBA 极常见（`示` = `E7 A4 BA`、`中` 附近俯拾皆是），
/// 于是回溯会一头扎进多字节字符的中间，随后的 `&text[k..end]` 直接
/// `byte index N is not a char boundary` panic。
/// （`is_whitespace` 同理：`0xA0 as char` 是 NBSP、`0x85 as char` 是 NEL，都算空白。）
/// ASCII 字节永远不会出现在多字节序列内部，所以 ASCII 判断天然安全。
fn item_start(text: &str, fn_pos: usize) -> usize {
    let b = text.as_bytes();
    let mut i = fn_pos;
    loop {
        // 向前跳过空白
        let mut j = i;
        while j > 0 && b[j - 1].is_ascii_whitespace() {
            j -= 1;
        }
        if j == 0 {
            return i;
        }
        let end = j;
        let last = b[end - 1];

        // `pub(crate)` / `pub(in path)`：回溯到匹配的 '('
        if last == b')' {
            let mut d = 0i32;
            let mut p = end;
            let mut open = None;
            while p > 0 {
                p -= 1;
                if b[p] == b')' {
                    d += 1;
                } else if b[p] == b'(' {
                    d -= 1;
                    if d == 0 {
                        open = Some(p);
                        break;
                    }
                }
            }
            let Some(op) = open else { return i };
            let mut q = op;
            while q > 0 && b[q - 1].is_ascii_whitespace() {
                q -= 1;
            }
            let mut r = q;
            while r > 0 && (b[r - 1].is_ascii_alphanumeric() || b[r - 1] == b'_') {
                r -= 1;
            }
            if &text[r..q] == "pub" {
                i = r;
                continue;
            }
            return i;
        }

        // `#[...]` 属性
        if last == b']' {
            let mut d = 0i32;
            let mut p = end;
            let mut open = None;
            while p > 0 {
                p -= 1;
                if b[p] == b']' {
                    d += 1;
                } else if b[p] == b'[' {
                    d -= 1;
                    if d == 0 {
                        open = Some(p);
                        break;
                    }
                }
            }
            let Some(op) = open else { return i };
            let mut q = op;
            while q > 0 && b[q - 1].is_ascii_whitespace() {
                q -= 1;
            }
            if q > 0 && b[q - 1] == b'#' {
                i = q - 1;
                continue;
            }
            return i;
        }

        // `extern "C"` 里的字符串字面量
        if last == b'"' {
            let mut p = end - 1;
            while p > 0 && b[p - 1] != b'"' {
                p -= 1;
            }
            if p == 0 {
                return i;
            }
            i = p - 1;
            continue;
        }

        // 普通标识符：pub / unsafe / async / const / extern
        let mut k = end;
        while k > 0 && (b[k - 1].is_ascii_alphanumeric() || b[k - 1] == b'_') {
            k -= 1;
        }
        let word = &text[k..end];
        if matches!(word, "pub" | "unsafe" | "async" | "const" | "extern") {
            i = k;
            continue;
        }
        return i;
    }
}

fn parse_fn(text: &str, fn_pos: usize, start: usize, path: &Path) -> Option<Candidate> {
    let b = text.as_bytes();
    let mut s = Scanner::new(text);
    s.i = fn_pos + 2;
    s.skip_ws();
    let (_, name) = s.next_word()?;

    s.skip_ws();
    // 泛型块
    let gen_open = s.i;
    let generics = if b.get(s.i) == Some(&b'<') {
        let close = s.match_brace(s.i, b'<', b'>')?;
        let g = text[s.i + 1..close].trim().to_string();
        s.i = close + 1;
        g
    } else {
        return None; // 没有泛型就没得改
    };

    let head = text[start..gen_open].trim_end().to_string();

    s.skip_ws();
    if b.get(s.i) != Some(&b'(') {
        return None;
    }
    let params_open = s.i;
    let close_params = s.match_brace(s.i, b'(', b')')?;
    let params = text[s.i + 1..close_params].to_string();
    s.i = close_params + 1;

    // 返回类型 / where / 函数体
    s.skip_ws();
    let ret_start = s.i;
    let body_start = {
        // 找到第一个顶层 `{`（同样要跳过 `->`）
        let mut d = 0i32;
        let mut j = s.i;
        let mut found = None;
        while j < b.len() {
            if b[j] == b'-' && b.get(j + 1) == Some(&b'>') {
                j += 2;
                continue;
            }
            match b[j] {
                b'<' | b'(' | b'[' => d += 1,
                b'>' | b')' | b']' => d -= 1,
                b'{' if d == 0 => {
                    found = Some(j);
                    break;
                }
                b';' if d == 0 => break, // 只有签名，没有 body（trait 里的声明）
                _ => {}
            }
            j += 1;
        }
        found?
    };
    let ret = text[ret_start..body_start].trim().to_string();
    let body_end = s.match_brace(body_start, b'{', b'}')?;
    let body = text[body_start + 1..body_end].to_string();
    let end = body_end + 1;

    // 完整签名（`fn` 到 `{` 之前），拆成 head / generics / tail 三块，
    // 这样 dyn 版可以只摘掉被 dyn 化的那个类型参数。
    let tail = text[params_open..body_start].trim_end().to_string();
    let sig = format!("{head}<{generics}>{tail}");

    // ---- 判定可改性 ----
    let type_params: Vec<&str> = generics
        .split(',')
        .map(|x| x.trim())
        .filter(|x| {
            let head = x.split(':').next().unwrap_or("").trim();
            !head.is_empty()
                && head.chars().next().map(|c| c.is_ascii_uppercase()).unwrap_or(false)
                && !x.contains("const ")
        })
        .collect();

    let mut reject = None;
    // 非 ASCII 函数名（中文标识符）：我们**不做**自动改写，显式拒绝。
    // 原因是不想靠"恰好解析对了"—— 名字由 `next_word` 取，若名字非 ASCII
    // 而代码里又没做字符边界处理，很容易取到后半截，进而改到别的东西上。
    // 实测真实工程里几乎不存在这种函数（只有中文**注释**才是常态），
    // 所以拒绝的代价远低于误改的代价。
    if !name.is_ascii() {
        reject = Some(format!("函数名 `{name}` 含非 ASCII 字符，暂不自动改写（请手工处理）"));
    } else if type_params.len() != 1 {
        reject = Some(format!("泛型参数有 {} 个，只支持 1 个", type_params.len()));
    }
    let param = type_params.first().map(|p| p.split(':').next().unwrap_or("").trim().to_string());
    let bound = type_params
        .first()
        .and_then(|p| p.split_once(':').map(|(_, b)| b.trim().to_string()));

    let (Some(param), Some(bound)) = (param.clone(), bound.clone()) else {
        return Some(Candidate {
            file: path.to_path_buf(),
            line: s.line_of(start),
            name,
            param: String::new(),
            bound: String::new(),
            head,
            generics,
            tail,
            sig,
            body,
            start,
            end,
            reject: Some("泛型参数没有 trait 约束".into()),
        });
    };

    if reject.is_none() {
        if bound.contains('?') || bound.contains("const ") {
            reject = Some("约束里有 `?Sized`/`const`，dyn 化不安全".into());
        } else if bound.contains('+') {
            reject = Some(format!("约束是 `{bound}`（多约束），需要手工决定用哪个 trait 对象"));
        } else if !params.contains(&format!("&{param}")) && !params.contains(&format!("&mut {param}")) {
            reject = Some(format!("`{param}` 没有以 `&{param}`/`&mut {param}` 出现在参数里"));
        } else if ret.contains(&param) {
            reject = Some(format!("返回类型里出现了 `{param}`，dyn 版没法表达"));
        } else if !body.is_empty() && body.contains(&format!("{param}::")) {
            reject = Some(format!("函数体里用了 `{param}::` 关联函数，对象安全不成立"));
        }
    }

    Some(Candidate {
        file: path.to_path_buf(),
        line: s.line_of(start),
        name,
        param,
        bound,
        head,
        generics,
        tail,
        sig,
        body,
        start,
        end,
        reject,
    })
}

/// 扫描一个目录下所有 .rs
/// 校验用户显式给的路径。**不存在的路径必须报错**，不能默默扫到 0 个 ——
/// 否则打错一个字得到的是"这个项目没有泛型热点"，那是个会被当成结论的假数据。
///
/// 顺带接受 Git Bash 的 `/d/...` 形态（Windows 上它不是绝对路径，会拼错）。
pub fn check_paths(paths: &[String], root: &Path) -> Result<(), String> {
    let mut bad: Vec<String> = Vec::new();
    for r in crate::util::scan_roots(paths, root) {
        if !r.exists() {
            bad.push(r.display().to_string());
        }
    }
    if bad.is_empty() {
        return Ok(());
    }
    Err(format!(
        "路径不存在：{}\n  提示：给的是目录或单个 .rs 文件都行；相对路径按 {} 解析。",
        bad.join("\n            "),
        root.display()
    ))
}

/// 按扫描范围收集 `.rs` 文件。`scan_paths`（扫描）与 `rs_files`（还原）共用同一套
/// "扫哪些文件"的逻辑 —— 两边一旦不一致，`undo` 就会漏还原一部分文件。
fn collect_from_roots(paths: &[String], root: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = Vec::new();
    for r in crate::util::scan_roots(paths, root) {
        if r.is_dir() {
            collect_rs(&r, &mut files);
        } else {
            files.push(r);
        }
    }
    files.sort();
    files.dedup();
    files
}

pub fn scan_paths(paths: &[String], root: &Path) -> Result<Vec<Candidate>, String> {
    let files = collect_from_roots(paths, root);
    let mut out = Vec::new();
    for f in files {
        out.extend(scan_file(&f)?);
    }
    out.sort_by(|a, b| {
        b.body.len()
            .cmp(&a.body.len())
            .then_with(|| a.file.cmp(&b.file))
    });
    Ok(out)
}

fn collect_rs(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            let n = p.file_name().and_then(|s| s.to_str()).unwrap_or("");
            if n != "target" && n != ".git" {
                collect_rs(&p, out);
            }
        } else if p.extension().map(|x| x == "rs").unwrap_or(false) {
            out.push(p);
        }
    }
}

/// 备份文件名：`<原名>.xmake-bak`（例如 `main.rs.xmake-bak`）。
/// ⚠️ 不能用 `Path::with_extension("rs")` 反推原名 —— `main.rs.xmake-bak` 的
/// stem 是 `main.rs`，加回 `.rs` 会变成 `main.rs.rs`，于是"回滚"写到了隔壁文件、
/// 原文件保持坏状态，且不报任何错。这里一律用字符串后缀拼接。
fn bak_name(file: &Path) -> PathBuf {
    let mut s = file.as_os_str().to_os_string();
    s.push(".xmake-bak");
    PathBuf::from(s)
}

fn unbak_name(bak: &Path) -> Option<PathBuf> {
    let s = bak.to_str()?;
    s.strip_suffix(".xmake-bak").map(PathBuf::from)
}

/// 真正落盘改写。返回 (改了几个, 备份路径)
pub fn apply(cands: &[Candidate], switch: Switch) -> Result<(usize, Vec<PathBuf>), String> {
    let mut by_file: std::collections::HashMap<&PathBuf, Vec<&Candidate>> =
        std::collections::HashMap::new();
    for c in cands.iter().filter(|c| c.ok()) {
        by_file.entry(&c.file).or_default().push(c);
    }

    let mut backups = Vec::new();
    let mut n = 0;
    for (file, mut list) in by_file {
        // 从后往前替换，避免偏移失效
        list.sort_by(|a, b| b.start.cmp(&a.start));
        let text = std::fs::read_to_string(file).map_err(|e| format!("读 {} 失败: {e}", file.display()))?;
        let mut out = text.clone();
        for c in list {
            let rendered = c.render(switch);
            if rendered.is_empty() {
                continue;
            }
            if c.start >= out.len() || c.end > out.len() {
                return Err(format!("{} 的偏移失效，已中止（未写入）", file.display()));
            }
            out.replace_range(c.start..c.end, rendered.trim_end());
            n += 1;
        }
        // 先写备份，再写正文；正文写失败立刻回滚
        let bak = bak_name(file);
        std::fs::write(&bak, &text).map_err(|e| format!("写备份 {} 失败: {e}", bak.display()))?;
        if let Err(e) = std::fs::write(file, out) {
            let _ = std::fs::write(file, &text);
            let _ = std::fs::remove_file(&bak);
            return Err(format!("写 {} 失败，已回滚: {e}", file.display()));
        }
        backups.push(bak);
    }
    Ok((n, backups))
}

/// 回滚。返回被恢复的文件列表。
pub fn restore(backups: &[PathBuf]) -> Result<Vec<PathBuf>, String> {
    let mut done = Vec::new();
    for b in backups {
        let orig = unbak_name(b).ok_or_else(|| format!("备份名看不懂: {}", b.display()))?;
        let text = std::fs::read_to_string(b).map_err(|e| format!("读备份失败: {e}"))?;
        std::fs::write(&orig, text).map_err(|e| format!("恢复 {} 失败: {e}", orig.display()))?;
        let _ = std::fs::remove_file(b);
        done.push(orig);
    }
    Ok(done)
}

// ---------------------------------------------------------------- 测试

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn scan(src: &str) -> Vec<Candidate> {
        scan_text(src, Path::new("t.rs"))
    }

    #[test]
    fn finds_rewritable_generic_fn() {
        let src = "pub fn drive<T: Work>(t: &T, n: u64) -> u64 {\n    t.step(n)\n}\n";
        let c = scan(src);
        assert_eq!(c.len(), 1);
        let c = &c[0];
        assert!(c.ok(), "应该可改，但被拒：{:?}", c.reject);
        assert_eq!(c.name, "drive");
        // 关键：dyn 版必须把 T 从泛型表里摘掉，否则 unused param 直接报错
        assert_eq!(c.dyn_form().unwrap(), "pub fn drive(t: &dyn Work, n: u64) -> u64");
        assert_eq!(c.generic_form(), "pub fn drive<T: Work>(t: &T, n: u64) -> u64");
    }

    #[test]
    fn keeps_other_generic_params_but_drops_the_dynified_one() {
        let src = "fn f<'a, T: Work>(t: &T, s: &'a str) -> usize { t.step(0) as usize + s.len() }\n";
        let c = scan(src);
        assert_eq!(c.len(), 1);
        let d = c[0].dyn_form().unwrap();
        assert!(d.starts_with("fn f<'a>("), "{d}");
        assert!(d.contains("t: &dyn Work"), "{d}");
    }

    #[test]
    fn replacement_includes_visibility_and_attributes() {
        // `pub` / `#[inline]` 必须落进替换区间，否则会生成
        // `pub #[cfg(..)] fn` —— 编译器报 "visibility `pub` is not followed by an item"
        let src = "#[inline]\npub fn go<T: Work>(t: &T) -> u64 { t.step(1) }\n";
        let c = scan(src);
        assert_eq!(c.len(), 1);
        let rendered = c[0].render(Switch::DebugAssertions);
        assert!(!rendered.contains("pub #[cfg"), "{rendered}");
        assert!(rendered.contains("pub fn go(t: &dyn Work) -> u64"), "{rendered}");
        // `#[inline]` 必须被保留在签名里（它属于 head 的一部分），
        // 而且要排在我们的 cfg **之后**，否则 cfg 会作用到错误的东西上
        let cfg_at = rendered.find("#[cfg(debug_assertions)]").unwrap();
        let inline_at = rendered.find("#[inline]").unwrap();
        assert!(cfg_at < inline_at, "{rendered}");
    }

    #[test]
    fn rejects_when_return_type_mentions_the_param() {
        let src = "fn make<T: Work>(t: &T) -> T { t.clone() }\n";
        let c = scan(src);
        assert_eq!(c.len(), 1);
        assert!(c[0].reject.is_some());
        assert!(c[0].reject.as_deref().unwrap().contains("返回类型"));
    }

    #[test]
    fn rejects_associated_function_use() {
        let src = "fn go<T: Work>(t: &T) -> u64 { T::default_n() + t.step(1) }\n";
        let c = scan(src);
        assert_eq!(c.len(), 1);
        assert!(c[0].reject.is_some());
    }

    #[test]
    fn ignores_non_generic_functions() {
        let src = "fn plain(t: &u64) -> u64 { *t }\nfn main() {}\n";
        assert!(scan(src).is_empty());
    }

    #[test]
    fn arrow_does_not_break_brace_matching() {
        // `->` 里的 `>` 如果被当成泛型闭合，body 就找不到，整个函数被漏掉
        let src = "fn go<T: Fn() -> u64>(f: &T, n: u64) -> u64 { f() + n }\n";
        let c = scan(src);
        assert_eq!(c.len(), 1, "箭头搞坏了括号匹配");
    }

    #[test]
    fn split_top_commas_respects_nesting() {
        assert_eq!(
            split_top_commas("T: Into<Vec<u8>>, U, 'a"),
            vec!["T: Into<Vec<u8>>", "U", "'a"]
        );
    }

    /// 曾经用 `with_extension("rs")` 反推备份名，得到 `main.rs.rs` ——
    /// 于是"回滚"写到了隔壁文件、原文件保持坏状态且不报错。
    #[test]
    fn backup_naming_roundtrips() {
        let f = Path::new(r"D:\p\src\main.rs");
        let b = bak_name(f);
        assert_eq!(b.to_str().unwrap(), r"D:\p\src\main.rs.xmake-bak");
        assert_eq!(unbak_name(&b).unwrap(), PathBuf::from(f));
    }

    /// 核心保证：`render()` 之后 `revert()` 必须**逐字节**回到原文。
    /// 这条测试不存在的话，"undo 能还原源码"就只是一句口号。
    #[test]
    fn render_then_revert_roundtrips_byte_for_byte() {
        let original = "\
use std::fmt::Debug;

pub trait Work {
    fn step(&self, x: u64) -> u64;
}

#[inline]
pub fn drive<T: Work>(t: &T, n: u64) -> u64 {
    let mut acc = 0u64;
    for i in 0..n {
        acc = acc.wrapping_add(t.step(acc ^ i));
    }
    acc
}

fn helper() -> u64 {
    let x = 1u64;
    x + 2
}
";
        let cands = scan_text(original, Path::new("src/main.rs"));
        assert_eq!(cands.len(), 1, "应该只认出 drive 一个");
        let c = &cands[0];

        // apply 的替换区间就是 item 本身
        let rewritten = format!(
            "{}{}{}",
            &original[..c.start],
            c.render(Switch::DebugAssertions),
            &original[c.end..]
        );
        assert_ne!(rewritten, original, "改写应该真的改了东西");
        assert!(rewritten.contains(MARKER_DYN), "必须留下标记，否则 revert 找不到");

        let (back, n) = revert(&rewritten);
        assert_eq!(n, 1, "应该折叠 1 处");
        assert_eq!(back, original, "revert 必须逐字节回到原文");
    }

    /// 用户自己在 dynify 之后改了别处，`revert` 不能碰那些编辑
    #[test]
    fn revert_leaves_unrelated_edits_alone() {
        let original = "pub fn drive<T: Work>(t: &T, n: u64) -> u64 {\n    t.step(n)\n}\n";
        let cands = scan_text(original, Path::new("src/main.rs"));
        let c = &cands[0];
        let rewritten = format!(
            "{}{}{}",
            &original[..c.start],
            c.render(Switch::DebugAssertions),
            &original[c.end..]
        );
        // 用户在文件末尾加了个函数
        let edited = format!("{rewritten}\npub fn added() -> u64 {{\n    7\n}}\n");
        let (back, n) = revert(&edited);
        assert_eq!(n, 1);
        assert!(back.contains("pub fn added()"), "无关编辑必须保留");
        assert_eq!(
            back,
            "pub fn drive<T: Work>(t: &T, n: u64) -> u64 {\n    t.step(n)\n}\n\npub fn added() -> u64 {\n    7\n}\n"
        );
    }

    /// 没有标记的普通代码原样返回（幂等），文件末尾没有换行也不能丢字符
    #[test]
    fn revert_is_a_noop_without_marker() {
        for s in ["fn main() {}\n", "fn main() {}", ""] {
            let (back, n) = revert(s);
            assert_eq!(n, 0);
            assert_eq!(back, s, "无标记时必须原样返回：{s:?}");
        }
    }

    /// 这条是 `revert` 的**前置契约**：它按「不额外消费换行」写，
    /// 一旦 `render()` 哪天又补上一个尾随 `\n`，`undo` 就会每次多留一个空行。
    /// 钉死它，免得以后有人"顺手"加回去。
    #[test]
    fn render_has_no_trailing_newline() {
        let src = "pub trait Work {\n    fn step(&self, x: u64) -> u64;\n}\n\npub fn drive<T: Work>(t: &T, n: u64) -> u64 {\n    t.step(n)\n}\n";
        let c = &scan_text(src, Path::new("m.rs"))[0];
        for sw in [Switch::DebugAssertions, Switch::Feature] {
            let r = c.render(sw);
            assert!(!r.is_empty());
            assert!(
                !r.ends_with('\n'),
                "render() 不能带尾随换行，否则 revert 会多留空行：{r:?}"
            );
            // 以泛型签名的收尾大括号结束
            assert!(r.ends_with('}'), "render() 应以收尾大括号结束：{r:?}");
        }
    }

    /// 🔴 真实项目回归：中文注释/中文字符串把扫描器搞 panic 过。
    /// `item_start` 里用 `(b as char).is_alphanumeric()` 判断标识符时，
    /// `0xBA as char` 是 `'º'`（**是**字母），而 `示` = `E7 A4 BA` ——
    /// 于是回溯一头扎进多字节字符中间，`&text[k..end]` 直接
    /// `not a char boundary` panic。修法是全部改成 ASCII 判断。
    #[test]
    fn chinese_source_does_not_panic_the_scanner() {
        // 属性 + `pub` 前缀 + 中文注释 + 中文字符串字面量，把 `item_start`
        // 往回走的每条分支都踩一遍（`pub(crate)` / `#[attr]` / 中文前置字符）
        let src = "\
// 这一行是中文注释：表示、中间、电话
pub(crate) fn 前面的中文() {}

/// 文档注释：中文说明，含「表示」与「中间」
#[inline]
#[must_use]
pub fn drive<T: Work>(t: &T, n: u64) -> u64 {
    // 函数体里也有中文：信号、显示
    let _ = \"中文字符串字面量 表示\";
    t.step(n)
}
";
        let cands = scan_text(src, Path::new("中文.rs"));
        assert_eq!(cands.len(), 1, "应找到 1 个泛型函数 drive");
        assert_eq!(cands[0].name, "drive");
        assert!(cands[0].ok(), "drive 应可改写：{:?}", cands[0].reject);
        // 整个 item 头（含属性和 pub）都要被覆盖 —— 注意属性在 sig 里是保留的
        assert!(
            cands[0].sig.contains("#[inline]") && cands[0].sig.contains("pub fn drive"),
            "sig={}",
            cands[0].sig
        );
        // 往返也不能崩
        let rendered = cands[0].render(Switch::DebugAssertions);
        let rewritten =
            format!("{}{}{}", &src[..cands[0].start], rendered.trim_end(), &src[cands[0].end..]);
        let (back, n) = revert(&rewritten);
        assert_eq!(n, 1);
        assert_eq!(back, src, "中文源码往返必须逐字节相同");
    }

    /// 中文标识符自己也是多字节：必须能识别出这个函数（不静默漏掉），
    /// 但**显式拒绝**改写（不靠"恰好解析对了"），且全程不 panic。
    #[test]
    fn non_ascii_identifiers_are_reported_not_misparsed() {
        let src = "pub fn 处理<T: Work>(t: &T, n: u64) -> u64 {\n    t.step(n)\n}\n";
        let cands = scan_text(src, Path::new("cn.rs"));
        assert_eq!(cands.len(), 1, "应能找到这个函数，而不是静默漏掉");
        assert_eq!(cands[0].name, "处理", "名字要完整取到，不能只取到后半截");
        assert!(!cands[0].ok(), "非 ASCII 名应被拒绝");
        assert!(
            cands[0].reject.as_deref().unwrap_or("").contains("非 ASCII"),
            "拒绝原因要说明是名字的问题：{:?}",
            cands[0].reject
        );
    }
}
