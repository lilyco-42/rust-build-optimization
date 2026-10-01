"""测量 lilyco 的真实构建成本。

用途：第 0 层「先量，别猜」—— lilyco 此前从未构建过，
之前所有数字都是从依赖图推算的，这里补上实测。

测量项：
  A. cargo check 冷（默认成员）—— 日常内环最常跑的命令
  B. cargo build 冷（默认成员）—— 真正产出二进制
  C. cargo build 增量（改一行）—— 内环真实体感
  D. target/ 构成拆解（rlib / rmeta / pdb / .lib / build / incremental）

每步都从干净状态开始（cargo clean，不用 rmtree —— 那会被安全策略拦）。
"""

import json
import os
import re
import subprocess
import sys
import time
from pathlib import Path

ROOT = Path("D:/Code/lilyco")
ENV = {k: v for k, v in os.environ.items() if k not in ("BASH_ENV", "ENV")}


def run(args, cwd=ROOT, timeout=None):
    """跑一条命令，返回 (耗时秒, 返回码, 输出尾部)"""
    t0 = time.perf_counter()
    p = subprocess.run(args, cwd=cwd, env=ENV, capture_output=True,
                       text=True, encoding="utf-8", errors="replace", timeout=timeout)
    dt = time.perf_counter() - t0
    out = (p.stdout or "") + (p.stderr or "")
    return dt, p.returncode, out


def dir_mb(path, pattern=None):
    """统计目录里匹配文件的总 MB"""
    path = Path(path)
    if not path.is_dir():
        return 0.0
    total = 0
    for e in os.scandir(path):
        pass
    # 用 os.walk 按后缀聚合
    agg = {}
    for dirpath, _, files in os.walk(path):
        for f in files:
            fp = Path(dirpath) / f
            try:
                sz = fp.stat(follow_symlinks=False).st_size
            except OSError:
                continue
            key = fp.suffix.lower() if pattern is None else None
            agg[key] = agg.get(key, 0) + sz
            agg["__total__"] = agg.get("__total__", 0) + sz
    if pattern:
        return agg.get(pattern, 0) / 1048576
    return agg


def count_pkgs(out):
    m = re.findall(r"^\s+Compiling\s+", out, re.M)
    return len(m)


def main():
    res = {}

    # ---------- A. cargo check 冷 ----------
    print("=" * 78)
    print("A. cargo check 冷构建（默认成员）")
    print("=" * 78, flush=True)
    run(["cargo", "clean"])
    dt, rc, out = run(["cargo", "check", "--timings"], timeout=3600)
    print(f"   耗时 {dt:.1f}s  rc={rc}  编译 {count_pkgs(out)} 个 crate")
    res["check_cold_s"] = round(dt, 1)
    res["check_cold_rc"] = rc
    if rc != 0:
        print("   ⚠️ 失败，输出尾部：")
        print("\n".join(out.splitlines()[-25:]))
        res["check_fail_tail"] = out.splitlines()[-25:]
    (ROOT / "check-cold.log").write_text(out, encoding="utf-8")

    # ---------- B. cargo build 冷 ----------
    print()
    print("=" * 78)
    print("B. cargo build 冷构建（默认成员）")
    print("=" * 78, flush=True)
    run(["cargo", "clean"])
    dt, rc, out = run(["cargo", "build"], timeout=7200)
    print(f"   耗时 {dt:.1f}s  rc={rc}  编译 {count_pkgs(out)} 个 crate")
    res["build_cold_s"] = round(dt, 1)
    res["build_cold_rc"] = rc
    res["build_cold_crates"] = count_pkgs(out)
    (ROOT / "build-cold.log").write_text(out, encoding="utf-8")
    if rc != 0:
        print("   ⚠️ 失败，输出尾部：")
        print("\n".join(out.splitlines()[-30:]))

    # ---------- C. target 构成 ----------
    print()
    print("=" * 78)
    print("C. target/ 构成拆解")
    print("=" * 78)
    tgt = ROOT / "target"
    agg = dir_mb(tgt)
    for suf in (".rlib", ".rmeta", ".pdb", ".lib", ".d", ".o"):
        mb = agg.get(suf, 0) / 1048576
        if mb > 0.05:
            print(f"   {suf:8s} {mb:9.1f} MB")
    print(f"   {'总计':8s} {agg.get('__total__', 0)/1048576:9.1f} MB")
    res["target_total_mb"] = round(agg.get("__total__", 0) / 1048576, 1)
    for suf in (".rlib", ".rmeta", ".pdb", ".lib"):
        res[f"target_{suf.strip('.')}_mb"] = round(agg.get(suf, 0) / 1048576, 1)

    for sub in ("debug/build", "debug/incremental", "debug/deps"):
        p = tgt / sub
        if p.is_dir():
            s = sum(f.stat().st_size for f in p.rglob("*") if f.is_file())
            print(f"   {sub:18s} {s/1048576:7.1f} MB")
            res[f"target_{sub.replace('/', '_')}_mb"] = round(s / 1048576, 1)

    # ---------- D. 增量构建 ----------
    print()
    print("=" * 78)
    print("D. 增量构建（改一行 lilyco-core 的源码）")
    print("=" * 78, flush=True)
    core_lib = ROOT / "lilyco-core" / "src" / "lib.rs"
    if core_lib.is_file():
        orig = core_lib.read_text(encoding="utf-8")
        try:
            core_lib.write_text(orig + "\n// probe\n", encoding="utf-8")
            dt2, rc2, out2 = run(["cargo", "build"], timeout=1800)
            print(f"   cargo build 增量 {dt2:.1f}s  rc={rc2}")
            dt3, rc3, out3 = run(["cargo", "check"], timeout=1800)
            print(f"   cargo check 增量 {dt3:.1f}s  rc={rc3}")
            res["build_incr_s"] = round(dt2, 1)
            res["check_incr_s"] = round(dt3, 1)
        finally:
            core_lib.write_text(orig, encoding="utf-8")
    else:
        print(f"   ⚠️ 找不到 {core_lib}")

    (ROOT / "measure-lilyco.json").write_text(
        json.dumps(res, indent=2, ensure_ascii=False), encoding="utf-8")
    print()
    print("结果已写入 measure-lilyco.json")
    return 0


if __name__ == "__main__":
    sys.exit(main())
