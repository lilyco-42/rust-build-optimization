//! cargo-xmake —— 让 `cargo` 的 debug 构建有 C/xmake 的速度和体积，
//! release 仍然是满血静态派发。
//!
//! 三条设计原则，全部由实测逼出来：
//!
//! 1. **对 cargo 透明。** 转发 `new/run/build/check/test/...` 全部原样，
//!    自己的开关一律带 `--xmk-` 前缀。
//! 2. **剖面设置写 `.cargo/config.toml`，不写环境变量。**
//!    环境变量每次有/无切换都会让 cargo 全量重编（`the profile configuration
//!    changed`）。写进 config 后指纹稳定、`cargo build` 与 `cargo xmake build`
//!    共用缓存、CI 也自动吃到。
//! 3. **绝不碰 `Cargo.toml`。** config 里的 `[profile.*]` 实测生效且优先级更高，
//!    而且不触发 manifest 的 feature gate（cranelift 就是栽在这上面）。
//!
//! 泛型那件事必须说清楚：**没有任何编译标志能把静态派发变成动态派发。**
//! 所以 `dynify` 是一个显式的源码变换，靠 `cfg(debug_assertions)` 让
//! dev 走 `&dyn Trait`（1 份单态化）、release 走 `<T: Trait>`（内联 + 去虚化）。

pub mod audit;
pub mod cli;
pub mod config;
pub mod dynify;
pub mod plan;
pub mod run;
pub mod slim;
pub mod util;

use cli::{Opts, Parsed};
use plan::{Tier, Val};
use std::path::{Path, PathBuf};
use util::Style;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

// ---------------------------------------------------------------- 入口

pub fn main_with(argv: Vec<String>) -> i32 {
    let style = Style::new();
    let parsed = match cli::parse(&argv) {
        Ok(p) => p,
        Err(errs) => {
            for e in errs {
                eprintln!("{} {e}", style.red("错误："));
            }
            return 2;
        }
    };

    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let root = util::workspace_root(&cwd).unwrap_or(cwd);

    match parsed.cmd.as_str() {
        "help" | "--help" | "-h" => {
            print_help(&style);
            0
        }
        "version" | "--version" | "-V" => {
            println!("cargo-xmake {VERSION}");
            0
        }
        "setup" => cmd_setup(&root, &parsed.opts, &style).unwrap_or_else(|| 0),
        "undo" => cmd_undo(&root, &parsed.opts, &style).unwrap_or_else(|| 0),
        "doctor" => cmd_doctor(&root, &parsed.opts, &style).unwrap_or_else(|| 0),
        "audit" => cmd_audit(&root, &parsed, &style).unwrap_or_else(|| 0),
        "dynify" => cmd_dynify(&root, &parsed, &style).unwrap_or_else(|| 0),
        "slim" => slim::cmd_slim(&root, &parsed.opts, &style).unwrap_or_else(|| 0),
        "selftest" => cmd_selftest(&root, &style).unwrap_or_else(|| 0),
        _ => cmd_passthrough(&root, &parsed, &style),
    }
}

fn fail(style: &Style, msg: &str) -> Option<i32> {
    eprintln!("{} {msg}", style.red("错误："));
    Some(1)
}

// ---------------------------------------------------------------- 转发

fn cmd_passthrough(root: &Path, p: &Parsed, style: &Style) -> i32 {
    // 先给一次温和的提示，但不阻塞、不改任何东西
    if run::is_buildish(&p.cmd) && !config::is_setup(root) {
        eprintln!(
            "{} 还没 setup，这次就是普通 cargo（缓存不受影响）。跑一次 {} 就能拿到 xmake 档的设置。",
            style.yellow("提示："),
            style.bold_cyan("cargo xmake setup")
        );
    }
    let plan = run::build_plan(&p.cmd, &p.args, &p.opts);
    let r = run::execute(root, &plan, &p.opts, style);
    if r.not_found {
        return 127;
    }
    r.code
}

// ---------------------------------------------------------------- setup / undo

fn host_section() -> Option<String> {
    util::host_triple(&[]).map(|t| format!("target.{t}"))
}

fn build_config_plan(opts: &Opts) -> config::Plan {
    let msvc = util::is_msvc();
    let mut profiles = vec![
        (plan::DEV_PROFILE.to_string(), plan::dev_plan(opts.tier)),
        (plan::RELEASE_PROFILE.to_string(), plan::release_plan()),
    ];
    // 用 dynify 的 debug_assertions 开关时，dev 必须保留 debug-assertions
    // 否则 `cfg(debug_assertions)` 在 dev 也为假，dyn 版本永远不会被选上。
    if opts.tier != Tier::Safe {
        if let Some((_, dev)) = profiles.iter_mut().find(|(n, _)| n == plan::DEV_PROFILE) {
            if let Some(s) = dev.iter_mut().find(|s| s.key == "debug-assertions") {
                s.val = Val::Bool(true);
                s.note = "⚠️ dynify 默认用 cfg(debug_assertions) 做开关，dev 必须保持 true";
            }
        }
    }
    let rf = plan::link_flags_for(msvc);
    config::Plan {
        profiles,
        rustflags_section: if opts.no_link_flags { None } else { host_section() },
        rustflags: rf.iter().map(|s| s.to_string()).collect(),
    }
}

