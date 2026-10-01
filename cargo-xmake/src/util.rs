//! 小工具：路径、进程、终端着色。刻意不引任何 crate。

use std::env;
use std::ffi::OsString;
use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

// ---------------------------------------------------------------- 终端着色

#[derive(Clone, Copy)]
pub struct Style {
    on: bool,
}

impl Style {
    pub fn new() -> Self {
        let on = env::var_os("NO_COLOR").is_none()
            && match env::var("CARGO_TERM_COLOR").ok().as_deref() {
                Some("never") => false,
                Some("always") => true,
                _ => std::io::stdout().is_terminal(),
            };
        Style { on }
    }
    fn wrap(&self, code: &str, s: &str) -> String {
        if self.on {
            format!("\x1b[{code}m{s}\x1b[0m")
        } else {
            s.to_string()
        }
    }
    /// 加粗
    pub fn b(&self, s: &str) -> String {
        self.wrap("1", s)
    }
    /// 加粗（别名，读起来顺一点）
    pub fn bold(&self, s: &str) -> String {
        self.b(s)
    }
    /// 暗色
    pub fn dim(&self, s: &str) -> String {
        self.wrap("2", s)
    }
    pub fn green(&self, s: &str) -> String {
        self.wrap("32", s)
    }
    pub fn yellow(&self, s: &str) -> String {
        self.wrap("33", s)
    }
    pub fn red(&self, s: &str) -> String {
        self.wrap("31", s)
    }
    pub fn cyan(&self, s: &str) -> String {
        self.wrap("36", s)
    }
    /// 涨红跌绿是国内惯例，这里用不到，但保留一个中性高亮
    pub fn bold_cyan(&self, s: &str) -> String {
        self.wrap("1;36", s)
    }
}

// ---------------------------------------------------------------- 工具链探测

/// `rustc -vV` 的 host 三元组，例如 `x86_64-pc-windows-msvc`。
pub fn host_triple(extra_env: &[(String, String)]) -> Option<String> {
    let mut cmd = Command::new(rustc_bin());
    cmd.arg("-vV").stdout(Stdio::piped()).stderr(Stdio::null());
    for (k, v) in extra_env {
        cmd.env(k, v);
    }
    let out = cmd.output().ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    text.lines()
        .find_map(|l| l.strip_prefix("host: "))
        .map(|s| s.trim().to_string())
}

pub fn is_msvc() -> bool {
    // 优先看已设置的 CARGO_BUILD_TARGET；否则问 rustc。
    if let Ok(t) = env::var("CARGO_BUILD_TARGET") {
        return t.contains("msvc");
    }
    match host_triple(&[]) {
        Some(t) => t.contains("msvc"),
        // 探测失败时，Windows 上按 msvc 处理（默认工具链就是它）
        None => cfg!(windows),
    }
}

pub fn rustc_bin() -> OsString {
    env::var_os("RUSTC").unwrap_or_else(|| OsString::from("rustc"))
}

pub fn cargo_bin() -> OsString {
    env::var_os("CARGO").unwrap_or_else(|| OsString::from("cargo"))
}

