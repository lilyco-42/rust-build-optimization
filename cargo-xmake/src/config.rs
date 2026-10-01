//! 把「计划」安全地合进 `.cargo/config.toml`。
//!
//! 设计约束（都是被实测逼出来的）：
//!   1. **只碰 `.cargo/config.toml`，绝不碰 `Cargo.toml`。**
//!      `.cargo/config.toml` 里的 `[profile.*]` 实测生效，而且优先级高于
//!      `Cargo.toml`；它不是 manifest，所以不会有 manifest feature-gate
//!      （cranelift 就是这么在 stable 上把整个构建搞崩的）。
//!   2. **不覆盖用户已有的 key**，除非 `--force`。
//!   3. **每一行我们加的都以 `# cargo-xmake` 结尾**，所以 `undo` 能精确还原。
//!   4. `rustflags` 是**追加**不是替换 —— 用户可能已经在里面放了别的 flag。

use crate::plan::{Setting, Val};
use std::fs;
use std::path::{Path, PathBuf};

pub const MARKER: &str = "# cargo-xmake";

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Action {
    Added,
    Replaced,
    Skipped,   // 用户已有，我们让位
    Unchanged, // 已经是我们要的值
    Removed,
}

impl Action {
    pub fn tag(&self) -> &'static str {
        match self {
            Action::Added => "add",
            Action::Replaced => "set",
            Action::Skipped => "skip",
            Action::Unchanged => "ok",
            Action::Removed => "del",
        }
    }
}

pub struct Edit {
    pub where_: String,
    pub what: String,
    pub action: Action,
}

pub struct Report {
    pub path: PathBuf,
    pub edits: Vec<Edit>,
    pub changed: bool,
    /// `undo` 把文件删掉了（原内容**全部**是我们写的，只剩空行）。
    /// 留一个只有空行的 `.cargo/config.toml` 是垃圾，也让人以为还有配置在生效。
    pub removed_file: bool,
}

impl Report {
    pub fn counts(&self) -> (usize, usize, usize) {
        let mut add = 0;
        let mut skip = 0;
        let mut other = 0;
        for e in &self.edits {
            match e.action {
                Action::Added | Action::Removed => add += 1,
                Action::Skipped => skip += 1,
                _ => other += 1,
            }
        }
        (add, skip, other)
    }
}

pub fn config_path(root: &Path) -> PathBuf {
    // cargo 同时认 config.toml 和 config；优先 toml（新的官方名字）
    let a = root.join(".cargo").join("config.toml");
    let b = root.join(".cargo").join("config");
    if a.is_file() || !b.is_file() {
        a
    } else {
        b
    }
}

/// 所有可能的用户级 config。
///
/// 不猜，全列出来。原因：本机实测 `CARGO_HOME=D:\...\rustup\.cargo`，
/// 但 cargo 实际吃的是 `C:\Users\liuqi\.cargo\config.toml` 里的
/// `[profile.release] opt-level = "z"`（用 `cargo build --release -v`
/// 看到 `-C opt-level=z` 才发现的）。与其赌哪个对，不如两个都报出去。
pub fn global_config_candidates() -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = Vec::new();
    let mut push = |p: PathBuf| {
        if p.is_file() && !v.iter().any(|x| x == &p) {
            v.push(p);
        }
    };
    if let Some(h) = std::env::var_os("CARGO_HOME") {
        let d = PathBuf::from(h);
        push(d.join("config.toml"));
        push(d.join("config"));
    }
    if let Some(home) = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")) {
        let d = PathBuf::from(home).join(".cargo");
        push(d.join("config.toml"));
        push(d.join("config"));
    }
    v
}

/// 兼容旧调用：返回第一个候选。
pub fn global_config_path() -> Option<PathBuf> {
    global_config_candidates().into_iter().next()
}

pub struct Source {
    pub label: String,
    pub path: PathBuf,
    /// `"profile.release" -> [("opt-level", "\"z\"")]`
    pub profiles: Vec<(String, Vec<(String, String)>)>,
}

impl Source {
    pub fn get(&self, section: &str, key: &str) -> Option<&str> {
        self.profiles
            .iter()
            .find(|(n, _)| n == section)
            .and_then(|(_, kvs)| kvs.iter().find(|(k, _)| k == key))
            .map(|(_, v)| v.as_str())
    }
}

