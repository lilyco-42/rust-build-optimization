#!/usr/bin/env python
# -*- coding: utf-8 -*-
"""最终测量：5 个变体 × (默认 / 去 PDB)"""
import os, shutil, subprocess, time, json

ROOT = os.path.dirname(os.path.abspath(__file__))
N = 3
NODBG = "-C debuginfo=0 -C link-args=/DEBUG:NONE"

VARIANTS = [
    ("A 现状 serde_json+thiserror", "lc_dep",        "lc-dep"),
    ("D no_std+serde_json(alloc)",  "lc_nostd_json", "lc-nostd-json"),
    ("B no_std+serde-json-core",    "lc_nostd",      "lc-nostd"),
    ("C 同B但去掉#![no_std]",        "lc_std",        "lc-std"),
    ("E no_std+serde 不开derive",    "lc_noderive",   "lc-noderive"),
]

def run(args, env=None):
    e = dict(os.environ); e.pop("BASH_ENV", None); e.pop("ENV", None)
    if env: e.update(env)
    return subprocess.run(args, cwd=ROOT, env=e, capture_output=True, text=True,
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

def measure(pkg, d, flags=None):
    tgt = os.path.join(ROOT, "target", pkg + ("_nd" if flags else ""))
    env = {"CARGO_TARGET_DIR": tgt}
    if flags: env["RUSTFLAGS"] = flags
    cold, incr, exe, tsz = 1e18, 1e18, 0, 0
    for i in range(N):
        shutil.rmtree(tgt, ignore_errors=True)
        ms, r = timed(["cargo", "build", "-p", pkg], env)
        if r.returncode != 0:
            print("FAIL %s\n%s" % (pkg, r.stderr[-1200:])); return None
        cold = min(cold, ms)
        src = os.path.join(ROOT, d, "src", "lib.rs")
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

out = {}
print("N=%d 每组取最小\n" % N)
print("%-30s %9s %9s %11s %13s" % ("变体", "cold ms", "incr ms", "exe B", "target B"))
print("-" * 76)
for label, pkg, d in VARIANTS:
    a = measure(pkg, d)
    b = measure(pkg, d, NODBG)
    out[pkg] = {"label": label, "default": a, "nodb": b}
    print("%-30s %9.0f %9.0f %11d %13d" % (label, *a))
    print("%-30s %9.0f %9.0f %11d %13d   [去PDB]" % ("", *b))
    print()
json.dump(out, open(os.path.join(ROOT, "result3.json"), "w"), indent=1)

base = out["lc_dep"]["default"]
print("=== 相对 A 组（默认 profile）===")
for label, pkg, _ in VARIANTS:
    c = out[pkg]["default"]
    print("  %-30s cold %5.0f%%  incr %5.0f%%  exe %5.0f%%  target %5.0f%%" % (
        label, (c[0]/base[0]-1)*100, (c[1]/base[1]-1)*100,
        (c[2]/base[2]-1)*100, (c[3]/base[3]-1)*100))
