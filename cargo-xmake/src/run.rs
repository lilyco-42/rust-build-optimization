//! 透明转发器 —— 把命令原样交给 cargo，只在必要处加料。
//!
//! ⚠️ 一条硬规则：**默认不加环境变量。**
//! 实测：`CARGO_PROFILE_DEV_*` 每切一次，cargo 就报
//! "the profile configuration changed"，整棵依赖树重编
//! （`cargo build` → `cargo xmake build` → `cargo build` 三连，每次都全量）。
//! 所以剖面设置一律走 `.cargo/config.toml`（`setup` 写入），
//! 这样两条命令共用同一份指纹和同一份缓存。
//!
//! 只有两类东西必须走环境变量，而且都是**显式选择**的：
//!   * `--xmk-cranelift`：`-Z` 开关只能挂在 rustc 命令行上；
//!   * `--xmk-lld`：换链接器。

use crate::cli::Opts;
use crate::util;
use std::env;
use std::path::Path;
use std::process::{Command, Stdio};

pub struct Plan {
    pub cargo_args: Vec<String>,
    pub envs: Vec<(String, String)>,
    pub notes: Vec<String>,
}

/// 组装要执行的 cargo 命令。
pub fn build_plan(cmd: &str, args: &[String], opts: &Opts) -> Plan {
    let msvc = util::is_msvc();
    let mut cargo_args = vec![cmd.to_string()];
    cargo_args.extend(args.iter().cloned());

    let mut envs: Vec<(String, String)> = Vec::new();
    let mut notes: Vec<String> = Vec::new();

    // ---- cranelift ----
    if opts.cranelift {
        let mut flags: Vec<String> = Vec::new();
        if let Ok(v) = env::var("RUSTFLAGS") {
            if !v.trim().is_empty() {
                flags.push(v);
            }
        }
        flags.push("-Zcodegen-backend=cranelift".into());
        if msvc && !opts.no_link_flags {
            // RUSTFLAGS 会整体覆盖 config 里的 rustflags，所以这里要自己补回来
            flags.push("-Clink-args=/DEBUG:NONE".into());
        }
        envs.push(("RUSTFLAGS".into(), flags.join(" ")));
        notes.push(
            "cranelift：仅供 dev。它会**改动指纹**，与非 cranelift 构建互相失效；且不支持 lto=fat"
                .into(),
        );
        // -Z 需要 nightly：把 +nightly 插到 cargo 参数最前面
        cargo_args.insert(0, "+nightly".into());
    }

    // ---- lld ----
    if opts.lld {
        if msvc {
            notes.push(
                "lld：MSVC 上实测几乎无收益（只影响链接，而链接在总时长里只占几秒），已忽略"
                    .into(),
            );
        } else {
            let mut flags = env::var("RUSTFLAGS").unwrap_or_default();
            if !flags.trim().is_empty() {
                flags.push(' ');
            }
            flags.push_str("-C link-arg=-fuse-ld=lld");
            envs.push(("RUSTFLAGS".into(), flags));
            notes.push("lld：换用 lld 链接器".into());
        }
    }

    Plan {
        cargo_args,
        envs,
        notes,
    }
}

pub struct RunResult {
    pub code: i32,
    /// 退出码 127 表示 cargo 不存在
    pub not_found: bool,
}

pub fn execute(root: &Path, plan: &Plan, opts: &Opts, style: &util::Style) -> RunResult {
    let cargo = util::cargo_bin();
    if opts.trace {
        println!("{}", style.dim("── cargo-xmake 实际执行 ──"));
        let mut shown = cargo.to_string_lossy().to_string();
        for a in &plan.cargo_args {
            shown.push(' ');
            shown.push_str(a);
        }
        println!("  {}", style.cyan(&shown));
        if plan.envs.is_empty() {
            println!("  {}", style.dim("(不额外注入任何环境变量 —— 指纹保持稳定)"));
        } else {
            for (k, v) in &plan.envs {
                println!("  {}={}", style.yellow(k), v);
            }
        }
        for n in &plan.notes {
            println!("  {} {n}", style.dim("i"));
        }
        println!();
    }

    let mut cmd = Command::new(&cargo);
    cmd.current_dir(root);
    cmd.args(&plan.cargo_args);
    cmd.stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());
    for (k, v) in &plan.envs {
        cmd.env(k, v);
    }

    match cmd.status() {
        Ok(st) => RunResult {
            code: st.code().unwrap_or(1),
            not_found: false,
        },
        Err(e) => {
            eprintln!("{} 启动 cargo 失败: {e}", style.red("错误："));
            RunResult {
                code: 127,
                not_found: true,
            }
        }
    }
}

/// cargo 是否有这个子命令？用于判断该不该加料。
pub fn is_buildish(cmd: &str) -> bool {
    matches!(
        cmd,
        "build"
            | "b"
            | "run"
            | "r"
            | "check"
            | "c"
            | "test"
            | "t"
            | "bench"
            | "clippy"
            | "rustc"
            | "doc"
            | "fix"
            | "install"
            | "nextest"
    )
}