/// 判断两个 TOML 字面量是不是等价（`true` vs `"fat"` 这种跨写法的不判等，够用）
pub fn toml_eq(key: &str, a: &str, b: &str) -> bool {
    let n = |s: &str| s.trim().trim_matches('"').trim_matches('\'').to_lowercase();
    let (x, y) = (n(a), n(b));
    if x == y {
        return true;
    }
    // cargo 里这些写法语义相同，不认就会刷假告警
    match key {
        "lto" => matches!((x.as_str(), y.as_str()), ("true", "fat") | ("fat", "true")),
        "strip" => matches!(
            (x.as_str(), y.as_str()),
            ("true", "symbols")
                | ("symbols", "true")
                | ("true", "debuginfo")
                | ("debuginfo", "true")
        ),
        _ => false,
    }
}

/// 读出某个 config 文件里所有 `[profile.*]` 的 key（行级解析，够用）。
pub fn read_source(label: String, path: &Path) -> Option<Source> {
    let text = fs::read_to_string(path).ok()?;
    let lines: Vec<String> = text.lines().map(|l| l.to_string()).collect();
    let mut profiles: Vec<(String, Vec<(String, String)>)> = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        if let Some(name) = section_name(&lines[i]) {
            if name.starts_with("profile.") {
                let (start, end) = section_range(&lines, i);
                let mut kvs = Vec::new();
                for l in &lines[start + 1..end] {
                    let t = l.split('#').next().unwrap_or("").trim();
                    if t.is_empty() {
                        continue;
                    }
                    if let Some((k, v)) = t.split_once('=') {
                        kvs.push((k.trim().to_string(), v.trim().to_string()));
                    }
                }
                profiles.push((name, kvs));
            }
        }
        i += 1;
    }
    if profiles.is_empty() {
        return None;
    }
    Some(Source {
        label,
        path: path.to_path_buf(),
        profiles,
    })
}

/// 项目 + 用户级所有配置里的剖面设置（已经是"我们还没写入"的状态）。
pub fn existing_sources(root: &Path) -> Vec<Source> {
    let mut v = Vec::new();
    let proj = config_path(root);
    if proj.is_file() {
        if let Some(s) = read_source("项目".into(), &proj) {
            v.push(s);
        }
    }
    let cargo_home = std::env::var_os("CARGO_HOME").map(PathBuf::from);
    for g in global_config_candidates() {
        let label = match &cargo_home {
            Some(ch) if g.starts_with(ch) => "全局($CARGO_HOME)".to_string(),
            _ => "全局(~/.cargo)".to_string(),
        };
        if let Some(s) = read_source(label, &g) {
            v.push(s);
        }
    }
    v
}

/// 是否已经 setup 过（看有没有我们的标记 + dev 剖面）。
pub fn is_setup(root: &Path) -> bool {
    let p = config_path(root);
    match fs::read_to_string(&p) {
        Ok(s) => s.contains(MARKER) && s.contains("[profile.dev]"),
        Err(_) => false,
    }
}

// ---------------------------------------------------------------- 行处理

fn section_name(line: &str) -> Option<String> {
    let t = line.trim();
    if t.starts_with("[[") {
        return None; // array of tables，我们不碰
    }
    if !t.starts_with('[') {
        return None;
    }
    let end = t.find(']')?;
    Some(t[1..end].trim().to_string())
}

fn find_section(lines: &[String], name: &str) -> Option<usize> {
    lines
        .iter()
        .position(|l| section_name(l).as_deref() == Some(name))
}

/// section 的 [start, end) —— end 是下一个 section header 或文件尾
fn section_range(lines: &[String], start: usize) -> (usize, usize) {
    let mut end = lines.len();
    for i in start + 1..lines.len() {
        if section_name(&lines[i]).is_some() {
            end = i;
            break;
        }
    }
    (start, end)
}

fn key_line(text: &str, key: &str) -> bool {
    let t = text.trim_start();
    if t.starts_with('#') {
        return false;
    }
    match t.split_once('=') {
        Some((k, _)) => k.trim() == key,
        None => false,
    }
}

fn render(s: &Setting) -> String {
    format!("{} = {} {MARKER}", s.key, s.val)
}

fn same_value(text: &str, val: &Val) -> bool {
    let Some((_, rhs)) = text.split_once('=') else {
        return false;
    };
    // 去掉行尾注释和空白
    let rhs = rhs.split('#').next().unwrap_or("").trim();
    match val {
        Val::Str(s) => rhs == format!("\"{s}\"") || rhs == format!("'{s}'"),
        Val::Int(n) => rhs.parse::<i64>().ok() == Some(*n),
        Val::Bool(b) => rhs == b.to_string(),
    }
}