fn cmd_setup(root: &Path, opts: &Opts, style: &Style) -> Option<i32> {
    let cfg = build_config_plan(opts);
    let rep = match config::apply(root, &cfg, opts.force) {
        Ok(r) => r,
        Err(e) => return fail(style, &e),
    };

    println!(
        "{} {} ({})",
        style.bold_cyan("cargo-xmake setup"),
        style.bold(&util::pretty_path(&rep.path)),
        style.dim(&format!("tier={}", opts.tier.as_str()))
    );
    println!("{}", style.dim(opts.tier.blurb()));
    println!();
    for e in &rep.edits {
        let (tag, color) = match e.action {
            config::Action::Added => ("+", style.green("add ")),
            config::Action::Replaced => ("~", style.yellow("set ")),
            config::Action::Skipped => ("=", style.dim("skip")),
            config::Action::Unchanged => (" ", style.dim("ok  ")),
            config::Action::Removed => ("-", style.red("del ")),
        };
        let _ = tag;
        let msg = match e.action {
            config::Action::Skipped => format!("{} 保留你原来的值 {}", color, style.dim(&e.what)),
            _ => format!("{} {:<28} {}", color, e.where_, e.what),
        };
        println!("  {msg}");
    }
    let (add, skip, other) = rep.counts();
    println!();
    println!(
        "{} {} 项写入，{} 项跳过（保留你原有的设置），{} 项本来就是对的。",
        style.green("完成："),
        add,
        skip,
        other
    );

    // ---- 覆盖警告：把"你原来的值"摊开讲清楚 ----
    let mut overrides: Vec<(String, String)> = Vec::new();
    for src in config::existing_sources(root) {
        for (section, settings) in &cfg.profiles {
            for s in settings {
                if let Some(old) = src.get(section, s.key) {
                    let new = s.val.to_string();
                    if !config::toml_eq(s.key, old, &new) {
                        overrides.push((
                            format!("[{}] {} = {}  →  {}", section, s.key, old, new),
                            format!("{} · {}", src.label, util::pretty_path(&src.path)),
                        ));
                    }
                }
            }
        }
    }
    if !overrides.is_empty() {
        println!();
        println!("  {}", style.yellow("⚠ 这些设置你原来就有，会被项目配置覆盖掉："));
        for (what, from) in &overrides {
            println!("      {}  {}", style.bold(what), style.dim(&format!("（来自 {from}）")));
        }
        println!(
            "  {}",
            style.dim("    想保留原值：删掉 .cargo/config.toml 里对应那几行（每行都带 `# cargo-xmake` 标记），")
        );
        println!(
            "  {}",
            style.dim("    `cargo xmake undo` 能一次全撤。")
        );
        println!(
            "  {}",
            style.dim("    用户级配置是「隐形」的：同一个项目「没配 profile」和「配了 profile」")
        );
        println!(
            "  {}",
            style.dim("    可能拿到不同的 opt-level，产物大小会因此变化 —— 上面就是这种情况。")
        );
    }

    if opts.tier != Tier::Safe {
        println!();
        println!(
            "  {} {}",
            style.yellow("! 关于 debug-assertions = true"),
            style.dim("这一项和 tier 的其它设置是相反的，刻意为之：")
        );
        println!(
            "  {}",
            style.dim("    dynify 默认用 cfg(debug_assertions) 区分 dev / release，所以 dev 必须是 true，")
        );
        println!(
            "  {}",
            style.dim("    否则 release 用的静态派发分支在 dev 也会被选中，dyn 白改。")
        );
        println!(
            "  {}",
            style.dim("    不接受这个耦合的话：用 `cargo xmake dynify --switch=feature` 改用 feature 门。")
        );
    }
    if rep.changed {
        println!(
            "{}",
            style.dim("  撤销：cargo xmake undo（我们加的每一行都带 `# cargo-xmake` 标记）")
        );
        println!(
            "{}",
            style.dim("  注意：改动 .cargo/config.toml 会让下次构建全量重编一次，之后就一直快了")
        );
    } else {
        println!("{}", style.dim("  无需改动。"));
    }
    Some(0)
}

