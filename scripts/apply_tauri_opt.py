# -*- coding: utf-8 -*-
"""
把 Tauri 体积优化配置应用到真实项目。

幂等：已有 [profile.dev] 的项目会跳过。
安全：每个被修改的文件先备份为 *.bak-before-opt
"""
import io, os, re, shutil, datetime, sys

STAMP = datetime.datetime.now().strftime("%Y%m%d-%H%M%S")

PROFILE_BLOCK = '''
# ============================================================================
# 体积优化（实测 target 4,135 MB -> 953 MB，见 TAURI_1GB_ACHIEVED.md）
# 追加于 {stamp}
# ============================================================================

[profile.dev]
opt-level = 1
debug = false                # 需要断点调试时改回 "line-tables-only"
incremental = true           # 改 false 再省 64 MB，但改一行会重编整个 crate
codegen-units = 16
strip = "debuginfo"
panic = "abort"

# 🔴 依赖用 opt-level="z" 压 rlib —— 实测省 113 MB
[profile.dev.package."*"]
opt-level = "z"
debug = false

[profile.release]
opt-level = "z"
lto = "fat"
codegen-units = 1
panic = "abort"
strip = "symbols"
incremental = false
overflow-checks = false
debug-assertions = false
'''.format(stamp=STAMP)

# /DEBUG:NONE 让 MSVC 不生成 PDB（实测省 179 MB）；sccache 让冷重建 113s -> 61.5s
CARGO_CONFIG = '''# 体积 / 速度优化 —— 生成于 {stamp}
#
# /DEBUG:NONE  : MSVC 链接器不生成 PDB（实测 target 省 179 MB）
#                代价：无法用 WinDbg / VS 断点调试；panic 回溯不受影响
#                需要断点调试时，注释掉下面这行即可
# rustc-wrapper: sccache，换分支 / cargo clean 后重建实测 113s -> 61.5s

[target.x86_64-pc-windows-msvc]
rustflags = ["-C", "link-args=/DEBUG:NONE"]

[build]
rustc-wrapper = "sccache"
'''.format(stamp=STAMP)


def read(p):
    return io.open(p, encoding="utf-8").read()


def write(p, s):
    io.open(p, "w", encoding="utf-8", newline="\n").write(s)


def backup(p):
    if os.path.exists(p):
        b = p + ".bak-before-opt"
        if not os.path.exists(b):
            shutil.copy2(p, b)
            return b
        # 已备份过就不覆盖
        return b
    return None


def add_profile(cargo_toml, label):
    """向 Cargo.toml 追加 profile 段（幂等）"""
    if not os.path.exists(cargo_toml):
        return f"  [跳过] {label}: 文件不存在"
    s = read(cargo_toml)
    if "[profile.dev]" in s:
        return f"  [跳过] {label}: 已有 [profile.dev]"
    backup(cargo_toml)
    if not s.endswith("\n"):
        s += "\n"
    write(cargo_toml, s + PROFILE_BLOCK)
    return f"  [完成] {label}: 已追加 profile"


def strip_crate_type(cargo_toml, label):
    """把 crate-type 从三件套改成只留 rlib（桌面端不需要 staticlib/cdylib）"""
    s = read(cargo_toml)
    m = re.search(r'crate-type\s*=\s*\[([^\]]*)\]', s)
    if not m:
        return f"  [跳过] {label}: 无 crate-type"
    cur = m.group(1)
    if "staticlib" not in cur and "cdylib" not in cur:
        return f"  [跳过] {label}: crate-type 已精简"
    backup(cargo_toml)
    s = s[:m.start()] + 'crate-type = ["rlib"]' + s[m.end():]
    write(cargo_toml, s)
    return f"  [完成] {label}: crate-type -> [\"rlib\"]（省 ~183 MB）"


def add_cargo_config(dir_path, label):
    """写 .cargo/config.toml"""
    if not os.path.isdir(dir_path):
        return f"  [跳过] {label}: 目录不存在"
    cd = os.path.join(dir_path, ".cargo")
    os.makedirs(cd, exist_ok=True)
    cfg = os.path.join(cd, "config.toml")
    if os.path.exists(cfg):
        s = read(cfg)
        if "DEBUG:NONE" in s:
            return f"  [跳过] {label}: 已有 /DEBUG:NONE"
        backup(cfg)
        write(cfg, s.rstrip() + "\n\n" + CARGO_CONFIG)
        return f"  [完成] {label}: 已追加到现有 config.toml"
    write(cfg, CARGO_CONFIG)
    return f"  [完成] {label}: 新建 .cargo/config.toml"


TARGETS = [
    dict(label="colibri/desktop/src-tauri",
         dir="D:/Code/colibri/desktop/src-tauri",
         profile_in="D:/Code/colibri/desktop/src-tauri/Cargo.toml",
         crate_type_in="D:/Code/colibri/desktop/src-tauri/Cargo.toml",
         cfg_dir="D:/Code/colibri/desktop/src-tauri"),
    dict(label="lilyco (workspace 根)",
         dir="D:/Code/lilyco",
         profile_in="D:/Code/lilyco/Cargo.toml",
         crate_type_in="D:/Code/lilyco/lilyco-tauri/Cargo.toml",
         cfg_dir="D:/Code/lilyco/lilyco-tauri"),
    dict(label="simul-demo/app/src-tauri",
         dir="D:/Code/simul-demo/app/src-tauri",
         profile_in="D:/Code/simul-demo/app/src-tauri/Cargo.toml",
         crate_type_in=None,
         cfg_dir="D:/Code/simul-demo/app/src-tauri"),
    dict(label="new-api/tauri",
         dir="D:/Code/new-api/tauri",
         profile_in="D:/Code/new-api/tauri/Cargo.toml",
         crate_type_in=None,
         cfg_dir="D:/Code/new-api/tauri"),
]

print("=" * 78)
print("应用 Tauri 体积优化配置")
print("=" * 78)

for t in TARGETS:
    print(f"\n[{t['label']}]")
    if not os.path.isdir(t["dir"]):
        print(f"  [跳过] 目录不存在: {t['dir']}")
        continue
    print(add_profile(t["profile_in"], "profile"))
    if t["crate_type_in"]:
        print(strip_crate_type(t["crate_type_in"], "crate-type"))
    print(add_cargo_config(t["cfg_dir"], ".cargo/config"))

print()
print("=" * 78)
print("完成。备份文件后缀：.bak-before-opt")
print("=" * 78)