/// `rustup toolchain list` 里是否存在 nightly。
pub fn has_nightly() -> bool {
    let out = Command::new("rustup")
        .args(["toolchain", "list"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output();
    match out {
        Ok(o) => String::from_utf8_lossy(&o.stdout).contains("nightly"),
        Err(_) => false,
    }
}

// ---------------------------------------------------------------- 路径

/// 从 `start` 向上找含 `Cargo.toml` 的目录。
pub fn workspace_root(start: &Path) -> Option<PathBuf> {
    let mut cur = Some(start.to_path_buf());
    while let Some(dir) = cur {
        if dir.join("Cargo.toml").is_file() {
            return Some(dir);
        }
        cur = dir.parent().map(|p| p.to_path_buf());
    }
    None
}

/// `cargo metadata` 里的 `workspace_root`。
///
/// 为什么要它：**cargo 调用 rustc 时把 cwd 设成 workspace 根**，
/// 而 rustc 的 `-Zdump-mono-stats=<dir>` 是相对**它自己的 cwd** 写的。
/// 单 crate 项目里"包根 == workspace 根"，看不出差别；
/// 一到 workspace（真实项目的常态）报告就落到 workspace 根，
/// 而工具若只在成员目录里找，就会稳定地报"没读到报告"。
pub fn cargo_workspace_root(start: &Path) -> Option<PathBuf> {
    let out = Command::new(cargo_bin())
        .args(["metadata", "--no-deps", "--format-version", "1"])
        .current_dir(start)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout);
    let key = "\"workspace_root\":\"";
    let i = s.find(key)? + key.len();
    let rest = &s[i..];
    let b = rest.as_bytes();
    let mut j = 0usize;
    let mut end = None;
    while j < b.len() {
        match b[j] {
            b'\\' => j += 2,
            b'"' => {
                end = Some(j);
                break;
            }
            _ => j += 1,
        }
    }
    Some(PathBuf::from(json_unescape(&rest[..end?])))
}

/// `-Zdump-mono-stats=<dir>` 的报告**可能落在哪**。
///
/// 实测（cargo 1.100 / workspace 成员 `lilyco-binfmt`）：报告落在
/// **workspace 根**的 `human/`，不是成员目录 —— 所以两边都得看。
/// 顺序：cargo 的 workspace 根优先，再是 `root`（单 crate 时就重合）。
pub fn mono_stats_dirs(root: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = Vec::new();
    if let Some(ws) = cargo_workspace_root(root) {
        v.push(ws);
    }
    v.push(root.to_path_buf());
    v.dedup();
    v
}

/// 依赖图里所有 crate 名 vs. 其中属于本 workspace 成员的那些。
///
/// 用途：把单态化热点分成「**你能改的**」（本地源码）和「你改不到的」（依赖库）。
/// 这条区分至关重要 —— 实测一个真实 crate 排名前 12 的热点**全在 std/alloc/core**，
/// 而本地代码的份数全是 1；如果不区分，工具会报出"预期能少掉 44%"这种
/// 完全做不到的结论。
///
/// 判据（mono-stats 的名字风格）：
///   * 本地 item 的名字是从 crate 根起的**模块路径**，不带 crate 名 ——
///     所以首段既不是 std/core/alloc、也不在任何包名集合里 → 本地；
///   * 首段命中依赖包名 → 那是依赖；
///   * 首段命中 workspace 成员名 → 也是本地（dynify 会扫所有成员）。
///
/// 同样不引 JSON 依赖：`packages[]` 里的包名有独特形状 `"name":"X","version":"`，
/// 用它来抓；workspace 成员另有 `"workspace_members":[…]` 列表。
pub fn crate_name_sets(root: &Path) -> (Vec<String>, Vec<String>) {
    let empty = (Vec::new(), Vec::new());
    let Ok(out) = Command::new(cargo_bin())
        .args(["metadata", "--format-version", "1"])
        .current_dir(root)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
    else {
        return empty;
    };
    if !out.status.success() {
        return empty;
    }
    let s = String::from_utf8_lossy(&out.stdout);

    // 所有包名：找 `"name":"…","version":"` 这个组合，避免误抓 dependencies 里的 name
    let mut all: Vec<String> = Vec::new();
    let key = "\"name\":\"";
    let mut from = 0usize;
    while let Some(rel) = s[from..].find(key) {
        let i = from + rel + key.len();
        let rest = &s[i..];
        let b = rest.as_bytes();
        let mut j = 0usize;
        let mut end = None;
        while j < b.len() {
            match b[j] {
                b'\\' => j += 2,
                b'"' => {
                    end = Some(j);
                    break;
                }
                _ => j += 1,
            }
        }
        let Some(e) = end else { break };
        let name = json_unescape(&rest[..e]);
        // 紧跟 `,"version":"` 才算 package 条目
        let tail = &rest[e..];
        if tail.starts_with("\",\"version\":\"") {
            all.push(name.replace('-', "_"));
        }
        from = i + e + 1;
    }
    all.sort();
    all.dedup();

    // workspace 成员：`"workspace_members":["path+…#name@ver", …]`
    let mut members: Vec<String> = Vec::new();
    if let Some(k) = s.find("\"workspace_members\":[") {
        let start = k + "\"workspace_members\":[".len();
        let mut depth = 0i32;
        let mut end = start;
        for (off, c) in s[start..].char_indices() {
            match c {
                '[' => depth += 1,
                ']' => {
                    if depth == 0 {
                        end = start + off;
                        break;
                    }
                    depth -= 1;
                }
                _ => {}
            }
        }
        let arr = &s[start..end.max(start)];
        for part in arr.split(',') {
            // 形如 "path+file:///…/lilyco-core#lilyco-core@0.1.0"
            if let Some(hash) = part.find('#') {
                let after = &part[hash + 1..];
                let name = after.split('@').next().unwrap_or("").trim_matches('"').trim();
                if !name.is_empty() {
                    members.push(name.replace('-', "_"));
                }
            }
        }
    }
    members.sort();
    members.dedup();
    (all, members)
}

/// 解析 target 目录。
/// 顺序：CARGO_TARGET_DIR → `cargo metadata` 的 target_directory → <root>/target
pub fn target_dir(root: &Path) -> PathBuf {
    if let Some(v) = env::var_os("CARGO_TARGET_DIR") {
        let p = PathBuf::from(v);
        return if p.is_absolute() { p } else { root.join(p) };
    }
    if let Some(t) = metadata_target_dir(root) {
        return t;
    }
    root.join("target")
}

/// 只抓 `cargo metadata` 输出里的一个字段，避免引入 JSON 依赖。
fn metadata_target_dir(root: &Path) -> Option<PathBuf> {
    let out = Command::new(cargo_bin())
        .args(["metadata", "--no-deps", "--format-version", "1"])
        .current_dir(root)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout);
    let key = "\"target_directory\":\"";
    let i = s.find(key)? + key.len();
    let rest = &s[i..];
    // 找到结束引号（处理 \\ 转义）
    let mut end = None;
    let b = rest.as_bytes();
    let mut j = 0;
    while j < b.len() {
        match b[j] {
            b'\\' => j += 2,
            b'"' => {
                end = Some(j);
                break;
            }
            _ => j += 1,
        }
    }
    let raw = &rest[..end?];
    Some(PathBuf::from(json_unescape(raw)))
}