fn cmd_undo(root: &Path, opts: &Opts, style: &Style) -> Option<i32> {
    println!("{}", style.bold_cyan("cargo-xmake undo"));

    // ---- 1. 源码：把 dynify 生成的一对 cfg 分支折叠回泛型版本 ----
    // 靠 `// cargo-xmake:dyn` 标记做结构性逆变换，不依赖备份文件
    // （备份在验证通过后就删了；留着又会覆盖用户后续的编辑）。
    if let Err(e) = dynify::check_paths(&opts.paths, root) {
        return fail(style, &e);
    }
    let files = dynify::rs_files(&opts.paths, root);
    let mut reverted = 0usize;
    let mut touched = 0usize;
    let mut undo_baks = Vec::new();
    for f in &files {
        let Ok(src) = std::fs::read_to_string(f) else {
            continue;
        };
        let (new, n) = dynify::revert(&src);
        if n == 0 {
            continue;
        }
        // 保险：改之前留一份，写失败也不至于丢源码
        let bak = f.with_file_name(format!(
            "{}.xmake-bak",
            f.file_name().and_then(|x| x.to_str()).unwrap_or("src")
        ));
        if std::fs::copy(f, &bak).is_ok() {
            undo_baks.push(bak);
        }
        if std::fs::write(f, &new).is_err() {
            eprintln!(
                "{} 写回 {} 失败，已跳过（原文件未动）",
                style.red("错误："),
                f.display()
            );
            continue;
        }
        println!(
            "  {} {}  ← 折叠 {} 处 dyn 改写",
            style.red("rev "),
            util::pretty_path(f),
            n
        );
        reverted += n;
        touched += 1;
    }
    if touched > 0 {
        println!(
            "{} 还原了 {} 个文件、{} 处改写（原样备份 *.xmake-bak，确认无误后可删）",
            style.green("源码："),
            touched,
            reverted
        );
    }

    // ---- 2. 配置 ----
    let rep = match config::undo(root) {
        Ok(r) => r,
        Err(e) => return fail(style, &e),
    };
    if rep.removed_file {
        println!(
            "{} {} 里的内容全是我们写的，已把文件删除。",
            style.green("配置："),
            util::pretty_path(&rep.path)
        );
    } else if !rep.changed {
        println!("{} 没有发现 cargo-xmake 写入的配置。", style.dim("配置："));
    } else {
        for e in &rep.edits {
            println!("  {} {}", style.red("del "), e.what);
        }
        println!(
            "{} {} 移除了 {} 行。",
            style.green("配置："),
            util::pretty_path(&rep.path),
            rep.edits.len()
        );
    }

    // ---- 3. slim：删掉 default-members ----
    // 只删带 `# cargo-xmake-slim` 标记的**我们自己写的那一行**；
    // 用户原有的 default-members 从一开始就不让覆盖，所以这里碰不到它。
    let ws = util::cargo_workspace_root(root).unwrap_or_else(|| root.to_path_buf());
    let slim_manifest = ws.join("Cargo.toml");
    let slim_removed = slim::undo(&slim_manifest);
    if slim_removed {
        println!(
            "{} {} 移除了 default-members（恢复成原来的样子）。",
            style.green("slim："),
            util::pretty_path(&slim_manifest)
        );
    }

    if touched == 0 && !rep.changed && !rep.removed_file && !slim_removed {
        println!("{} 没有发现任何 cargo-xmake 留下的改动。", style.dim("undo："));
        return Some(0);
    }
    println!();
    println!(
        "{} 下次构建会全量重编一次（指纹变了）。",
        style.dim("提示：")
    );
    Some(0)
}

// ---------------------------------------------------------------- doctor

