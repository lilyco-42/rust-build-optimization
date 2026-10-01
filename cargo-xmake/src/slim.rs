//! `slim` —— **砍编译量**，这是让 cargo 内环真正快过 xmake 的唯一大杠杆。
//!
//! ## 为什么这个功能排第一
//!
//! 实测（lilyco，24 成员 / 865 包）：根目录裸 `cargo build` 会编**全部 24 个成员**，
//! 显式 `default-members`（19 个）之后只要 **379 包** —— **一行省 56%**。
//! 相比之下 `dynify` 在真实项目上的收益是 **0.04%**（21623 份单态化里本地只有 8 份），
//! 因为它只能改**你的源码**，而热度大头在 std / 依赖里。
//!
//! **想「打赢 xmake」，靠的就是这里 —— 减编译量，不是减单态化。**
//!
//! ## 它做什么
//!
//! 1. 跑一次 `cargo metadata`（带依赖图），解析 `resolve.nodes`；
//! 2. 对每个 workspace 成员算**依赖闭包**，再算它的**独占包数**
//!    （= 只有它需要、去掉它就不再编译的那些包）；
//! 3. 按独占数排出「谁最贵」，算出去掉它们之后的确切包数；
//! 4. `--apply` 时把 `default-members` 写进工作区根 `Cargo.toml`（带标记，`undo` 可逆）。
//!
//! ## 安全约定
//!
//! - **绝不用它替换用户自己写的 `default-members`**（无标记的遇到就报错退出）；
//! - 写入的那一行带 `# cargo-xmake-slim` 标记，`undo` 靠标记删除，纯增删可逆；
//! - `--drop` 里给了不存在的成员名会**明确报错**，不会静默忽略（历史上吃过「静默 0」的亏）。

use crate::cli::Opts;
use crate::util;
use crate::util::Style;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::Path;

pub const MARKER: &str = "# cargo-xmake-slim";

// ------------------------------------------------------------ JSON 定向扫描
//
// 零依赖，沿用 `util::workspace_manifest_dirs` 的做法：**不全量解析 JSON**，
// 只把要用的字段揪出来。好处是代码短、没有依赖树，
// 代价是"遇到不合预期的排版要能容错" —— 所以下面每个 helper 都优先用
// 相对宽松的搜索（跳过空白、兼容 `"k":v` 与 `"k" : v` 两种写法）。

/// 在 `[lo, hi)` 范围内取 `"<key>": "..."` 的字符串值（两侧带引号，避免匹配进 `"realname"`）。
/// 兼容紧凑与美化两种 JSON 排版：先找 `"key"`，跳过空白找 `:`，再跳过空白找 `"`。
fn str_field(s: &str, key: &str, span: (usize, usize)) -> Option<String> {
    let body = s.get(span.0..span.1)?;
    let pat = format!("\"{key}\"");
    let rel = body.find(&pat)?;
    let after = rel + pat.len();
    let colon = body.get(after..)?.find(':')?;
    let rest = body.get(after + colon + 1..)?;
    let q = rest.find('"')?;
    let tail = rest.get(q + 1..)?;
    let b = tail.as_bytes();
    let mut j = 0usize;
    while j < b.len() {
        match b[j] {
            b'\\' => j += 2,
            b'"' => return Some(util::json_unescape(tail.get(..j).unwrap_or(""))),
            _ => j += 1,
        }
    }
    None
}

/// 找 `"<key>"` 后面紧跟的那个 `[` 的下标（相对传入的整串坐标）。
fn array_after_key(s: &str, key: &str) -> Option<usize> {
    let pat = format!("\"{key}\"");
    let rel = s.find(&pat)?;
    let after = rel + pat.len();
    let colon = s.get(after..)?.find(':')?;
    let rest = s.get(after + colon..)?;
    let open = rest.find('[')?;
    Some(after + colon + open)
}