/// workspace 里所有成员的**包根目录**（含 `Cargo.toml` 的那一层）。
///
/// 为什么需要它：`workspace_root()` 是「从 cwd 往上找最近的 `Cargo.toml`」，
/// 所以站在 workspace 根上时拿到的是根，而**根下通常没有 `src/`** ——
/// 真实项目几乎都是 workspace，于是"默认扫 `root/src`"会稳定地扫到 0 个，
/// 还长得很像一个结论。这里用 `cargo metadata` 把成员列全，
/// 让默认行为是"扫这个 workspace 里所有成员"。
///
/// 同样不引 JSON 依赖：`--no-deps` 之后输出里属于"成员"的
/// `"manifest_path":"…"` 会出现在 `packages` 数组里，全抓出来即可。
pub fn workspace_manifest_dirs(root: &Path) -> Vec<PathBuf> {
    let Ok(out) = Command::new(cargo_bin())
        .args(["metadata", "--no-deps", "--format-version", "1"])
        .current_dir(root)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
    else {
        return Vec::new();
    };
    if !out.status.success() {
        return Vec::new();
    }
    let s = String::from_utf8_lossy(&out.stdout);
    let key = "\"manifest_path\":\"";
    let mut dirs: Vec<PathBuf> = Vec::new();
    let mut from = 0usize;
    while let Some(rel) = s[from..].find(key) {
        let i = from + rel + key.len();
        let rest = &s[i..];
        // 扫描到结束引号，注意别把 \" 当结尾
        let b = rest.as_bytes();
        let mut j = 0usize;
        let mut end = None;
        while j < b.len() {
            match b[j] {
                b'\\' => j += 2,
                b'"' => {
                    end = Some(j);
                    break;
                }
                _ => j += 1,
            }
        }
        let Some(e) = end else { break };
        let manifest = PathBuf::from(json_unescape(&rest[..e]));
        if let Some(dir) = manifest.parent() {
            dirs.push(dir.to_path_buf());
        }
        from = i + e + 1;
    }
    dirs.sort();
    dirs.dedup();
    dirs
}