fn cmd_doctor(root: &Path, opts: &Opts, style: &Style) -> Option<i32> {
    let cfg_path = config::config_path(root);
    let msvc = util::is_msvc();
    let triple = util::host_triple(&[]).unwrap_or_else(|| "(未知)".into());

    println!("{}", style.bold_cyan("cargo-xmake doctor"));
    println!();
    println!("  {:<16} {}", "项目根", util::pretty_path(root));
    println!("  {:<16} {}", "配置文件", util::pretty_path(&cfg_path));
    println!(
        "  {:<16} {}",
        "配置状态",
        if config::is_setup(root) {
            style.green("已 setup")
        } else {
            style.yellow("未 setup（当前等同于普通 cargo）")
        }
    );
    println!("  {:<16} {}", "host 三元组", triple);
    println!(
        "  {:<16} {}",
        "平台",
        if msvc { "MSVC（会注入 /DEBUG:NONE）" } else { "非 MSVC" }
    );
    println!("  {:<16} {}", "nightly", if util::has_nightly() { "有（audit 可用 -Zprint-mono-items）" } else { "没有（audit 退回数 LLVM IR）" });

    println!();
    // 用 build_config_plan 而不是 plan::dev_plan()，这样显示的就是 setup
    // **真正会写下去**的那一份（含 dyn 开关对 debug-assertions 的强制覆盖）。
    let cfg = build_config_plan(opts);
    println!("{}", style.bold("dev 档会写入的设置："));
    if let Some((_, settings)) = cfg.profiles.iter().find(|(n, _)| n == plan::DEV_PROFILE) {
        print_settings(settings, style, true);
    }
    println!();
    println!("{}", style.bold("release 档会写入的设置："));
    if let Some((_, settings)) = cfg.profiles.iter().find(|(n, _)| n == plan::RELEASE_PROFILE) {
        print_settings(settings, style, false);
    }
    println!();
    println!("{}", style.bold("链接期："));
    if opts.no_link_flags {
        println!("  {}", style.dim("--xmk-no-link-flags 已开，跳过"));
    } else if msvc {
        println!(
            "  {} {}",
            style.green("target.…-msvc / rustflags 追加"),
            style.dim("-C link-args=/DEBUG:NONE")
        );
        println!("  {}", style.dim("  原因：rustc 即使 debug=false 也会给 MSVC 链接器传 /DEBUG，PDB 照生成"));
    } else {
        println!("  {}", style.dim("非 MSVC，无需 /DEBUG:NONE"));
    }

    println!();
    println!("{}", style.bold("剖面设置来源（cargo 会逐级合并，项目覆盖全局）："));
    let sources = config::existing_sources(root);
    if sources.is_empty() {
        println!("  {}", style.dim("没有找到任何已有的 [profile.*] 设置"));
    }
    for s in &sources {
        println!(
            "  {} {}",
            style.cyan(&format!("[{}]", s.label)),
            util::pretty_path(&s.path)
        );
        for (section, kvs) in &s.profiles {
            let body: Vec<String> = kvs.iter().map(|(k, v)| format!("{k} = {v}")).collect();
            println!("      {:<22} {}", section, body.join(", "));
        }
    }
    println!(
        "  {}",
        style.dim("  注意：用户全局的 [profile.*] 是隐形的 —— 同一个项目「什么都没配」")
    );
    println!(
        "  {}",
        style.dim("  和「配了 profile」可能拿到不同的 opt-level，产物大小会因此变化。")
    );

    println!();
    println!("{}", style.bold("转发时会额外注入的环境变量："));
    println!("  {}", style.dim("默认：无。这是刻意的 —— 环境变量一变，cargo 就会全量重编。"));
    println!("  {}", style.dim("仅 --xmk-cranelift / --xmk-lld 会注入 RUSTFLAGS。"));
    Some(0)
}

fn print_settings(list: &[plan::Setting], style: &Style, is_dev: bool) {
    for s in list {
        let warn = s.note.starts_with('⚠');
        let mark = if warn {
            style.yellow("!")
        } else if is_dev {
            style.green("•")
        } else {
            style.cyan("•")
        };
        println!("  {} {:<16} {}", mark, s.key, s.val);
        println!("      {}", style.dim(s.note));
    }
}

// ---------------------------------------------------------------- audit