/// 取一对方括号里所有**顶层字符串**（处理转义）。`open` 是 `[` 的下标。
fn strings_in_array(s: &str, open: usize) -> Vec<String> {
    let b = s.as_bytes();
    let mut out = Vec::new();
    let mut i = open + 1;
    let mut depth = 0i32;
    let mut start: Option<usize> = None;
    while i < b.len() {
        match b[i] {
            b'\\' if start.is_some() => i += 2,
            b'"' => match start {
                None => {
                    start = Some(i + 1);
                    i += 1;
                }
                Some(st) => {
                    out.push(util::json_unescape(s.get(st..i).unwrap_or("")));
                    start = None;
                    i += 1;
                }
            },
            b'[' | b'{' => {
                if start.is_none() {
                    depth += 1;
                }
                i += 1;
            }
            b']' | b'}' => {
                if start.is_none() {
                    if depth == 0 {
                        break;
                    }
                    depth -= 1;
                }
                i += 1;
            }
            _ => i += 1,
        }
    }
    out
}

/// 取一对方括号里所有**顶层对象**的字节范围 `[start, end)`。
fn object_spans_in_array(s: &str, open: usize) -> Vec<(usize, usize)> {
    let b = s.as_bytes();
    let mut spans = Vec::new();
    let mut i = open + 1;
    let mut depth = 0i32;
    let mut in_str = false;
    let mut start: Option<usize> = None;
    while i < b.len() {
        match b[i] {
            b'\\' if in_str => i += 2,
            b'"' => {
                in_str = !in_str;
                i += 1;
            }
            b'{' if !in_str => {
                if depth == 0 {
                    start = Some(i);
                }
                depth += 1;
                i += 1;
            }
            b'}' if !in_str => {
                depth -= 1;
                if depth == 0 {
                    if let Some(st) = start {
                        spans.push((st, i + 1));
                        start = None;
                    }
                }
                i += 1;
            }
            b'[' if !in_str => {
                depth += 1;
                i += 1;
            }
            b']' if !in_str => {
                if depth == 0 {
                    break;
                }
                depth -= 1;
                i += 1;
            }
            _ => i += 1,
        }
    }
    spans
}

// ------------------------------------------------------------------ 依赖图

struct Graph {
    /// workspace 成员的 package id
    members: Vec<String>,
    /// package id -> package name
    names: HashMap<String, String>,
    /// package id -> 它依赖的 package id
    edges: HashMap<String, Vec<String>>,
}

fn parse_metadata(s: &str) -> Result<Graph, String> {
    let wm = array_after_key(s, "workspace_members")
        .ok_or_else(|| "cargo metadata 输出里找不到 workspace_members".to_string())?;
    let members = strings_in_array(s, wm);

    let pk = array_after_key(s, "packages")
        .ok_or_else(|| "cargo metadata 输出里找不到 packages".to_string())?;
    let mut names: HashMap<String, String> = HashMap::new();
    for (a, b) in object_spans_in_array(s, pk) {
        let id = str_field(s, "id", (a, b));
        let nm = str_field(s, "name", (a, b));
        if let (Some(i), Some(n)) = (id, nm) {
            names.insert(i, n);
        }
    }

    let nd = array_after_key(s, "nodes").ok_or_else(|| {
        "cargo metadata 输出里找不到 resolve.nodes —— slim 需要完整依赖图".to_string()
    })?;
    let mut edges: HashMap<String, Vec<String>> = HashMap::new();
    for (a, b) in object_spans_in_array(s, nd) {
        let Some(id) = str_field(s, "id", (a, b)) else {
            continue;
        };
        let sub = s.get(a..b).unwrap_or("");
        let mut deps = Vec::new();
        // cargo 1.60+ 用 `deps`，更老的版本只用 `dependencies`
        for key in ["deps", "dependencies"] {
            if let Some(d) = array_after_key(sub, key) {
                for (x, y) in object_spans_in_array(sub, d) {
                    if let Some(p) = str_field(sub, "pkg", (x, y)) {
                        deps.push(p);
                    }
                }
                if !deps.is_empty() {
                    break;
                }
            }
        }
        deps.sort();
        deps.dedup();
        edges.insert(id, deps);
    }

    Ok(Graph {
        members,
        names,
        edges,
    })
}