/// `dynify` / `undo` 的**扫描范围**。
///
/// * 给了路径 → 就按给的来（相对路径按 `root` 解析）；
/// * 没给路径且 `<root>/src` 存在 → 就扫它（单 crate 项目，行为不变）；
/// * 没给路径且 `<root>/src` 不存在 → 说明 `root` 是 workspace 根，
///   扫**所有成员**的 `src/`（真实项目的常态）。
///
/// 若三者都落空，返回空表 —— 调用方必须把"扫到 0 个"和"没东西可扫"区分开，
/// 否则 0 会被误读成"这个项目没有泛型热点"。
pub fn scan_roots(paths: &[String], root: &Path) -> Vec<PathBuf> {
    if !paths.is_empty() {
        let mut out = Vec::new();
        for p in paths {
            let pb = PathBuf::from(p);
            // ⚠️ Git Bash 的 `/d/Code/...` 在 Windows 上 `is_absolute() == false`，
            // 会被当成相对路径拼到 root 后面而静默落空。这里顺手把这种形式认出来。
            let pb = if pb.is_absolute() {
                pb
            } else if let Some(fixed) = msys_path(&pb) {
                fixed
            } else {
                root.join(pb)
            };
            // 不管存不存在都保留：不存在的路径交给调用方明确报错。
            // 静默丢掉的话，打错一个字得到的是"扫到 0 个泛型函数"—— 看着像结论。
            out.push(pb);
        }
        return out;
    }
    let src = root.join("src");
    if src.is_dir() {
        return vec![src];
    }
    // workspace 根：把每个成员的 src/ 都算上
    let mut out = Vec::new();
    for dir in workspace_manifest_dirs(root) {
        let s = dir.join("src");
        if s.is_dir() {
            out.push(s);
        }
    }
    out.sort();
    out.dedup();
    out
}

/// 把 Git Bash(MSYS) 风格的 `/d/Code/...` 或 `/c/Users/...` 还原成 `D:\Code\...`。
/// 只认「`/` + 单个字母 + `/`」这一种形态，其它一律返回 None 交回调用方。
fn msys_path(p: &Path) -> Option<PathBuf> {
    let s = p.to_str()?;
    let b = s.as_bytes();
    if b.len() >= 3 && b[0] == b'/' && b[1].is_ascii_alphabetic() && b[2] == b'/' {
        let drive = (b[1] as char).to_ascii_uppercase();
        // 去掉开头的 "/x"，剩下的 "/Code/..." 直接接在盘符后
        return Some(PathBuf::from(format!("{drive}:{}", &s[2..])));
    }
    None
}