fn cmd_audit(root: &Path, p: &Parsed, style: &Style) -> Option<i32> {
    let opts = &p.opts;
    println!("{}", style.bold_cyan("cargo-xmake audit"));
    println!(
        "  {}",
        style.dim("正在采集单态化数据（额外参数只作用于目标 crate，依赖复用缓存）…")
    );
    let (entries, source, diag) = match audit::collect(root, opts) {
        Ok(x) => x,
        Err(e) => return fail(style, &e),
    };
    if !diag.is_empty() {
        println!("  {}", style.dim(&diag));
    }
    let (groups, roots) = audit::group(&entries);
    let n_entries: usize = entries.iter().map(|e| e.count).sum();
    // 谁的地盘：本地（dynify 能改）还是 std/依赖（改不到）。
    // 这个区分决定了下面结论段能不能给出"做得到"的建议。
    let (dep_crates, local_crates) = util::crate_name_sets(root);
    let rep = audit::Report {
        package: opts
            .package
            .clone()
            .or_else(|| read_pkg_name(root))
            .unwrap_or_else(|| "(本包)".into()),
        profile: opts.profile.clone().unwrap_or_else(|| "dev".into()),
        source,
        entries: n_entries,
        groups,
        roots,
        local_crates,
        dep_crates,
    };

    println!();
    println!(
        "  包 {}   剖面 {}   数据源 {}",
        style.bold(&rep.package),
        rep.profile,
        style.dim(rep.source.label())
    );
    println!("  {}", style.dim(&"─".repeat(66)));
    if source.has_cost() {
        println!(
            "  单态化份数 {:<8} 不同的「根」 {:<8} 平均复制 {:.1}×",
            style.bold(&n_entries.to_string()),
            style.bold(&roots.to_string()),
            rep.duplication()
        );
    } else {
        println!(
            "  单态化条目 {:<8} 不同的「根」 {:<8} 平均复制 {:.1}×",
            style.bold(&n_entries.to_string()),
            style.bold(&roots.to_string()),
            rep.duplication()
        );
        println!(
            "  {}",
            style.dim("  （这个数据源没有 rustc 的代价模型，所以按份数排，不是按收益排）")
        );
    }
    if let Some(c) = source.caveat() {
        println!("  {}", style.yellow(&format!("  ⚠ {c}")));
    }
    println!();

    if rep.groups.is_empty() {
        println!("  没有采到任何条目。");
        return Some(0);
    }

    let cost_hdr = if source.has_cost() { "代价" } else { "" };
    println!(
        "  {} {} {} {}",
        util::pad_visible(&style.bold("排名"), 6),
        util::pad_visible(&style.bold("份数"), 7),
        util::pad_visible(&style.bold(cost_hdr), 9),
        style.bold("根")
    );

    let top = opts.top.min(rep.groups.len());
    for (i, g) in rep.groups.iter().take(top).enumerate() {
        // 两色语义（第三种情况不存在，见下）：
        //   黄色 = 泛型函数体被复制，dyn 化能合并
        //   青色 = 每个具体类型一份的 impl/vtable，dyn 化动不了
        // 注意 `duplicated()` 的定义里**已经排除**了 trait 方法，所以
        // 「vtable」这个分支必须用 `is_vtable_slot()` 判断，
        // 不能用 `duplicated() && is_trait_method(...)` —— 后者永远为假，标签会消失。
        let duplicated = g.duplicated();
        let vtable = g.is_vtable_slot();
        let raw = format!("{:<7}", g.count);
        let count = if g.count <= 1 {
            style.dim(&raw)
        } else if vtable {
            style.cyan(&raw)
        } else {
            // 份数 > 1 且不是 trait 方法，只可能是实例化复制。
            style.yellow(&raw)
        };
        let cost = g
            .total
            .map(|t| format!("{:<9}", t))
            .unwrap_or_else(|| " ".repeat(9));

        let key = if g.key.chars().count() > 54 {
            format!("{}…", g.key.chars().take(53).collect::<String>())
        } else {
            g.key.clone()
        };
        let (key, tag) = if vtable {
            (key, style.dim("  · vtable，每类型一份，dyn 去不掉"))
        } else if duplicated {
            // ⚠️ 光是"被复制了"不等于"我们改得到"。真实项目里被复制得最凶的
            // 几乎全是 std/alloc 的泛型（`Iterator::fold`、`Vec::extend_desugared`…），
            // 而 dynify 只能改本 workspace 的源码 —— 不标出来，
            // 用户会以为跑一下 dynify 就能省 44%。
            match rep.place(&g.key) {
                audit::Place::Local => (
                    style.bold(&key),
                    style.yellow("  · 可 dyn 化（在你的源码里）"),
                ),
                audit::Place::Dep => (
                    key,
                    style.dim("  · 被复制，但在**依赖库**里，dynify 改不到"),
                ),
                audit::Place::Std => (
                    key,
                    style.dim("  · 被复制，但在标准库里，dynify 改不到"),
                ),
            }
        } else {
            (key, String::new())
        };
        println!(
            "  {} {} {} {}{}",
            util::pad_visible(&style.dim(&format!("#{}", i + 1)), 6),
            count,
            cost,
            key,
            tag
        );
    }
    if rep.groups.len() > top {
        println!(
            "  {}",
            style.dim(&format!(
                "… 还有 {} 个根（--xmk-top=N 看更多）",
                rep.groups.len() - top
            ))
        );
    }

    // ---- 结论 ----
    let re = rep.reclaimable();
    let inherent = rep.type_inherent();
    let local = rep.reclaimable_local();
    let foreign = rep.reclaimable_foreign();
    println!();
    if re == 0 {
        println!("{}", style.green("结论： 没有可 dyn 化的泛型函数热点了。"));
    } else {
        print!(
            "{} 有 {} 份是「同一个泛型函数体被复制」",
            style.bold("结论："),
            style.yellow(&re.to_string())
        );
        if let (Some(rc), Some(tc)) = (rep.reclaimable_cost(), rep.total_cost()) {
            if tc > 0 {
                print!(
                    "，估算代价 {}（占全部代价的 {:.0}%）",
                    style.yellow(&rc.to_string()),
                    rc as f64 * 100.0 / tc as f64
                );
            }
        }
        println!("。");
        let local_pct = local as f64 * 100.0 / n_entries.max(1) as f64;
        println!(
            "  其中 {} 份在**你的源码**里（{}），另外 {} 份在依赖库/标准库里（{}）。",
            style.yellow(&local.to_string()),
            style.cyan("dynify 能改"),
            style.dim(&foreign.to_string()),
            style.dim("改不到")
        );
        if local == 0 {
            println!(
                "  {}",
                style.bold("这个项目的泛型复制全在你的代码之外，dynify 在这里帮不上忙。")
            );
            println!(
                "  {}",
                style.dim(
                    "  想压掉这部分只能从别处下手：减少具体类型数量（比如给泛型参数做类型擦除）、\
                     换掉泛型重的依赖、或用 `cargo xmake setup` 先拿编译速度。"
                )
            );
        } else if local_pct < 1.0 {
            // 本地份数在总量里不到 1% —— 老实说"不值得做"，别把 dynify 塞给用户
            println!(
                "  {} 本地那 {} 份就算全改掉，也只占总份数的 {:.2}% —— {}",
                style.bold("收益很小："),
                local,
                local_pct,
                style.dim("dynify 在这个项目上不值得做。")
            );
            println!(
                "  {}",
                style.dim(
                    "  真正的大头是上面那些标准库/依赖里的泛型，和每类型一份的 vtable 槽位 ——\
                     两条路 dynify 都碰不到。"
                )
            );
        } else {
            println!(
                "  改本地那些，预期能少掉约 {:.0}% 的单态化份数。",
                local_pct
            );
            println!(
                "  {} 下一个动作：{}",
                style.dim(""),
                style.bold_cyan("cargo xmake dynify")
            );
        }
    }
    if inherent > 0 {
        println!(
            "  另有 {} 份是 {} 形态（青色）—— 每个具体类型必然一份，dyn 化消不掉；",
            style.cyan(&inherent.to_string()),
            style.dim("`<# as Trait>::`")
        );
        println!(
            "  {}",
            style.dim("  想减这部分只能减少具体类型本身的数量（或让它们共用一份 impl）。")
        );
    }
    println!();
    println!(
        "  {}",
        style.dim("量级参考：N=200 实测去泛型让 LLVM IR −69%、dev exe −22.7%，release 产物不变。")
    );
    Some(0)
}

