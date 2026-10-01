//! 命令行解析。
//!
//! 设计要点：cargo-xmake 必须**对 cargo 完全透明**。
//! 所以：
//!   1. 我们自己的开关统一带 `--xmk-` 前缀，从 argv 的**任意位置**摘掉，
//!      绝不会误吞 cargo 的 flag；
//!   2. 摘完之后剩下的就是纯粹的 `cargo <sub> <args...>`，原样转发；
//!   3. 只有 `setup/undo/doctor/audit/dynify/selftest` 这些 cargo 没有的
//!      子命令才由我们自己处理，而且它们可以额外接受不带前缀的短开关。

use crate::plan::Tier;
use std::path::PathBuf;

pub const OUR_COMMANDS: &[&str] = &[
    "setup", "undo", "doctor", "audit", "dynify", "selftest", "help", "version",
];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Switch {
    /// 用 `cfg(debug_assertions)` 切换 —— dev 开虚函数、release 关。零额外接线。
    DebugAssertions,
    /// 用 `#[cfg(feature = "xmake-dyn")]` 切换 —— 需要 dev 时传 --features。
    Feature,
}

impl Switch {
    pub fn as_str(self) -> &'static str {
        match self {
            Switch::DebugAssertions => "debug-assertions",
            Switch::Feature => "feature",
        }
    }
    pub fn cfg(self) -> &'static str {
        match self {
            Switch::DebugAssertions => "debug_assertions",
            Switch::Feature => "xmake-dyn",
        }
    }
    pub fn feature_name(self) -> Option<&'static str> {
        match self {
            Switch::DebugAssertions => None,
            Switch::Feature => Some("xmake-dyn"),
        }
    }
}

pub struct Opts {
    pub tier: Tier,
    pub force: bool,
    pub trace: bool,
    pub cranelift: bool,
    pub lld: bool,
    pub no_link_flags: bool,
    pub package: Option<String>,
    pub target_dir: Option<PathBuf>,
    pub rewrite: bool,
    pub dry_run: bool,
    pub switch: Switch,
    pub paths: Vec<String>,
    pub top: usize,
    pub mode: AuditMode,
    pub profile: Option<String>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AuditMode {
    /// 自动挑：有 nightly 就用 `-Zdump-mono-stats`，否则退回 LLVM IR
    Auto,
    /// `-Zdump-mono-stats=human -Zdump-mono-stats-format=json`
    /// —— rustc 会给实例化次数 + 代价估算，信息最全
    Stats,
    /// `-Zprint-mono-items=yes` —— 名字现成 demangle 好，但要自己分组
    MonoItems,
    /// `--emit=llvm-ir` 数 `define` —— stable 也能跑，名字要自己拆
    LlvmIr,
}

impl AuditMode {
    pub fn parse(s: &str) -> AuditMode {
        match s {
            "stats" | "mono-stats" => AuditMode::Stats,
            "mono" | "items" => AuditMode::MonoItems,
            "ir" | "llvm-ir" => AuditMode::LlvmIr,
            _ => AuditMode::Auto,
        }
    }
}

impl Default for Opts {
    fn default() -> Self {
        Opts {
            tier: Tier::Fast,
            force: false,
            trace: false,
            cranelift: false,
            lld: false,
            no_link_flags: false,
            package: None,
            target_dir: None,
            rewrite: false,
            dry_run: true, // dynify 默认只看不改
            switch: Switch::DebugAssertions,
            paths: Vec::new(),
            top: 20,
            mode: AuditMode::Auto,
            profile: None,
        }
    }
}

pub struct Parsed {
    pub opts: Opts,
    /// cargo 子命令，或我们自己的子命令
    pub cmd: String,
    /// 子命令之后的参数（已剔除 --xmk-*；我们自己的子命令还剔除了裸开关）
    pub args: Vec<String>,
    /// 是否是我们自己处理的子命令
    pub ours: bool,
}

fn take_value(args: &[String], i: &mut usize, inline: Option<String>) -> Option<String> {
    if let Some(v) = inline {
        return Some(v);
    }
    *i += 1;
    args.get(*i).cloned()
}

/// 从 argv 里摘掉所有 `--xmk-*`，返回 (剩下的, 我们的设置)。
fn strip_xmk(argv: &[String], opts: &mut Opts, errs: &mut Vec<String>) -> Vec<String> {
    let mut out = Vec::with_capacity(argv.len());
    let mut i = 0;
    while i < argv.len() {
        let a = &argv[i];
        let (name, inline) = match a.strip_prefix("--xmk-") {
            Some(rest) => match rest.split_once('=') {
                Some((n, v)) => (n.to_string(), Some(v.to_string())),
                None => (rest.to_string(), None),
            },
            None => {
                out.push(a.clone());
                i += 1;
                continue;
            }
        };
        match name.as_str() {
            "tier" => match take_value(argv, &mut i, inline).as_deref().and_then(Tier::parse) {
                Some(t) => opts.tier = t,
                None => errs.push("--xmk-tier 需要 safe|fast|extreme".into()),
            },
            "force" => opts.force = true,
            "trace" => opts.trace = true,
            "cranelift" => opts.cranelift = true,
            "lld" => opts.lld = true,
            "no-link-flags" | "no-debug-none" => opts.no_link_flags = true,
            "rewrite" => {
                opts.rewrite = true;
                opts.dry_run = false;
            }
            "dry-run" => opts.dry_run = true,
            "package" | "p" => opts.package = take_value(argv, &mut i, inline),
            "target-dir" => opts.target_dir = take_value(argv, &mut i, inline).map(PathBuf::from),
            "top" => {
                if let Some(v) = take_value(argv, &mut i, inline) {
                    match v.parse() {
                        Ok(n) => opts.top = n,
                        Err(_) => errs.push("--xmk-top 需要整数".into()),
                    }
                }
            }
            "mode" => {
                opts.mode = take_value(argv, &mut i, inline)
                    .map(|v| AuditMode::parse(&v))
                    .unwrap_or(AuditMode::Auto);
            }
            "switch" => {
                opts.switch = match take_value(argv, &mut i, inline).as_deref() {
                    Some("feature") => Switch::Feature,
                    _ => Switch::DebugAssertions,
                }
            }
            // `--xmk-help` 等同于要帮助
            "help" | "h" => out.push("--help".into()),
            other => errs.push(format!("未知开关 --xmk-{other}")),
        }
        i += 1;
    }
    out
}

/// 对我们自己的子命令，额外允许不带前缀的开关。
fn strip_bare(args: &[String], opts: &mut Opts, cmd: &str, errs: &mut Vec<String>) -> Vec<String> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < args.len() {
        let a = &args[i];
        let (name, inline) = match a.split_once('=') {
            Some((n, v)) if n.starts_with("--") => (n.to_string(), Some(v.to_string())),
            _ => (a.clone(), None),
        };
        let consumed = match (cmd, name.as_str()) {
            ("setup", "--force") | (_, "--force") => {
                opts.force = true;
                true
            }
            ("setup", "--tier") | ("doctor", "--tier") => {
                if let Some(v) = take_value(args, &mut i, inline) {
                    if let Some(t) = Tier::parse(&v) {
                        opts.tier = t;
                    } else {
                        errs.push("--tier 需要 safe|fast|extreme".into());
                    }
                }
                true
            }
            ("dynify", "--rewrite") => {
                opts.rewrite = true;
                opts.dry_run = false;
                true
            }
            ("dynify", "--dry-run") => {
                opts.dry_run = true;
                true
            }
            ("dynify", "--switch") => {
                opts.switch = match take_value(args, &mut i, inline).as_deref() {
                    Some("feature") => Switch::Feature,
                    _ => Switch::DebugAssertions,
                };
                true
            }
            ("audit", "--mode") => {
                opts.mode = take_value(args, &mut i, inline)
                    .map(|v| AuditMode::parse(&v))
                    .unwrap_or(AuditMode::Auto);
                true
            }
            ("audit", "--top") => {
                if let Some(v) = take_value(args, &mut i, inline) {
                    if let Ok(n) = v.parse() {
                        opts.top = n;
                    }
                }
                true
            }
            ("audit", "--package") | ("audit", "-p") => {
                opts.package = take_value(args, &mut i, inline);
                true
            }
            _ => false,
        };
        if !consumed {
            out.push(a.clone());
        }
        i += 1;
    }
    out
}