/// 从某个包出发、沿依赖边能到达的**所有**包（含它自己）。
fn closure_of(start: &str, edges: &HashMap<String, Vec<String>>) -> BTreeSet<String> {
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut stack: Vec<String> = vec![start.to_string()];
    while let Some(cur) = stack.pop() {
        if !seen.insert(cur.clone()) {
            continue;
        }
        if let Some(ds) = edges.get(&cur) {
            for d in ds {
                stack.push(d.clone());
            }
        }
    }
    seen
}

/// package id 读不出名字时的兜底：现代格式是 `path+file:///a/b#name@0.1.0`，
/// 老格式是 `name 0.1.0 (path+file:///...)` —— 两种都能拿出可读的名字。
fn short_id(id: &str) -> String {
    if let Some(p) = id.rfind('#') {
        let tail = id.get(p + 1..).unwrap_or(id);
        return match tail.rfind('@') {
            Some(q) => tail.get(..q).unwrap_or(tail).to_string(),
            None => tail.to_string(),
        };
    }
    match id.split(' ').next() {
        Some(f) => f.to_string(),
        None => id.to_string(),
    }
}

// ---------------------------------------------------------------- 分析结果

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub name: String,
    /// 该成员的整个依赖闭包有多少个包
    pub closure_len: usize,
    /// **独占包数** = 只有它需要、去掉它就不再编译的包数
    pub exclusive: usize,
}

pub struct Analysis {
    /// 按独占数降序（同数再按名字，保证输出稳定）
    pub entries: Vec<Entry>,
    closures: HashMap<String, BTreeSet<String>>,
    /// 全部成员一起构建时要编的包数
    pub total_all: usize,
}

impl Analysis {
    /// 丢掉 `drop` 这些成员之后，确切还要编多少个包。
    pub fn total_without(&self, drop: &HashSet<String>) -> usize {
        let mut u: BTreeSet<String> = BTreeSet::new();
        for e in &self.entries {
            if drop.contains(&e.name) {
                continue;
            }
            if let Some(c) = self.closures.get(&e.name) {
                for p in c {
                    u.insert(p.clone());
                }
            }
        }
        u.len()
    }

    pub fn names(&self) -> Vec<String> {
        self.entries.iter().map(|e| e.name.clone()).collect()
    }
}

fn analyze_graph(g: &Graph) -> Analysis {
    let mut raw: Vec<(String, BTreeSet<String>)> = Vec::new();
    for m in &g.members {
        let name = g
            .names
            .get(m)
            .cloned()
            .unwrap_or_else(|| short_id(m));
        let c = closure_of(m, &g.edges);
        raw.push((name, c));
    }

    let mut entries = Vec::new();
    let mut closures: HashMap<String, BTreeSet<String>> = HashMap::new();
    for (name, c) in &raw {
        let mut others: BTreeSet<String> = BTreeSet::new();
        for (n2, c2) in &raw {
            if n2 == name {
                continue;
            }
            for p in c2 {
                others.insert(p.clone());
            }
        }
        let exclusive = c.difference(&others).count();
        closures.insert(name.clone(), c.clone());
        entries.push(Entry {
            name: name.clone(),
            closure_len: c.len(),
            exclusive,
        });
    }
    let mut total_union: BTreeSet<String> = BTreeSet::new();
    for (_, c) in &raw {
        for p in c {
            total_union.insert(p.clone());
        }
    }

    entries.sort_by(|a, b| {
        b.exclusive
            .cmp(&a.exclusive)
            .then_with(|| a.name.cmp(&b.name))
    });
    Analysis {
        entries,
        closures,
        total_all: total_union.len(),
    }
}

/// 跑 `cargo metadata` 并分析。**这是唯一会碰到外界的函数。**
pub fn analyze(root: &Path) -> Result<Analysis, String> {
    let txt = util::metadata_json(root, false)?;
    let g = parse_metadata(&txt)?;
    if g.members.is_empty() {
        return Err("这看起来不是 cargo workspace（没有 workspace 成员），slim 只对 workspace 有意义".into());
    }
    Ok(analyze_graph(&g))
}

// ------------------------------------------------------------------- 写入

fn render_line(keep: &[String]) -> String {
    let items: Vec<String> = keep.iter().map(|n| format!("\"{n}\"")).collect();
    format!("default-members = [{}]  {}", items.join(", "), MARKER)
}