fn report_value_line(line: &str) -> String {
    line.split('#').next().unwrap_or(line).trim().to_string()
}

/// 保证某个 section 存在，返回它的 header 行号。
fn ensure_section(lines: &mut Vec<String>, name: &str) -> usize {
    if let Some(i) = find_section(lines, name) {
        return i;
    }
    if !lines.is_empty() && !lines.last().map(|l| l.trim().is_empty()).unwrap_or(true) {
        lines.push(String::new());
    }
    lines.push(format!("[{name}] {MARKER}"));
    lines.len() - 1
}

fn set_key(
    lines: &mut Vec<String>,
    section: &str,
    s: &Setting,
    force: bool,
    edits: &mut Vec<Edit>,
) -> bool {
    let hdr = ensure_section(lines, section);
    let (start, end) = section_range(lines, hdr);
    // 找同 section 内已有的 key
    let found = (start + 1..end).find(|&i| key_line(&lines[i], s.key));
    let want = render(s);
    let short = format!("{}/{}", section.replace("profile.", ""), s.key);

    match found {
        Some(i) => {
            if same_value(&lines[i], &s.val) {
                // 值一样：把标记补上，方便 undo，但不计入变更
                if !lines[i].contains(MARKER) {
                    lines[i] = format!("{} {MARKER}", lines[i].trim_end());
                }
                edits.push(Edit {
                    where_: short,
                    what: report_value_line(&lines[i]),
                    action: Action::Unchanged,
                });
                false
            } else if force {
                let old = report_value_line(&lines[i]);
                lines[i] = want;
                edits.push(Edit {
                    where_: short,
                    what: format!("{old}  ->  {}", s.val),
                    action: Action::Replaced,
                });
                true
            } else {
                edits.push(Edit {
                    where_: short,
                    what: format!("保留用户值 {}", report_value_line(&lines[i])),
                    action: Action::Skipped,
                });
                false
            }
        }
        None => {
            // 插到 section 头部（紧跟 header 之后），保持同类设置聚在一起
            lines.insert(start + 1, want);
            edits.push(Edit {
                where_: short,
                what: s.val.to_string(),
                action: Action::Added,
            });
            true
        }
    }
}

// ---------------------------------------------------------------- rustflags

fn ensure_rustflags(
    lines: &mut Vec<String>,
    section: &str,
    flags: &[String],
    edits: &mut Vec<Edit>,
) -> bool {
    if flags.is_empty() {
        return false;
    }
    let hdr = ensure_section(lines, section);
    let (start, end) = section_range(lines, hdr);
    let short = format!("{section}/rustflags");
    let one = flags.join(" ");

    // 追加语义：这一节里已经出现我们全部片段就算 ok
    let joined = lines[start..end].join("\n");
    if flags.iter().all(|f| joined.contains(f.as_str())) {
        edits.push(Edit {
            where_: short,
            what: one,
            action: Action::Unchanged,
        });
        return false;
    }

    let key_at = (start + 1..end).find(|&i| key_line(&lines[i], "rustflags"));
    match key_at {
        None => {
            let arr: Vec<String> = flags.iter().map(|f| format!("\"{f}\"")).collect();
            lines.insert(
                start + 1,
                format!("rustflags = [{}] {MARKER}", arr.join(", ")),
            );
            edits.push(Edit {
                where_: short,
                what: format!("新增 [{}]", arr.join(", ")),
                action: Action::Added,
            });
            true
        }
        Some(i) => {
            // 找这个数组的结束 ']'（支持多行数组）
            let close = (i..end).find(|&j| lines[j].contains(']'));
            let Some(j) = close else {
                edits.push(Edit {
                    where_: short,
                    what: "rustflags 不是内联数组，跳过（请手动追加）".into(),
                    action: Action::Skipped,
                });
                return false;
            };
            let joined = lines[i..=j].join("\n");
            let missing: Vec<String> = flags
                .iter()
                .filter(|f| !joined.contains(f.as_str()))
                .cloned()
                .collect();
            if missing.is_empty() {
                return false;
            }
            // 在当前行最后一个 ']' 之前插入 ", "flag"
            let quote: Vec<String> = missing.iter().map(|f| format!("\"{f}\"")).collect();
            let raw = lines[j].clone();
            let cut = raw.rfind(']').unwrap_or(raw.len());
            let mut head = raw[..cut].trim_end().to_string();
            let sep = if head.ends_with('[') { "" } else { ", " };
            head.push_str(sep);
            head.push_str(&quote.join(", "));
            head.push(']');
            let tail_comment = raw[cut + 1..].trim();
            if tail_comment.is_empty() {
                head.push(' ');
                head.push_str(MARKER);
            } else if tail_comment.contains(MARKER) {
                head.push(' ');
                head.push_str(tail_comment);
            } else {
                head.push(' ');
                head.push_str(tail_comment);
                head.push(' ');
                head.push_str(MARKER);
            }
            lines[j] = head;
            edits.push(Edit {
                where_: short,
                what: format!("追加 [{}]", quote.join(", ")),
                action: Action::Added,
            });
            true
        }
    }
}

