#!/usr/bin/env python
# -*- coding: utf-8 -*-
"""lilyco no_std 对照实验测量器

四组变体，每组测：
  cold    —— 删光 target 后的完整构建（含所有依赖）
  incr    —— 只 touch 一个源文件后的增量构建
  exe     —— 可执行文件字节数
  target  —— 整个 target 目录字节数（PDB 在这里面）

每组取 N 次最小值，消除抖动。
"""
import os, shutil, subprocess, time, sys, json

ROOT = os.path.dirname(os.path.abspath(__file__))
N = 3

VARIANTS = [
    ("A 现状 std+serde_json+thiserror", "lc_dep",        "lc-dep"),
    ("D no_std+serde_json(alloc)",      "lc_nostd_json", "lc-nostd-json"),
    ("B no_std+serde-json-core",        "lc_nostd",      "lc-nostd"),
    ("C 同B但去掉 no_std",              "lc_std",        "lc-std"),
]

def run(args, env=None, cwd=ROOT):
    e = dict(os.environ)
    e.pop("BASH_ENV", None); e.pop("ENV", None)
    if env: e.update(env)
    return subprocess.run(args, cwd=cwd, env=e, capture_output=True, text=True,
                          encoding="utf-8", errors="replace")

def dirsize(p):
    t = 0
    for r, d, f in os.walk(p):
        for n in f:
            try: t += os.path.getsize(os.path.join(r, n))
            except OSError: pass
    return t

def timed(args, env=None):
    s = time.perf_counter()
    r = run(args, env)
    return (time.perf_counter() - s) * 1000.0, r

def measure(pkg, d, extra_rustflags=None):
    tgt = os.path.join(ROOT, "target", pkg)
    env = {"CARGO_TARGET_DIR": tgt}
    if extra_rustflags:
        env["RUSTFLAGS"] = extra_rustflags

    best_cold, best_incr, exe, tsz = 1e18, 1e18, 0, 0
    for i in range(N):
        shutil.rmtree(tgt, ignore_errors=True)
        ms, r = timed(["cargo", "build", "-p", pkg], env)
        if r.returncode != 0:
            print("   BUILD FAIL\n" + r.stderr[-1500:]); return None
        best_cold = min(best_cold, ms)

        # 增量：只改一个源文件
        src = os.path.join(ROOT, d, "src", "lib.rs")
        with open(src, "a", encoding="utf-8") as fh:
            fh.write("\n// touch %d\n" % i)
        ms2, r2 = timed(["cargo", "build", "-p", pkg], env)
        best_incr = min(best_incr, ms2)
        with open(src, "r", encoding="utf-8") as fh:
            lines = fh.read().split("\n")
        while lines and lines[-1].strip() == "": lines.pop()
        if lines and lines[-1].startswith("// touch"): lines.pop()
        open(src, "w", encoding="utf-8").write("\n".join(lines) + "\n")

        exe_p = os.path.join(tgt, "debug", pkg + ".exe")
        if os.path.exists(exe_p): exe = os.path.getsize(exe_p)
        tsz = dirsize(tgt)
    return dict(cold=best_cold, incr=best_incr, exe=exe, target=tsz)

def main():
    rows = []
    print("N=%d（每组取最小）\n" % N)
    print("%-34s %10s %10s %12s %14s" % ("变体", "cold ms", "incr ms", "exe B", "target B"))
    print("-" * 84)
    for label, pkg, d in VARIANTS:
        m = measure(pkg, d)
        if not m: continue
        rows.append((label, pkg, m))
        print("%-34s %10.0f %10.0f %12d %14d" % (label, m["cold"], m["incr"], m["exe"], m["target"]))
    print()
    json.dump([{"label": l, "pkg": p, **m} for l, p, m in rows],
              open(os.path.join(ROOT, "result.json"), "w"), indent=1)

    # 依赖树规模
    print("=== 依赖单元数（cargo tree）===")
    for label, pkg, _ in VARIANTS:
        r = run(["cargo", "tree", "-p", pkg, "--no-dedupe", "-e", "normal"])
        n = len([x for x in r.stdout.split("\n") if x.strip()])
        print("  %-34s %d" % (label, n))

if __name__ == "__main__":
    main()