/// 把 `default-members` 写进工作区根 Cargo.toml。
///
/// 只做两件**可逆**的事：① 没有就新增一行（带标记）；② 有且带我们的标记就替换。
/// 遇到用户自己写的、没有标记的 `default-members` **一律报错**，绝不覆盖。
pub fn write_default_members(manifest: &Path, keep: &[String]) -> Result<(), String> {
    let txt = std::fs::read_to_string(manifest).map_err(|e| format!("读 Cargo.toml 失败: {e}"))?;
    let lines: Vec<&str> = txt.lines().collect();
    let ws_hdr = lines
        .iter()
        .position(|l| l.trim() == "[workspace]")
        .ok_or("工作区根 Cargo.toml 里没有 [workspace] 段")?;
    // 段落结束 = 下一个以 `[` 开头的行（可能是 [package] / [dependencies] …）
    let sec_end = lines
        .iter()
        .enumerate()
        .skip(ws_hdr + 1)
        .find(|(_, l)| l.trim_start().starts_with('['))
        .map(|(i, _)| i)
        .unwrap_or(lines.len());

    let existing = (ws_hdr + 1..sec_end)
        .find(|&i| lines[i].trim_start().starts_with("default-members"));

    let mut out: Vec<String> = lines.iter().map(|l| l.to_string()).collect();
    match existing {
        Some(i) if lines[i].contains(MARKER) => out[i] = render_line(keep),
        Some(_) => {
            return Err(
                "Cargo.toml 里已有不是本工具写入的 default-members —— 为避免覆盖你的设置，请先在 Cargo.toml 里手动处理它，再重跑 `cargo xmake slim --apply`"
                    .into(),
            )
        }
        None => out.insert(ws_hdr + 1, render_line(keep)),
    }

    // 保留原来的换行风格（Windows 上很多仓库是 CRLF，别顺手给改成 LF）
    let crlf = txt.contains("\r\n");
    let sep = if crlf { "\r\n" } else { "\n" };
    let body = out.join(sep);
    let tail = if txt.ends_with('\n') { sep } else { "" };
    std::fs::write(manifest, format!("{body}{tail}"))
        .map_err(|e| format!("写 Cargo.toml 失败: {e}"))
}

/// `cargo xmake undo` 用：删掉我们写的那一行。返回是否真的动了文件。
pub fn undo(manifest: &Path) -> bool {
    let Ok(txt) = std::fs::read_to_string(manifest) else {
        return false;
    };
    if !txt.contains(MARKER) {
        return false;
    }
    let crlf = txt.contains("\r\n");
    let mut out: Vec<String> = Vec::new();
    let mut removed = false;
    for l in txt.lines() {
        if l.contains(MARKER) && l.trim_start().starts_with("default-members") {
            removed = true;
            continue;
        }
        out.push(l.to_string());
    }
    if !removed {
        return false;
    }
    let sep = if crlf { "\r\n" } else { "\n" };
    let body = out.join(sep);
    let tail = if txt.ends_with('\n') { sep } else { "" };
    let _ = std::fs::write(manifest, format!("{body}{tail}"));
    true
}

// -------------------------------------------------------------------- 命令

fn err(style: &Style, msg: &str) -> Option<i32> {
    eprintln!("{} {msg}", style.red("错误："));
    Some(1)
}

