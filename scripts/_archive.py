"""把本次 Rust 编译/体积优化的全部产物归档成一个新仓库。

源：D:/Code/rust/question
目标：D:/Code/rust-build-optimization

排除：target/、.git/、__pycache__、*.pdb、*.exe、*.obj、*.lib
（全是可重新生成的构建产物，且体积巨大）

幂等：已存在则覆盖同名文件，不删除目标里已有的其他文件。
"""

import os
import shutil
import sys
from pathlib import Path

SRC = Path("D:/Code/rust/question")
DST = Path("D:/Code/rust-build-optimization")

EXCLUDE_DIRS = {"target", ".git", "__pycache__", ".cargo-lock", "node_modules"}
EXCLUDE_EXTS = {".pdb", ".exe", ".obj", ".lib", ".dll", ".rlib", ".rmeta", ".exp"}

# 显式排除：不属于「实验记录」的东西
#   lilyco-src 是用户真实产品的完整源码（24 个 crate，含嵌套 .git），
#   它是「被测量的对象」而不是「实验本身」—— 不进归档、不推 GitHub。
EXCLUDE_PATHS = {"lilyco-src"}

# 报告类 → docs/
DOCS = [
    "XMAKE_VS_RUST_PIPELINE.md",
    "TAURI_DEV_EFFICIENCY_PLAN.md",
    "TAURI_1GB_ACHIEVED.md",
    "RUST_NOSTD_VERDICT.md",
    "NO_STD_LIBS.md",
    "NOSTD_LIB_REPLACEMENT_VERDICT.md",
    "CAN_MATCH_C.md",
    "TUI_NOSTD_CORRECTION.md",
    "RUST_FAST_SMALL.md",
]

# 脚本类 → scripts/
SCRIPTS = [
    "apply_tauri_opt.py",
    "measure_all.py",
    "measure_final.py",
    "measure_one.py",
    "extract_session.py",
]

copied = []
skipped = []
errors = []


def should_skip(name: str) -> bool:
    if name in EXCLUDE_DIRS or name in EXCLUDE_PATHS:
        return True
    # target-sc / target-nostd 这类自定义 CARGO_TARGET_DIR 同样排除
    if name.startswith("target"):
        return True
    return Path(name).suffix.lower() in EXCLUDE_EXTS


def copy_tree(src: Path, dst: Path):
    """递归复制，按规则跳过。返回 (文件数, 跳过数)"""
    n_ok = n_skip = 0
    dst.mkdir(parents=True, exist_ok=True)
    try:
        entries = list(os.scandir(src))
    except OSError as e:
        errors.append(f"{src}: {e}")
        return 0, 0
    for e in entries:
        if should_skip(e.name):
            n_skip += 1
            skipped.append(str(Path(e.path)))
            continue
        s, d = Path(e.path), dst / e.name
        if e.is_dir(follow_symlinks=False):
            a, b = copy_tree(s, d)
            n_ok += a
            n_skip += b
        elif e.is_file(follow_symlinks=False):
            try:
                shutil.copy2(s, d)
                n_ok += 1
                copied.append(str(d))
            except OSError as err:
                errors.append(f"{s}: {err}")
        else:
            n_skip += 1
    return n_ok, n_skip


def main():
    if not SRC.is_dir():
        print(f"源目录不存在: {SRC}")
        return 1

    DST.mkdir(parents=True, exist_ok=True)

    # 1) docs/
    docs_dir = DST / "docs"
    docs_dir.mkdir(exist_ok=True)
    for name in DOCS:
        s = SRC / name
        if s.is_file():
            shutil.copy2(s, docs_dir / name)
            copied.append(str(docs_dir / name))
        else:
            errors.append(f"报告缺失: {s}")

    # 2) configs/  ← 来自 tauri-probe/_solution
    sol = SRC / "tauri-probe" / "_solution"
    cfg_dir = DST / "configs"
    cfg_dir.mkdir(exist_ok=True)
    if sol.is_dir():
        for e in os.scandir(sol):
            if e.is_file() and not should_skip(e.name):
                shutil.copy2(e.path, cfg_dir / e.name)
                copied.append(str(cfg_dir / e.name))

    # 3) scripts/
    scr_dir = DST / "scripts"
    scr_dir.mkdir(exist_ok=True)
    for name in SCRIPTS:
        s = SRC / name
        if s.is_file():
            shutil.copy2(s, scr_dir / name)
            copied.append(str(scr_dir / name))

    # 4) experiments/  ← 其余全部（含 tauri-probe 和所有 ip-* 实验工程）
    exp_dir = DST / "experiments"
    exp_dir.mkdir(exist_ok=True)
    for e in sorted(os.scandir(SRC), key=lambda x: x.name):
        if should_skip(e.name):
            continue
        if e.is_file() and (e.name.endswith(".md") or e.name in SCRIPTS):
            continue  # 已归到 docs/ 或 scripts/
        if e.is_dir():
            copy_tree(Path(e.path), exp_dir / e.name)
        elif e.is_file():
            shutil.copy2(e.path, exp_dir / e.name)
            copied.append(str(exp_dir / e.name))

    print(f"复制文件 {len(copied)} 个")
    print(f"跳过 {len(skipped)} 项（target/构建产物）")
    if errors:
        print(f"\n错误 {len(errors)} 条：")
        for x in errors[:20]:
            print("  " + x)
    return 0


if __name__ == "__main__":
    sys.exit(main())
