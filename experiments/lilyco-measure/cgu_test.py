"""对比 dev profile 的 codegen-units：我配的 16 vs cargo 默认的 256。

背景：cargo 的 dev profile 默认 codegen-units = 256（为编译并行度），
      我此前写的优化配置把它设成 16（那是 release 档的值）。
      codegen-units 越小 = 单个 crate 内部 codegen 越少并行 = 编译越慢。
      本脚本用单变量实测验证这一点。

每个变体都：改配置 -> cargo clean -> 计时 cargo check -> 计时 cargo build -> 量体积
"""

import json
import os
import re
import subprocess
import sys
import time
from pathlib import Path

ROOT = Path("D:/Code/lilyco")
CARGO_TOML = ROOT / "Cargo.toml"
ENV = {k: v for k, v in os.environ.items() if k not in ("BASH_ENV", "ENV")}


def run(args, timeout=None):
    t = time.perf_counter()
    p = subprocess.run(args, cwd=ROOT, env=ENV, capture_output=True,
                       text=True, encoding="utf-8", errors="replace", timeout=timeout)
    return time.perf_counter() - t, p.returncode, (p.stdout or "") + (p.stderr or "")


def target_mb():
    p = ROOT / "target"
    if not p.is_dir():
        return 0.0
    return sum(f.stat().st_size for f in p.rglob("*") if f.is_file()) / 1048576


def set_cgu(n):
    """把 [profile.dev] 段里的 codegen-units 改成 n"""
    t = CARGO_TOML.read_text(encoding="utf-8")
    lines = t.splitlines()
    out, in_dev = [], False
    for ln in lines:
        if ln.strip() == "[profile.dev]":
            in_dev = True
            out.append(ln)
            continue
        if ln.startswith("[") and ln.strip() != "[profile.dev]":
            in_dev = False
        if in_dev and ln.strip().startswith("codegen-units"):
            out.append(f"codegen-units = {n}")
        else:
            out.append(ln)
    CARGO_TOML.write_text("\n".join(out) + "\n", encoding="utf-8")


def main():
    orig = CARGO_TOML.read_text(encoding="utf-8")
    results = []
    try:
        for label, cgu in (("A 我配的 codegen-units=16", 16),
                           ("B cargo 默认 codegen-units=256", 256)):
            print("=" * 74)
            print(label)
            print("=" * 74, flush=True)
            set_cgu(cgu)

            run(["cargo", "clean"])
            dt, rc, out = run(["cargo", "check"], timeout=3600)
            n = len(re.findall(r"^\s+Checking\s+", out, re.M))
            print(f"  cargo check 冷  {dt:6.1f}s  rc={rc}  检查 {n} 个 crate", flush=True)
            check_s = dt

            run(["cargo", "clean"])
            dt, rc, out = run(["cargo", "build"], timeout=3600)
            n = len(re.findall(r"^\s+Compiling\s+", out, re.M))
            print(f"  cargo build 冷  {dt:6.1f}s  rc={rc}  编译 {n} 个 crate", flush=True)
            build_s = dt
            mb = target_mb()
            print(f"  target/  {mb:.1f} MB", flush=True)

            results.append({"label": label, "codegen_units": cgu,
                            "check_cold_s": round(check_s, 1),
                            "build_cold_s": round(build_s, 1),
                            "target_mb": round(mb, 1)})
            print(flush=True)
    finally:
        CARGO_TOML.write_text(orig, encoding="utf-8")

    print("=" * 74)
    print("汇总")
    print("=" * 74)
    for r in results:
        print(f"  {r['label']:34s} check {r['check_cold_s']:6.1f}s  "
              f"build {r['build_cold_s']:6.1f}s  target {r['target_mb']:7.1f} MB")
    a, b = results[0], results[1]
    print()
    print(f"  codegen-units 256 相对 16：check {b['check_cold_s']-a['check_cold_s']:+.1f}s  "
          f"build {b['build_cold_s']-a['build_cold_s']:+.1f}s  "
          f"target {b['target_mb']-a['target_mb']:+.1f} MB")
    (ROOT / "_cgu.json").write_text(json.dumps(results, indent=2, ensure_ascii=False),
                                    encoding="utf-8")
    print("\n  已还原 Cargo.toml；结果写入 _cgu.json")
    return 0


if __name__ == "__main__":
    sys.exit(main())
