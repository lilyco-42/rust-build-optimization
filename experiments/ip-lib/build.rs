//! 替代 .cargo/config.toml 的链接参数声明。
//!
//! 用 build.rs 的 `cargo:rustc-link-arg` 而不是 `[build] rustflags`，原因：
//! 全局 rustflags 也会命中 proc-macro 宿主的 DLL 构建，导致
//!   `msvcrt.lib(exe_main.obj) : error LNK2019: 无法解析的外部符号 main`
//! 而 rustc-link-arg 只作用于本 package 的最终产物。

use std::path::PathBuf;

fn main() {
    // MSVC 不会为 no_main 自动推导子系统与入口点。
    println!("cargo:rustc-link-arg=/ENTRY:mainCRTStartup");
    println!("cargo:rustc-link-arg=/SUBSYSTEM:CONSOLE");

    // rlibc 的 mem* 符号必须用 /WHOLEARCHIVE 强制整包拉入：
    // MSVC 归档按需取成员，libcore 引用 memset 时 librlibc 尚未被扫描到。
    // 路径不硬编码：从 OUT_DIR 反推 target/<profile>/deps。
    //   OUT_DIR = <target>/<profile>/build/<pkg>-<hash>/out
    //   ancestors: 0=out  1=<pkg>-<hash>  2=build  3=<profile>  4=<target>
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let profile = out
        .ancestors()
        .nth(3)
        .unwrap()
        .file_name()
        .unwrap()
        .to_os_string();
    let target = out.ancestors().nth(4).unwrap().to_path_buf();
    let deps = target.join(&profile).join("deps");

    // 只取字典序最大者，避免 /WHOLEARCHIVE 重复引入同名符号 → LNK2005。
    let mut best_name = String::new();
    let mut best_path: Option<PathBuf> = None;
    for e in std::fs::read_dir(&deps).into_iter().flatten().flatten() {
        let s = e.file_name().to_string_lossy().to_string();
        if s.starts_with("librlibc-") && s.ends_with(".rlib") && s > best_name {
            best_name = s;
            best_path = Some(e.path());
        }
    }
    match best_path {
        Some(p) => {
            // link.exe 认正斜杠，避免转义问题。
            let wp = p.to_string_lossy().replace(char::from(92), "/");
            println!("cargo:rustc-link-arg=/WHOLEARCHIVE:{wp}");
        }
        None => println!("cargo:warning=rlibc rlib not found in {}", deps.display()),
    }

    println!("cargo:rerun-if-changed=build.rs");
}