pub fn cmd_slim(root: &Path, opts: &Opts, style: &Style) -> Option<i32> {
    let ws = util::cargo_workspace_root(root).unwrap_or_else(|| root.to_path_buf());
    let manifest = ws.join("Cargo.toml");

    println!("{}", style.bold_cyan("cargo-xmake slim"));
    println!(
        "  {}",
        style.dim("（减编译量 —— cargo 内环跑到 xmake 前面去的唯一大杠杆）")
    );
    println!();

    if !manifest.is_file() {
        return err(style, "没找到工作区根的 Cargo.toml");
    }

    let analysis = match analyze(root) {
        Ok(a) => a,
        Err(e) => return err(style, &e),
    };

    println!(
        "  {} {} 个成员，全量一起构建要编 {} 个包",
        style.dim("规模："),
        style.bold(&analysis.entries.len().to_string()),
        style.bold(&analysis.total_all.to_string())
    );
    println!();
    println!(
        "  {}",
        style.bold("「独占包数」= 去掉这个成员就少编多少包（按省得多少排序）：")
    );
    for e in analysis.entries.iter().take(opts.top.max(1)) {
        let pct = if analysis.total_all > 0 {
            (e.exclusive as f64 / analysis.total_all as f64) * 100.0
        } else {
            0.0
        };
        println!(
            "    {}  独占 {} 个（{:.1}%）  闭包共 {} 个",
            style.cyan(&e.name),
            style.bold(&e.exclusive.to_string()),
            pct,
            style.dim(&e.closure_len.to_string())
        );
    }

    // 决定丢哪些：显式 --drop > --auto（独占占比 ≥5%）> 只在报告里建议
    let mut drop: Vec<String> = Vec::new();
    if !opts.slim_drop.is_empty() {
        // ⚠️ 显式输入必须校验 —— 打错一个字得到静默的"什么都没丢"，看着像结论。
        for d in &opts.slim_drop {
            if !analysis.entries.iter().any(|e| e.name == *d) {
                return err(
                    style,
                    &format!("--drop 里的 `{d}` 不是这个 workspace 的成员（成员名见上表）"),
                );
            }
        }
        drop = opts.slim_drop.clone();
    } else if opts.slim_auto {
        let limit = (analysis.total_all as f64) * 0.05;
        drop = analysis
            .entries
            .iter()
            .filter(|e| (e.exclusive as f64) >= limit)
            .map(|e| e.name.clone())
            .collect();
    }

    if drop.is_empty() {
        println!();
        println!(
            "  {} 给 `--drop a,b` 或 `--auto`（自动挑独占 ≥5% 的）看收益，加 `--apply` 落盘。",
            style.dim("试算：")
        );
        return Some(0);
    }

    let set: HashSet<String> = drop.iter().cloned().collect();
    let new_total = analysis.total_without(&set);
    let saved = analysis.total_all.saturating_sub(new_total);
    let pct = if analysis.total_all > 0 {
        (saved as f64 / analysis.total_all as f64) * 100.0
    } else {
        0.0
    };
    let keep: Vec<String> = analysis
        .names()
        .into_iter()
        .filter(|n| !set.contains(n))
        .collect();

    println!();
    println!("  {} 丢掉：{}", style.yellow("试算："), style.red(&drop.join(", ")));
    println!(
        "  {} 保留 {} 个成员 → 编 {} 个包（原 {}，省 {} 个 / {:.1}%）",
        style.dim("结果："),
        style.bold(&keep.len().to_string()),
        style.bold(&new_total.to_string()),
        analysis.total_all,
        style.green(&saved.to_string()),
        pct
    );

    if !opts.slim_apply {
        println!();
        println!(
            "  {} 加 `--apply` 才会改 {}",
            style.dim("只读："),
            style.bold(util::pretty_path(&manifest))
        );
        return Some(0);
    }

    match write_default_members(&manifest, &keep) {
        Ok(()) => {
            println!();
            println!(
                "{} 已写入 {}",
                style.bold("已落盘："),
                style.bold(util::pretty_path(&manifest))
            );
            println!(
                "  {} 裸命令会走 default-members；CI 用 `-p <crate>` 本就显式指定，不受影响。",
                style.dim("注意：")
            );
            println!("  {} cargo xmake undo", style.dim("撤销："));
            Some(0)
        }
        Err(e) => err(style, &e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const META: &str = r##"{
  "packages": [
    {"name":"member-a","version":"0.1.0","id":"pa"},
    {"name":"member-b","version":"0.1.0","id":"pb"},
    {"name":"shared","version":"1.0.0","id":"sh"},
    {"name":"heavy","version":"1.0.0","id":"hv"}
  ],
  "workspace_members": ["pa","pb"],
  "resolve": {"nodes":[
    {"id":"pa","deps":[{"name":"shared","pkg":"sh"}]},
    {"id":"pb","deps":[{"name":"shared","pkg":"sh"},{"name":"heavy","pkg":"hv"}]},
    {"id":"sh","deps":[]},
    {"id":"hv","deps":[]}
  ]}
}"##;

    #[test]
    fn arrays_of_strings_and_objects_are_scanned() {
        let s = r#"["a","b","c"]"#;
        assert_eq!(strings_in_array(s, 0), vec!["a", "b", "c"]);
        // 孤独的对象 + 嵌套数组不能把对象边界算错
        let s2 = r#"[{"x":1},{"y":[1,2]}]"#;
        let spans = object_spans_in_array(s2, 0);
        assert_eq!(spans.len(), 2);
        assert_eq!(s2.get(spans[1].0..spans[1].1), Some(r#"{"y":[1,2]}"#));
    }

    #[test]
    fn metadata_graph_is_parsed() {
        let g = parse_metadata(META).expect("合成 metadata 应当能解析");
        assert_eq!(g.members, vec!["pa", "pb"]);
        assert_eq!(g.names.get("pa").map(|s| s.as_str()), Some("member-a"));
        assert_eq!(g.edges.get("pb").map(|v| v.len()), Some(2));
        assert_eq!(g.edges.get("sh").map(|v| v.len()), Some(0));
    }

    #[test]
    fn closure_and_exclusive_counts_the_right_packages() {
        let g = parse_metadata(META).unwrap();
        assert_eq!(closure_of("pa", &g.edges).len(), 2); // pa + shared
        assert_eq!(closure_of("pb", &g.edges).len(), 3); // pb + shared + heavy

        let a = analyze_graph(&g);
        // 全量并集 = {pa,pb,shared,heavy}
        assert_eq!(a.total_all, 4);
        // pb 独占 {pb, heavy} = 2；member-a 独占 {pa} = 1
        let by_name = |n: &str| -> usize {
            a.entries
                .iter()
                .find(|e| e.name == n)
                .map(|e| e.exclusive)
                .expect("成员应在表内")
        };
        assert_eq!(by_name("member-b"), 2);
        assert_eq!(by_name("member-a"), 1);
        // 排序：独占多的在前
        assert_eq!(a.entries[0].name, "member-b");

        // 丢掉 member-b：只剩 closure(pa) = {pa, shared} = 2 → 省 50%
        let mut d = HashSet::new();
        d.insert("member-b".to_string());
        assert_eq!(a.total_without(&d), 2);

        // 什么都不丢 = 全量
        assert_eq!(a.total_without(&HashSet::new()), 4);
    }

    #[test]
    fn existing_default_members_is_never_clobbered() {
        let dir = std::env::temp_dir().join(format!("xmk-slim-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let p = dir.join("Cargo.toml");
        let mine = "[workspace]\ndefault-members = [\"a\"]\nmembers = [\"a\", \"b\"]\n";
        std::fs::write(&p, mine).unwrap();
        let e = write_default_members(&p, &["a".to_string(), "b".to_string()])
            .expect_err("用户自己写的 default-members 必须被拒绝");
        assert!(e.contains("手动处理"), "错误信息要说明怎么办：{e}");
        assert_eq!(
            std::fs::read_to_string(&p).unwrap(),
            mine,
            "被拒绝时不能改动文件"
        );
    }

    #[test]
    fn write_then_undo_is_byte_exact() {
        let dir = std::env::temp_dir().join(format!("xmk-slim-rt-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let p = dir.join("Cargo.toml");
        let before = "[workspace]\nmembers = [\"a\", \"b\"]\nresolver = \"2\"\n";
        std::fs::write(&p, before).unwrap();

        write_default_members(&p, &["a".to_string()]).expect("首次写入应成功");
        let after = std::fs::read_to_string(&p).unwrap();
        assert!(after.contains(MARKER));
        assert!(after.contains("default-members = [\"a\"]"));
        // 插在 [workspace] 之后、其余内容一个字不动
        assert!(after.contains("members = [\"a\", \"b\"]"));

        assert!(undo(&p), "undo 应删掉自己写的那一行");
        assert_eq!(
            std::fs::read_to_string(&p).unwrap(),
            before,
            "undo 必须逐字节还原"
        );
        // 幂等：再 undo 一次不能报错，也不改文件
        assert!(!undo(&p));
        assert_eq!(std::fs::read_to_string(&p).unwrap(), before);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