fn read_pkg_name(root: &Path) -> Option<String> {
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

// ---------------------------------------------------------------- dynify

fn cmd_dynify(root: &Path, p: &Parsed, style: &Style) -> Option<i32> {
    let opts = &p.opts;
    // 显式给了路径就先验一遍：不存在的路径要当场报错，
    // 不能让它退化成"扫到 0 个泛型函数"（那会被当成结论）
    if let Err(e) = dynify::check_paths(&opts.paths, root) {
        return fail(style, &e);
    }
    let cands = match dynify::scan_paths(&opts.paths, root) {
        Ok(c) => c,
        Err(e) => return fail(style, &e),
    };
    let ok: Vec<&dynify::Candidate> = cands.iter().filter(|c| c.ok()).collect();
    let bad: Vec<&dynify::Candidate> = cands.iter().filter(|c| !c.ok()).collect();

    println!(
        "{} {}",
        style.bold_cyan("cargo-xmake dynify"),
        style.dim(if opts.rewrite { "（改写模式）" } else { "（只读预览，加 --rewrite 才落盘）" })
    );
    println!(
        "  扫描 {} 个泛型函数：{} 个可改，{} 个不改",
        cands.len(),
        style.green(&ok.len().to_string()),
        style.dim(&bad.len().to_string())
    );
    println!(
        "  开关 {}  →  {}",
        style.bold(opts.switch.as_str()),
        style.dim(match opts.switch {
            cli::Switch::DebugAssertions => "dev 用 cfg(debug_assertions)=true 选 dyn，release 自动关",
            cli::Switch::Feature => "dev 需 cargo xmake 自动加 --features xmake-dyn",
        })
    );
    println!();

    if ok.is_empty() {
        println!("  没有可自动改写的候选。");
    }
    for (i, c) in ok.iter().take(opts.top.max(1)) .enumerate() {
        println!(
            "  {}{}  {}:{}",
            style.green("可改 "),
            style.bold(&c.name),
            util::pretty_path(&c.file),
            c.line
        );
        println!("        {} <{}: {}>  函数体 {} 字节", style.dim("泛型"), c.param, c.bound, c.body.len());
        if !opts.rewrite {
            let rendered = c.render(opts.switch);
            for l in rendered.lines().take(6) {
                println!("        {}", style.dim(l));
            }
            if rendered.lines().count() > 6 {
                println!("        {}", style.dim("…"));
            }
            let _ = i;
        }
    }
    if ok.len() > opts.top.max(1) {
        println!("  {}", style.dim(&format!("… 还有 {} 个", ok.len() - opts.top.max(1))));
    }

    if !bad.is_empty() {
        println!();
        println!("  {}", style.bold("不改的（附原因）："));
        for c in bad.iter().take(8) {
            println!(
                "    {} {}  {}",
                style.dim("skip"),
                c.name,
                style.dim(c.reject.as_deref().unwrap_or(""))
            );
        }
    }

    if !opts.rewrite {
        println!();
        println!(
            "  {} 调用点一个字都不用改 —— `f(&x)` 在 `&T` 和 `&dyn Trait` 两种签名下都能编译。",
            style.dim("提示：")
        );
        println!(
            "  {} {}",
            style.dim("落盘："),
            style.bold_cyan("cargo xmake dynify --rewrite")
        );
        return Some(0);
    }

    // ---- 落盘 + 编译验证 + 失败自动回滚 ----
    let list: Vec<dynify::Candidate> = ok.iter().map(|c| (*c).clone_for_apply()).collect();
    let (n, backups) = match dynify::apply(&list, opts.switch) {
        Ok(x) => x,
        Err(e) => return fail(style, &e),
    };
    println!();
    println!("{} 改写了 {} 个函数，正在编译验证…", style.bold("已落盘："), n);
    // `cargo check` 与 `cargo build` 的指纹不同，同一份源码会在
    // `target/<profile>/incremental/` 下**多留一份孤儿缓存**（实测单文件项目 1.7 MB，
    // 大项目按比例更大）。沿用 `-Zdump-mono-stats` 那套纪律：只删本次自己新建的，
    // 用户原有的缓存一个都不碰。
    let before_inc = incremental_dirs(root);
    let check = std::process::Command::new(util::cargo_bin())
        .current_dir(root)
        .arg("check")
        .arg("--quiet")
        .output();
    let swept = sweep_incremental(&before_inc, root);
    let swept_note = if swept.0 > 0 {
        format!(
            "（顺带清掉验证留下的 {} 个缓存目录，{}）",
            swept.0,
            util::bytes_human(swept.1)
        )
    } else {
        String::new()
    };
    match check {
        Ok(o) if o.status.success() => {
            for b in &backups {
                let _ = std::fs::remove_file(b);
            }
            println!(
                "{} 编译通过，备份已清理。{}",
                style.green("验证："),
                style.dim(&swept_note)
            );
            Some(0)
        }
        Ok(o) => {
            let msg = String::from_utf8_lossy(&o.stderr);
            match dynify::restore(&backups) {
                Ok(files) => eprintln!(
                    "{} 编译没过，已自动回滚 {} 个文件。",
                    style.red("验证失败："),
                    files.len()
                ),
                Err(e) => eprintln!("{} 自动回滚也失败了：{e}（备份文件请手动恢复）", style.red("严重：")),
            }
            let mut lines: Vec<&str> = msg.lines().filter(|l| l.contains("error")).collect();
            lines.truncate(12);
            for l in lines {
                eprintln!("    {l}");
            }
            eprintln!();
            eprintln!(
                "{}",
                style.dim("常见原因：函数体里用了 T 的关联类型/关联函数，对象安全不成立。")
            );
            Some(1)
        }
        Err(e) => {
            eprintln!("{} 无法运行 cargo check: {e}", style.red("错误："));
            match dynify::restore(&backups) {
                Ok(files) => eprintln!("  已回滚 {} 个文件。", files.len()),
                Err(e2) => eprintln!("{} 回滚失败：{e2}", style.red("严重：")),
            }
            Some(1)
        }
    }
}

/// `<target>/debug/incremental` 下的全部子目录。
fn incremental_dirs(root: &Path) -> Vec<PathBuf> {
    let base = util::target_dir(root).join("debug").join("incremental");
    let Ok(rd) = std::fs::read_dir(&base) else {
        return Vec::new();
    };
    let mut v: Vec<PathBuf> = rd
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    v.sort();
    v
}

/// 删掉 `before` 里没有的增量缓存目录，返回 (目录数, 字节数)。
fn sweep_incremental(before: &[PathBuf], root: &Path) -> (usize, u64) {
    let mut n = 0usize;
    let mut bytes = 0u64;
    for p in incremental_dirs(root) {
        if before.contains(&p) {
            continue;
        }
        bytes += util::dir_size(&p);
        if std::fs::remove_dir_all(&p).is_ok() {
            n += 1;
        }
    }
    (n, bytes)
}

impl dynify::Candidate {
    fn clone_for_apply(&self) -> dynify::Candidate {        dynify::Candidate {
            file: self.file.clone(),
            line: self.line,
            name: self.name.clone(),
            param: self.param.clone(),
            bound: self.bound.clone(),
            head: self.head.clone(),
            generics: self.generics.clone(),
            tail: self.tail.clone(),
            sig: self.sig.clone(),
            body: self.body.clone(),
            start: self.start,
            end: self.end,
            reject: self.reject.clone(),
        }
    }
}

// ---------------------------------------------------------------- selftest

fn cmd_selftest(_root: &Path, style: &Style) -> Option<i32> {
    println!("{}", style.bold_cyan("cargo-xmake selftest"));
    println!("  {}", style.dim("在临时目录造一个最小 crate，验证「写 config → 设置真的到达 rustc」整条链路"));
    let tmp = std::env::temp_dir().join(format!("xmake-selftest-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    if let Err(e) = std::fs::create_dir_all(&tmp) {
        return fail(style, &format!("建临时目录失败: {e}"));
    }

    let ok = (|| -> Result<Vec<String>, String> {
        let mut log = Vec::new();
        // 造 crate
        std::fs::create_dir_all(tmp.join("src")).map_err(|e| e.to_string())?;
        std::fs::write(
            tmp.join("Cargo.toml"),
            "[package]\nname = \"xmake-selftest\"\nversion = \"0.0.0\"\nedition = \"2021\"\n\n[dependencies]\n",
        )
        .map_err(|e| e.to_string())?;
        std::fs::write(tmp.join("src/main.rs"), "fn main() { println!(\"hi\"); }\n")
            .map_err(|e| e.to_string())?;

        // 注入
        let opts = Opts::default();
        let cfg = build_config_plan(&opts);
        let rep = config::apply(&tmp, &cfg, false)?;
        log.push(format!("写入 {} 行到 {}", rep.edits.len(), util::pretty_path(&rep.path)));

        // 编译并抓 rustc 命令行
        let out = std::process::Command::new(util::cargo_bin())
            .current_dir(&tmp)
            .args(["build", "-v"])
            .env("CARGO_TERM_COLOR", "never")
            .output()
            .map_err(|e| format!("跑 cargo build -v 失败: {e}"))?;
        let text = String::from_utf8_lossy(&out.stderr).to_string();
        if !out.status.success() {
            return Err(format!("构建失败：\n{}", text.lines().take(10).collect::<Vec<_>>().join("\n")));
        }

        let mut checked = 0;
        for (label, needle) in [
            ("-C strip=debuginfo", "-C strip=debuginfo"),
            ("-C debuginfo（应消失）", "-C debuginfo=2"),
            ("-C link-args=/DEBUG:NONE", "link-args=/DEBUG:NONE"),
        ] {
            let present = text.contains(needle);
            match label {
                "-C debuginfo（应消失）" => {
                    if present {
                        return Err("debug=false 没生效，仍然看到 -C debuginfo=2".into());
                    }
                    log.push(format!("✔ {label}"));
                    checked += 1;
                }
                _ => {
                    if !present {
                        // strip/link-args 在 rustc 里可能被拆开，做个宽松点的兜底
                        let loose = text.contains(&needle.replace("-C ", "").replace("-C", ""));
                        if !loose {
                            return Err(format!("没在 rustc 命令行里看到 `{needle}`"));
                        }
                    }
                    log.push(format!("✔ {label}"));
                    checked += 1;
                }
            }
        }

        // 产物体积
        let exe = tmp.join("target").join("debug").join("xmake-selftest.exe");
        if let Ok(m) = std::fs::metadata(&exe) {
            log.push(format!("dev exe {}", util::bytes_human(m.len())));
            let pdb = exe.with_extension("pdb");
            log.push(format!(
                "dev pdb {}",
                if pdb.exists() {
                    format!("{} ← /DEBUG:NONE 没生效！", util::bytes_human(std::fs::metadata(&pdb).map(|m| m.len()).unwrap_or(0)))
                } else {
                    "不存在 ✔".to_string()
                }
            ));
        }
        let _ = checked;
        let _ = std::fs::remove_dir_all(&tmp);
        Ok(log)
    })();

    match ok {
        Ok(log) => {
            for l in log {
                println!("  {l}");
            }
            println!("{}", style.green("selftest 通过。"));
            Some(0)
        }
        Err(e) => {
            let _ = std::fs::remove_dir_all(&tmp);
            fail(style, &e)
        }
    }
}

// ---------------------------------------------------------------- help

fn print_help(style: &Style) {
    let b = |s: &str| style.bold(s);
    let c = |s: &str| style.bold_cyan(s);
    println!(
        "{} {}

{}  cargo xmake <cargo 子命令> [参数…]        # 原样转发
{}  cargo xmake setup [--tier=safe|fast|extreme]   # 把设置写进 .cargo/config.toml
{}  cargo xmake undo                              # 精确撤销上面写的内容
{}  cargo xmake doctor                            # 打印当前会生效的每一刀
{}  cargo xmake audit [--xmk-top=N] [--xmk-mode=auto|mono|ir]
{}  cargo xmake dynify [路径…] [--rewrite] [--switch=debug-assertions|feature]
{}  cargo xmake selftest                          # 端到端自检

{}（都放在命令行任意位置即可，cargo 不会看到它们）
  --xmk-tier=safe|fast|extreme    dev 档激进度，默认 fast
  --xmk-force                     覆盖你在 config 里已有的同名设置
  --xmk-trace                     打印实际执行的 cargo 命令与注入的环境变量
  --xmk-cranelift                 用 nightly 的 cranelift 后端（dev 专用）
  --xmk-no-link-flags             不注入 /DEBUG:NONE
  --xmk-package=NAME / --xmk-target-dir=PATH

{}  cargo new / cargo run / cargo build / cargo test / clippy / … 全部照旧可用。
      cargo xmake build   等价于 cargo build，只是顺带享受 setup 过的剖面。
      另外还能直接写 cargo-xmake build（不开头的 xmake 会被自动跳过）。

{}  泛型不能靠编译标志解决 —— 实测 -Zshare-generics 改了 IR 一个字节都不变，
      -Zpolymorphize 已从 nightly 移除。所以 dynify 是显式的源码变换，
      用 cfg(debug_assertions) 让 dev 走 &dyn Trait、release 走 <T: Trait>。
      它的代价是源码里同一函数体出现两份，收益则集中在 audit 排出来的头部热点上。
",
        c("cargo-xmake"), style.dim(&format!("v{VERSION}")),
        b("用法："),
        b(""), b(""), b(""), b(""), b(""), b(""),
        b("我们自己的开关"),
        b("兼容性"),
        b("关于泛型"),
    );
}
