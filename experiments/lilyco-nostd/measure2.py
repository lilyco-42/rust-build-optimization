#!/usr/bin/env python
# -*- coding: utf-8 -*-
"""第二轮：① PDB/debuginfo 杠杆（零代码改动） ② 无 derive 变体的天花板"""
import os, shutil, subprocess, time

ROOT = os.path.dirname(os.path.abspath(__file__))
N = 3
NODBG = "-C debuginfo=0 -C link-args=/DEBUG:NONE"

VARIANTS = [
    ("A 现状",                  "lc_dep",        "lc-dep"),
    ("D no_std+serde_json",     "lc_nostd_json", "lc-nostd-json"),
    ("B no_std+serde-json-core","lc_nostd",      "lc-nostd"),
]

def run(args, env=None, cwd=ROOT):
    e = dict(os.environ); e.pop("BASH_ENV", None); e.pop("ENV", None)
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

def timed(args, env):
    s = time.perf_counter(); r = run(args, env)
    return (time.perf_counter() - s) * 1000.0, r

def measure(pkg, flags=None):
    tgt = os.path.join(ROOT, "target", pkg + ("_nd" if flags else ""))
    env = {"CARGO_TARGET_DIR": tgt}
    if flags: env["RUSTFLAGS"] = flags
    cold, incr, exe, tsz = 1e18, 1e18, 0, 0
    for i in range(N):
        shutil.rmtree(tgt, ignore_errors=True)
        ms, r = timed(["cargo", "build", "-p", pkg], env)
        if r.returncode != 0:
            print("FAIL\n" + r.stderr[-1200:]); return None
        cold = min(cold, ms)
        src = os.path.join(ROOT, pkg.replace("_", "-"), "src", "lib.rs")
        if not os.path.exists(src):
            src = os.path.join(ROOT, {"lc_dep": "lc-dep", "lc_nostd_json": "lc-nostd-json",
                                      "lc_nostd": "lc-nostd"}[pkg], "src", "lib.rs")
        open(src, "a", encoding="utf-8").write("\n// t%d\n" % i)
        ms2, _ = timed(["cargo", "build", "-p", pkg], env)
        incr = min(incr, ms2)
        ls = open(src, encoding="utf-8").read().split("\n")
        while ls and ls[-1].strip() == "": ls.pop()
        if ls and ls[-1].startswith("// t"): ls.pop()
        open(src, "w", encoding="utf-8").write("\n".join(ls) + "\n")
        p = os.path.join(tgt, "debug", pkg + ".exe")
        if os.path.exists(p): exe = os.path.getsize(p)
        tsz = dirsize(tgt)
    return cold, incr, exe, tsz

print("=== 加 -C debuginfo=0 /DEBUG:NONE（零代码改动）===\n")
print("%-28s %10s %10s %12s %14s" % ("变体", "cold ms", "incr ms", "exe B", "target B"))
print("-" * 78)
base = {}
for label, pkg, d in VARIANTS:
    c, i, e, t = measure(pkg)
    c2, i2, e2, t2 = measure(pkg, NODBG)
    base[pkg] = (c, i, e, t)
    print("%-28s %10.0f %10.0f %12d %14d   (默认)" % (label, c, i, e, t))
    print("%-28s %10.0f %10.0f %12d %14d   (去 PDB)" % ("", c2, i2, e2, t2))
    print("%-28s %10s %10s %12s %13.1fx" % ("  → 变化", "%.0f%%" % ((c2/c-1)*100),
          "%.0f%%" % ((i2/i-1)*100), "%.0f%%" % ((e2/e-1)*100), t/t2))
    print()
