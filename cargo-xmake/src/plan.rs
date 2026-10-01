//! 「计划」：cargo-xmake 想给 dev / release 剖面塞什么设置。
//!
//! 全部以数据形式描述，好处是：
//!   * `doctor` 可以直接打印，用户看得见；
//!   * `setup` 可以原样写进 `.cargo/config.toml`（TOML 支持 `[profile.*]`）；
//!   * 想改策略只改这一张表。
//!
//! ⚠️ 为什么写进 config 而不是用 `CARGO_PROFILE_*` 环境变量：
//! 实测（见 docs/06-cargo-xmake.md）—— 环境变量每次「有/无」切换都会让
//! cargo 判定 "the profile configuration changed"，整棵依赖树全部重编。
//! 379 个 crate 的项目就是每切一次 91 秒。写进 `.cargo/config.toml` 之后
//! 指纹是稳定的，`cargo build` 与 `cargo xmake build` 共用同一份缓存，
//! 而且 CI 也能自动吃到。

use std::fmt;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Val {
    Str(&'static str),
    Int(i64),
    Bool(bool),
}

impl fmt::Display for Val {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Val::Str(s) => write!(f, "\"{s}\""),
            Val::Int(n) => write!(f, "{n}"),
            Val::Bool(b) => write!(f, "{b}"),
        }
    }
}

#[derive(Clone, Copy)]
pub struct Setting {
    pub key: &'static str,
    pub val: Val,
    /// 为什么这么设 —— 会出现在 `doctor` 和 setup 的注释里。
    pub note: &'static str,
}

/// dev 档的激进度。
///
/// 关键区分是**改不改语义**：
///   * Safe    —— 只动「产物带不带调试信息」，运行时行为一字不变。
///   * Fast    —— 额外关掉 overflow check / debug_assert，等价于让 dev 用 C 的默认语义。
///   * Extreme —— 再加 panic=abort（失去 unwind，`catch_unwind` 失效，test 由 cargo 兜底）。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tier {
    Safe,
    Fast,
    Extreme,
}

impl Tier {
    pub fn parse(s: &str) -> Option<Tier> {
        match s {
            "safe" => Some(Tier::Safe),
            "fast" => Some(Tier::Fast),
            "extreme" | "max" => Some(Tier::Extreme),
            _ => None,
        }
    }
    pub fn as_str(&self) -> &'static str {
        match self {
            Tier::Safe => "safe",
            Tier::Fast => "fast",
            Tier::Extreme => "extreme",
        }
    }
    pub fn blurb(&self) -> &'static str {
        match self {
            Tier::Safe => "只砍调试信息，运行时语义零改动",
            Tier::Fast => "再关 overflow check / debug_assert，即 C 的默认语义",
            Tier::Extreme => "再加 panic=abort（放弃 unwind）",
        }
    }
}

pub const DEV_PROFILE: &str = "profile.dev";

/// dev 剖面 —— 「像 xmake 一样快、一样小」的那一半。
pub fn dev_plan(tier: Tier) -> Vec<Setting> {
    let mut v = vec![
        Setting {
            key: "debug",
            val: Val::Bool(false),
            note: "不生成 DWARF。PDB/DWARF 是 dev 产物体积的绝对大头",
        },
        Setting {
            key: "strip",
            val: Val::Str("debuginfo"),
            note: "双保险：即使别的来源要求 debug，也在链接期剥掉",
        },
        Setting {
            key: "incremental",
            val: Val::Bool(true),
            note: "内环速度的命根子，绝不关",
        },
    ];
    if tier != Tier::Safe {
        v.push(Setting {
            key: "opt-level",
            val: Val::Int(0),
            note: "0 = 不跑 LLVM 优化 pass，codegen 最快；dev 不需要它优化",
        });
        v.push(Setting {
            key: "debug-assertions",
            val: Val::Bool(false),
            note: "去掉 debug_assert!，C 也没有",
        });
        v.push(Setting {
            key: "overflow-checks",
            val: Val::Bool(false),
            note: "去掉整数溢出检查，C 的默认就是静默回绕",
        });
        v.push(Setting {
            key: "codegen-units",
            val: Val::Int(16),
            note: "cargo dev 默认 256；16 让产物更小且可复现，内环代价可忽略",
        });
    }
    if tier == Tier::Extreme {
        v.push(Setting {
            key: "panic",
            val: Val::Str("abort"),
            note: "省掉 unwind 表。cargo 会为 test 剖面强制 unwind，测试不受影响",
        });
    }
    v
}

pub const RELEASE_PROFILE: &str = "profile.release";

/// release 剖面 —— 「关掉虚函数、要极限性能」的那一半。
///
/// 注意这里**没有**也不需要任何「动态派发」设置：Rust 默认就是静态派发，
/// 单态化 + 内联 + fat LTO 才是 release 要的东西。cargo-xmake 在 release
/// 做的只是把优化开关推到顶。
pub fn release_plan() -> Vec<Setting> {
    vec![
        Setting {
            key: "opt-level",
            val: Val::Int(3),
            note: "最高优化级别",
        },
        Setting {
            key: "lto",
            val: Val::Str("fat"),
            note: "跨 crate 内联；会把 dev 里为省编译时间留下的 dyn 调用重新去虚化",
        },
        Setting {
            key: "codegen-units",
            val: Val::Int(1),
            note: "单 CGU，LLVM 能看到全程序；这是 release 性能的主开关",
        },
        Setting {
            key: "panic",
            val: Val::Str("abort"),
            note: "省 unwind 表，也让优化器少一堆 landing pad",
        },
        Setting {
            key: "strip",
            val: Val::Str("symbols"),
            note: "发布产物不留符号表",
        },
        Setting {
            key: "incremental",
            val: Val::Bool(false),
            note: "release 不需要增量，关掉以免污染指纹",
        },
    ]
}

// ---------------------------------------------------------------- 链接期：/DEBUG:NONE

/// MSVC 上 rustc 即使 `debug=false` 也照传 `/DEBUG`，于是照样生成 .pdb。
/// 实测 Tauri 项目这一刀省 179 MB（181MB → 0）。
pub const DEBUG_NONE_FLAGS: [&str; 2] = ["-C", "link-args=/DEBUG:NONE"];

/// 非 MSVC 平台不需要这个。
pub fn link_flags_for(msvc: bool) -> &'static [&'static str] {
    if msvc {
        &DEBUG_NONE_FLAGS
    } else {
        &[]
    }
}