fn json_unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut it = s.chars();
    while let Some(c) = it.next() {
        if c == '\\' {
            match it.next() {
                Some('\\') => out.push('\\'),
                Some('/') => out.push('/'),
                Some('"') => out.push('"'),
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
                Some(other) => out.push(other),
                None => break,
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// 相对当前目录显示路径，短一点。
pub fn pretty_path(p: &Path) -> String {
    let cwd = env::current_dir().unwrap_or_default();
    match p.strip_prefix(&cwd) {
        Ok(rel) if rel.as_os_str().is_empty() => ".".to_string(),
        Ok(rel) => rel.display().to_string(),
        Err(_) => p.display().to_string(),
    }
}

// ---------------------------------------------------------------- 杂项

/// 按「可见宽度」补空格 —— 因为 ANSI 转义序列不占屏幕列数，
/// 直接用 `{:<8}` 对齐会歪。
pub fn pad_visible(s: &str, w: usize) -> String {
    let mut visible = 0usize;
    let mut in_esc = false;
    for c in s.chars() {
        if in_esc {
            if c == 'm' {
                in_esc = false;
            }
            continue;
        }
        if c == '\x1b' {
            in_esc = true;
            continue;
        }
        visible += 1;
    }
    let mut out = s.to_string();
    for _ in visible..w {
        out.push(' ');
    }
    out
}

/// 目录递归体积（字节）。用于报告"顺手清掉了多少"。
pub fn dir_size(p: &Path) -> u64 {
    let mut total = 0u64;
    let mut stack = vec![p.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else {
            continue;
        };
        for e in rd.flatten() {
            let Ok(ft) = e.file_type() else { continue };
            if ft.is_dir() {
                stack.push(e.path());
            } else if let Ok(m) = e.metadata() {
                total = total.saturating_add(m.len());
            }
        }
    }
    total
}

/// 把字节数变成人看得懂的写法。
pub fn bytes_human(n: u64) -> String {
    const KB: f64 = 1024.0;
    let f = n as f64;
    if f < KB {
        format!("{n} B")
    } else if f < KB * KB {
        format!("{:.1} KB", f / KB)
    } else if f < KB * KB * KB {
        format!("{:.1} MB", f / (KB * KB))
    } else {
        format!("{:.2} GB", f / (KB * KB * KB))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 🔴 真实踩过的坑：在 Git Bash 里敲 `cargo xmake dynify /d/Code/...`，
    /// Windows 上 `Path::is_absolute()` 对 `/d/Code/...` 返回 **false** ——
    /// 于是它被当成相对路径拼到 root 后面，静默落空、扫到 0 个文件，
    /// 还长得很像一个结论（"这项目没有泛型热点"）。
    #[test]
    fn msys_style_paths_are_recognized() {
        assert_eq!(
            msys_path(Path::new("/d/Code/x/src")),
            Some(PathBuf::from("D:/Code/x/src"))
        );
        assert_eq!(
            msys_path(Path::new("/c/Users/liuqi")),
            Some(PathBuf::from("C:/Users/liuqi"))
        );
        // 不该动的：普通相对路径、真正的 Windows 绝对路径、越界的形态
        assert_eq!(msys_path(Path::new("src")), None);
        assert_eq!(msys_path(Path::new("D:/Code/x")), None);
        assert_eq!(msys_path(Path::new("/d")), None);
        assert_eq!(msys_path(Path::new("/dc/x")), None, "两字母驱动器名不算");
        assert_eq!(msys_path(Path::new("/2/code")), None, "驱动器必须是字母");
    }

    /// `scan_roots` 的三条分支：给了路径 / 单 crate / workspace
    #[test]
    fn scan_roots_prefers_src_then_falls_back_to_members() {
        let tmp = std::env::temp_dir().join(format!("xmk-scan-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        // 单 crate：root/src 存在 → 只扫它
        std::fs::create_dir_all(tmp.join("src")).unwrap();
        let r = scan_roots(&[], &tmp);
        assert_eq!(r, vec![tmp.join("src")]);

        // 显式路径优先（相对路径按 root 解析）
        let r = scan_roots(&["other".into()], &tmp);
        assert_eq!(r, vec![tmp.join("other")], "给了路径就按给的来，不去猜");

        // 显式路径 + Git Bash 形态 → 转成 Windows 盘符路径，而不是拼在 root 后面
        let r = scan_roots(&["/d/Code/zzz".into()], &tmp);
        assert_eq!(r, vec![PathBuf::from("D:/Code/zzz")]);

        let _ = std::fs::remove_dir_all(&tmp);
    }
}
