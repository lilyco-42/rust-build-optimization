"""第二轮：解释第一轮的两个异常 + 补测并行前端。

异常1：nightly 基准 target 697.9 MB vs stable 935.3 MB（同配置差 237 MB）—— 差在哪？
异常2：cranelift 的 check 也变快了（47.2→37.5s）—— 机制是什么？
补测 ：--jobs-frontend 的正确写法
"""
import json
import os
import re
import subprocess
import time
from pathlib import Path

ROOT = Path("D:/Code/lilyco")
CFG_DIR = ROOT / ".cargo"
CFG = CFG_DIR / "config.toml"
ENV_BASE = {k: v for k, v in os.environ.items() if k not in ("BASH_ENV", "ENV")}
SEP = "\x1f"
NO_DEBUG = "-Clink-args=/DEBUG:NONE"
CFG_NO_RUSTFLAGS = '[build]\n# rustc-wrapper = "sccache"\n'


def run(args, env, timeout=1800):
    t = time.perf_counter()
    p = subprocess.run(args, cwd=ROOT, env=env, capture_output=True,
                       text=True, encoding="utf-8", errors="replace", timeout=timeout)
    return time.perf_counter() - t, p.returncode, (p.stdout or "") + (p.stderr or "")


def breakdown():
    """拆 target/ 构成。"""
    t = ROOT / "target" / "debug"
    out = {}
    for name in ("deps", "incremental", "build"):
        d = t / name
        out[name] = round(sum(f.stat().st_size for f in d.rglob("*") if f.is_file()) / 1048576, 1) if d.is_dir() else 0.0
    out["total"] = round(sum(f.stat().st_size for f in (ROOT / "target").rglob("*")
                             if f.is_file()) / 1048576, 1)
    ext = {}
    for e in ("rlib", "rmeta", "pdb", "obj", "lib"):
        ext[e] = round(sum(f.stat().st_size for f in (ROOT / "target").rglob(f"*.{e}")
                           if f.is_file()) / 1048576, 1)
    out.update(ext)
    fc = ROOT / "target" / "debug" / "incremental"
    out["inc_dirs"] = len([x for x in fc.iterdir()]) if fc.is_dir() else 0
    return out


def measure(cargo, env, do_check, do_build, label):
    run(["cargo", "clean"], env)
    res = {"label": label}
    if do_check:
        dt, rc, out = run(cargo + ["check"], env)
        n_proc = len(re.findall(r"^\s+Compiling\s+", out, re.M))
        n_chk = len(re.findall(r"^\s+Checking\s+", out, re.M))
        res["check_s"] = round(dt, 1) if rc == 0 else None
        res["check_compiling_lines"] = n_proc   # ← 这些就是 check 期间仍要做 codegen 的
        res["check_checking_lines"] = n_chk
        print(f"  {label}: check {dt:6.1f}s rc={rc}  Compiling {n_proc} 行 / Checking {n_chk} 行", flush=True)
        if rc != 0:
            print("     " + "\n     ".join(out.splitlines()[-4:]), flush=True)
    if do_build:
        if do_check:
            run(["cargo", "clean"], env)
        dt, rc, out = run(cargo + ["build"], env)
        res["build_s"] = round(dt, 1) if rc == 0 else None
        print(f"  {label}: build {dt:6.1f}s rc={rc}", flush=True)
        if rc != 0:
            print("     " + "\n     ".join(out.splitlines()[-4:]), flush=True)
        res.update(breakdown())
        print(f"     deps {res['deps']} / incremental {res['incremental']} / build {res['build']}"
              f"  | rlib {res['rlib']} rmeta {res['rmeta']} pdb {res['pdb']}"
              f"  | inc_dirs {res['inc_dirs']}", flush=True)
    return res


def main():
    orig = CFG.read_text(encoding="utf-8") if CFG.exists() else None
    results = []
    try:
        CFG_DIR.mkdir(exist_ok=True)
        CFG.write_text(CFG_NO_RUSTFLAGS, encoding="utf-8")

        print("=" * 78, flush=True)
        print("补测 D · 并行前端（正确旗标 --jobs-frontend）", flush=True)
        print("=" * 78, flush=True)
        print("  [D-control] nightly 无前端参数", flush=True)
        results.append(measure(["cargo", "+nightly"],
                               {**ENV_BASE, "CARGO_ENCODED_RUSTFLAGS": NO_DEBUG},
                               True, False, "C2 nightly 对照"))
        print("  [D] -Zunstable-options --jobs-frontend=8", flush=True)
        results.append(measure(["cargo", "+nightly"],
                               {**ENV_BASE, "CARGO_ENCODED_RUSTFLAGS":
                                SEP.join([NO_DEBUG, "-Zunstable-options", "--jobs-frontend=8"])},
                               True, False, "D2 nightly + 并行前端8"))

        print(flush=True)
        print("=" * 78, flush=True)
        print("异常复核 · nightly 基准 target 构成（对照 stable 935.3 MB）", flush=True)
        print("=" * 78, flush=True)
        r = measure(["cargo", "+nightly"], {**ENV_BASE, "CARGO_ENCODED_RUSTFLAGS": NO_DEBUG},
                    False, True, "C3 nightly 基准")
        c3 = dict(r)
        results.append(r)

        print(flush=True)
        print("=" * 78, flush=True)
        print("异常复核 · cranelift target 构成（为何 +274 MB）", flush=True)
        print("=" * 78, flush=True)
        r = measure(["cargo", "+nightly"],
                    {**ENV_BASE, "CARGO_ENCODED_RUSTFLAGS":
                     SEP.join([NO_DEBUG, "-Zcodegen-backend=cranelift"])},
                    False, True, "E2 cranelift")
        results.append(r)
    finally:
        if orig is not None:
            CFG.write_text(orig, encoding="utf-8")

    print(flush=True)
    print("=" * 78, flush=True)
    print("汇总", flush=True)
    print("=" * 78, flush=True)
    for r in results:
        print(json.dumps(r, ensure_ascii=False), flush=True)
    (ROOT / "_speed_cards2.json").write_text(
        json.dumps(results, indent=2, ensure_ascii=False), encoding="utf-8")
    print("\n  .cargo/config.toml 已还原", flush=True)


if __name__ == "__main__":
    main()