pub fn parse(argv: &[String]) -> Result<Parsed, Vec<String>> {
    let mut opts = Opts::default();
    let mut errs = Vec::new();

    // 1. 跳过 argv[0]；cargo 以 `cargo xmake ...` 调用时 argv[1] == "xmake"
    let mut rest: Vec<String> = argv.iter().skip(1).cloned().collect();
    if rest.first().map(|s| s == "xmake").unwrap_or(false) {
        rest.remove(0);
    }

    // 2. 全局摘掉 --xmk-*
    let rest = strip_xmk(&rest, &mut opts, &mut errs);

    // 3. 拆命令
    let (cmd, cmd_args) = match rest.split_first() {
        Some((c, a)) => (c.clone(), a.to_vec()),
        None => ("help".to_string(), Vec::new()),
    };
    let ours = OUR_COMMANDS.contains(&cmd.as_str());

    // 4. 我们自己命令的裸开关 / 顺带把 -p 也认一下
    let cmd_args = if ours {
        let mut a = cmd_args;
        // 允许 `audit --release` 之类
        let mut kept = Vec::new();
        for t in a.drain(..) {
            if t == "--release" {
                opts.profile = Some("release".into());
            } else {
                kept.push(t);
            }
        }
        let kept = strip_bare(&kept, &mut opts, &cmd, &mut errs);
        // 位置参数 = 要扫描的路径（`dynify` / `undo` 用它限定范围）。
        // ⚠️ 这里原来漏了赋值，于是 `opts.paths` 恒为空 —— `cargo xmake dynify <path>`
        // **静默忽略**路径，永远退回 `root/src`；在 workspace 根下没有 `src/`，
        // 结果是"扫到 0 个泛型函数"这种看着像结论的假数据。
        if matches!(cmd.as_str(), "dynify" | "undo") {
            opts.paths = kept
                .iter()
                .filter(|a| !a.starts_with('-'))
                .cloned()
                .collect();
        }
        kept
    } else {
        // cargo 命令：顺手看有没有 --release / --profile
        let mut it = cmd_args.iter().peekable();
        while let Some(t) = it.next() {
            if t == "--release" || t == "-r" {
                opts.profile = Some("release".into());
            } else if let Some(p) = t.strip_prefix("--profile=") {
                opts.profile = Some(p.to_string());
            } else if t == "--profile" {
                if let Some(v) = it.peek() {
                    opts.profile = Some((*v).clone());
                }
            } else if let Some(p) = t.strip_prefix("--package=") {
                opts.package = Some(p.to_string());
            } else if (t == "--package" || t == "-p") && opts.package.is_none() {
                if let Some(v) = it.peek() {
                    opts.package = Some((*v).clone());
                }
            }
        }
        cmd_args
    };

    if !errs.is_empty() {
        return Err(errs);
    }

    Ok(Parsed {
        opts,
        cmd,
        args: cmd_args,
        ours,
    })
}