// ---------------------------------------------------------------- 对外 API

pub struct Plan {
    pub profiles: Vec<(String, Vec<Setting>)>,
    pub rustflags_section: Option<String>,
    pub rustflags: Vec<String>,
}

pub fn apply(root: &Path, plan: &Plan, force: bool) -> Result<Report, String> {
    let path = config_path(root);
    let original = fs::read_to_string(&path).unwrap_or_default();
    let mut lines: Vec<String> = if original.is_empty() {
        Vec::new()
    } else {
        original.lines().map(|l| l.to_string()).collect()
    };
    // 保住结尾换行
    let trailing_nl = original.is_empty() || original.ends_with('\n');

    let mut edits = Vec::new();
    let mut changed = false;

    for (section, settings) in &plan.profiles {
        for s in settings.iter() {
            changed |= set_key(&mut lines, section, s, force, &mut edits);
        }
    }
    if let Some(sec) = &plan.rustflags_section {
        changed |= ensure_rustflags(&mut lines, sec, &plan.rustflags, &mut edits);
    }

    if changed {
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir).map_err(|e| format!("创建 {} 失败: {e}", dir.display()))?;
        }
        let mut text = lines.join("\n");
        if trailing_nl {
            text.push('\n');
        }
        fs::write(&path, text).map_err(|e| format!("写入 {} 失败: {e}", path.display()))?;
    }

    Ok(Report {
        path,
        edits,
        changed,
        removed_file: false,
    })
}

pub fn undo(root: &Path) -> Result<Report, String> {
    let path = config_path(root);
    // 文件不在 = 已经没东西可撤了 → 幂等空操作。
    // 上一次 `undo` 很可能刚把整份（全是我们写的）config 删掉，
    // 这时候再报 `os error 3` 只会吓人，所以这里显式当成"无事发生"。
    let original = match fs::read_to_string(&path) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok(Report {
                path,
                edits: Vec::new(),
                changed: false,
                removed_file: false,
            });
        }
        Err(e) => return Err(format!("读 {} 失败: {e}", path.display())),
    };
    let trailing_nl = original.ends_with('\n');
    let mut lines: Vec<String> = original.lines().map(|l| l.to_string()).collect();
    let before = lines.len();
    let mut edits = Vec::new();

    let mut kept: Vec<String> = Vec::with_capacity(lines.len());
    for l in lines.drain(..) {
        if l.contains(MARKER) {
            let shown = l.split('#').next().unwrap_or(&l).trim().to_string();
            if !shown.is_empty() {
                edits.push(Edit {
                    where_: "config".into(),
                    what: shown,
                    action: Action::Removed,
                });
            }
        } else {
            kept.push(l);
        }
    }
    let mut lines = kept;
    // 收尾：多余的空行合掉
    while lines.last().map(|l| l.trim().is_empty()).unwrap_or(false) {
        lines.pop();
    }
    let changed = lines.len() != before;
    // 全是我们写的 → 连文件一起删掉，别留一个只有空行的尸体
    let blank = lines.iter().all(|l| l.trim().is_empty());

    if changed {
        if blank {
            fs::remove_file(&path).map_err(|e| format!("删除 {} 失败: {e}", path.display()))?;
            if let Some(dir) = path.parent() {
                let _ = fs::remove_dir(dir); // 目录空了才会成功
            }
            return Ok(Report {
                path,
                edits,
                changed,
                removed_file: true,
            });
        }
        let mut text = lines.join("\n");
        if trailing_nl || !text.is_empty() {
            text.push('\n');
        }
        fs::write(&path, text).map_err(|e| format!("写入 {} 失败: {e}", path.display()))?;
    }

    Ok(Report {
        path,
        edits,
        changed,
        removed_file: false,
    })
}
