#!/usr/bin/env python
# -*- coding: utf-8 -*-
"""三方同口径终极测量：C/xmake vs 常规 Rust(std) vs 极限 Rust(no_std)

回答的问题：Rust 要"像 xmake C 那样快、那样小"，代价是什么。
每组 N=3 取最小。
"""
import os, shutil, subprocess, time, json

Q = "D:/Code/rust/question"
N = 3

def run(args, cwd, env=None):
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

def ms(fn):
    s = time.perf_counter(); fn(); return (time.perf_counter() - s) * 1000.0

# ── C / xmake ──────────────────────────────────────────────
def measure_c():
    d = os.path.join(Q, "ip-c")
    bd = os.path.join(d, "build")
    cold = incr = 1e18; exe = 0
    for i in range(N):
        shutil.rmtree(bd, ignore_errors=True)
        run(["xmake", "f", "-p", "windows", "-a", "x64", "--toolchain=msvc", "-y"], d)
        cold = min(cold, ms(lambda: run(["xmake", "-y"], d)))
        src = os.path.join(d, "src", "main.c")
        open(src, "a", encoding="utf-8").write("\n/* t%d */\n" % i)
        incr = min(incr, ms(lambda: run(["xmake", "-y"], d)))
        ls = open(src, encoding="utf-8").read().split("\n")
        while ls and ls[-1].strip() == "": ls.pop()
        if ls and ls[-1].startswith("/* t"): ls.pop()
        open(src, "w", encoding="utf-8").write("\n".join(ls) + "\n")
    for root, dirs, files in os.walk(bd):
        for f in files:
            if f.endswith(".exe"):
                p = os.path.join(root, f)
                if "release" in p: exe = max(exe, os.path.getsize(p))
    return dict(cold=cold, incr=incr, exe=exe, target=dirsize(bd))

# ── Rust ───────────────────────────────────────────────────
def measure_rust(name, extra=None):
    d = os.path.join(Q, name)
    tgt = os.path.join(d, "target")
    env = {}
    if extra: env["RUSTFLAGS"] = extra
    cold = incr = 1e18; exe = 0; tsz = 0
    for i in range(N):
        shutil.rmtree(tgt, ignore_errors=True)
        cold = min(cold, ms(lambda: run(["cargo", "build"], d, env)))
        src = os.path.join(d, "src", "main.rs")
        open(src, "a", encoding="utf-8").write("\n// t%d\n" % i)
        incr = min(incr, ms(lambda: run(["cargo", "build"], d, env)))
        ls = open(src, encoding="utf-8").read().split("\n")
        while ls and ls[-1].strip() == "": ls.pop()
        if ls and ls[-1].startswith("// t"): ls.pop()
        open(src, "w", encoding="utf-8").write("\n".join(ls) + "\n")
        p = os.path.join(tgt, "debug", name.replace("-", "_") + ".exe")
        if not os.path.exists(p):
            p = os.path.join(tgt, "debug", name + ".exe")
        if os.path.exists(p): exe = os.path.getsize(p)
        tsz = dirsize(tgt)
    return dict(cold=cold, incr=incr, exe=exe, target=tsz)

def measure_rust_release(name):
    d = os.path.join(Q, name)
    tgt = os.path.join(d, "target")
    shutil.rmtree(tgt, ignore_errors=True)
    run(["cargo", "build", "--release"], d)
    for cand in (name.replace("-", "_"), name):
        p = os.path.join(tgt, "release", cand + ".exe")
        if os.path.exists(p): return os.path.getsize(p)
    return 0

rows = []
print("N=%d 取最小\n" % N)
print("=== dev/debug 档（日常开发循环）===")
print("%-34s %9s %9s %11s %13s" % ("方案", "cold ms", "incr ms", "exe B", "target B"))
print("-" * 82)
c = measure_c(); rows.append(("C / xmake (release)", c))
print("%-34s %9.0f %9.0f %11d %13d" % ("C / xmake (release)", c["cold"], c["incr"], c["exe"], c["target"]))

s = measure_rust("ip-std", "-C debuginfo=0 -C link-args=/DEBUG:NONE"); rows.append(("Rust 常规(std, 零依赖)", s))
print("%-34s %9.0f %9.0f %11d %13d" % ("Rust 常规(std, 零依赖)", s["cold"], s["incr"], s["exe"], s["target"]))

b = measure_rust("ip-bare"); rows.append(("Rust 极限(no_std, 零依赖)", b))
print("%-34s %9.0f %9.0f %11d %13d" % ("Rust 极限(no_std, 零依赖)", b["cold"], b["incr"], b["exe"], b["target"]))

print()
print("=== release 档（交付体积）===")
cr = 0
for root, dirs, files in os.walk(os.path.join(Q, "ip-c", "build")):
    for f in files:
        if f.endswith(".exe") and "release" in root: cr = max(cr, os.path.getsize(os.path.join(root, f)))
sr = measure_rust_release("ip-std")
br = measure_rust_release("ip-bare")
print("%-34s %11s" % ("方案", "exe B"))
print("-" * 48)
print("%-34s %11d" % ("C / xmake release", cr))
print("%-34s %11d" % ("Rust 常规 release (std)", sr))
print("%-34s %11d" % ("Rust 极限 release (no_std)", br))

json.dump({"dev": rows, "release": {"c": cr, "std": sr, "bare": br}},
          open(os.path.join(Q, "final.json"), "w"), indent=1)

# 相对 C
print()
print("=== 相对 C/xmake 的比例（越小越好）===")
print("%-34s %9s %9s %9s %9s" % ("方案", "cold", "incr", "exe(dev)", "target"))
print("-" * 78)
for label, r in rows:
    print("%-34s %8.2fx %8.2fx %8.2fx %8.2fx" % (
        label, r["cold"]/c["cold"], r["incr"]/c["incr"], r["exe"]/c["exe"], r["target"]/c["target"]))
print("%-34s %8s %8s %8.2fx %8.2fx" % ("Rust 常规 release", "-", "-", sr/cr if cr else 0, 0))
print("%-34s %8s %8s %8.2fx %8.2fx" % ("Rust 极限 release", "-", "-", br/cr if cr else 0, 0))
