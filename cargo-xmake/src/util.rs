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
