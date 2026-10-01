# Tauri 真实项目实测 v2：debug 速度 vs 体积
# 边跑边落盘，单步失败不中断
import os, subprocess, time, shutil, json

ROOT = r"D:\Code\rust\question\tauri-probe"
CARGO_BIN = r"D:\app\scoop\apps\rustup\current\.cargo\bin"
OUT = os.path.join(ROOT, "_bench.json")
LOG = []

def save():
    json.dump(LOG, open(OUT, "w"), indent=2)

def env_with(**kw):
    e = dict(os.environ)
    e.pop("BASH_ENV", None); e.pop("ENV", None)
    e["PATH"] = CARGO_BIN + ";" + e.get("PATH", "")
    e.update(kw)
    return e

def dirsize(p):
    tot = 0
    for dp, dn, fn in os.walk(p):
        for f in fn:
            try: tot += os.path.getsize(os.path.join(dp, f))
            except OSError: pass
    return tot

def clean():
    subprocess.run(["cargo","clean"], cwd=ROOT, env=env_with(),
                   capture_output=True, text=True, errors="replace")

def build(release=False, env=None):
    cmd = ["cargo","build"] + (["--release"] if release else [])
    t = time.perf_counter()
    p = subprocess.run(cmd, cwd=ROOT, env=env or env_with(),
                       capture_output=True, text=True, errors="replace")
    return time.perf_counter()-t, p

def measure(label, release=False, env=None, do_clean=True):
    if do_clean: clean()
    el, p = build(release, env)
    if p.returncode != 0:
        print(f"  !! {label} FAILED rc={p.returncode}")
        print("    ", (p.stderr or "")[-900:].replace("\n","\n     "))
        save(); return None
    tgt = os.path.join(ROOT,"target")
    prof = "release" if release else "debug"
    def sub(*a): 
        q = os.path.join(tgt, prof, *a)
        return dirsize(q)/1048576 if os.path.isdir(q) else 0
    exe = os.path.join(tgt, prof, "tauri-probe.exe")
    rec = dict(label=label, sec=round(el,1),
               target_mb=round(dirsize(tgt)/1048576,1),
               deps_mb=round(sub("deps"),1),
               incr_mb=round(sub("incremental"),1),
               build_mb=round(sub("build"),1),
               exe_kb=round(os.path.getsize(exe)/1024,1) if os.path.exists(exe) else 0)
    LOG.append(rec); save()
    print(f"  {rec['label']:<32} {rec['sec']:>7.1f}s  target {rec['target_mb']:>7.1f}MB  "
          f"deps {rec['deps_mb']:>6.1f}  incr {rec['incr_mb']:>6.1f}  build {rec['build_mb']:>5.1f}  "
          f"exe {rec['exe_kb']:>8.1f}KB")
    return rec

def incr(label, release=False, env=None):
    p = os.path.join(ROOT,"src","lib.rs")
    s = open(p, encoding="utf-8").read()
    s = s.replace('"Hello, {}!"', '"Hello {}!"') if '"Hello, {}!"' in s else s.replace('"Hello {}!"', '"Hello, {}!"')
    open(p,"w",encoding="utf-8",newline="\n").write(s)
    el, r = build(release, env)
    ok = "OK" if r.returncode==0 else "FAIL"
    print(f"  {label:<32} {el:>7.1f}s  (改一行增量 {ok})")
    rec = dict(label=label, sec=round(el,1), note="incremental")
    LOG.append(rec); save(); return el

CT = os.path.join(ROOT,"Cargo.toml")
BASE = open(CT, encoding="utf-8").read()
if "[profile.dev]" in BASE:
    BASE = BASE[:BASE.index("[profile.dev]")].rstrip()+"\n"

print("="*104)
print("A. 默认配置（cargo 出厂默认）")
print("="*104)
open(CT,"w",encoding="utf-8",newline="\n").write(BASE)
measure("A1 默认 debug 冷构建")
incr("A2 默认 debug 增量")

print()
print("="*104)
print("B. 体积优化 debug：opt-level=1 + debug=0 + strip [profile.dev]")
print("="*104)
OPT = BASE + """
[profile.dev]
opt-level = 1
debug = false
incremental = true
codegen-units = 16
strip = "debuginfo"

[profile.dev.package."*"]
opt-level = 1
debug = false
"""
open(CT,"w",encoding="utf-8",newline="\n").write(OPT)
measure("B1 优化 debug 冷构建")
incr("B2 优化 debug 增量")

print()
print("="*104)
print("C. 极限瘦身 debug：opt-level=z + lto thin + panic abort")
print("="*104)
LEAN = BASE + """
[profile.dev]
opt-level = "z"
debug = false
incremental = false
codegen-units = 1
lto = "thin"
strip = "symbols"
panic = "abort"
overflow-checks = false
debug-assertions = false

[profile.dev.package."*"]
opt-level = "z"
debug = false
"""
open(CT,"w",encoding="utf-8",newline="\n").write(LEAN)
measure("C1 极限瘦身 debug 冷构建")

open(CT,"w",encoding="utf-8",newline="\n").write(BASE)
print()
print("="*104)
print("D. release（对照）")
print("="*104)
measure("D1 默认 release", release=True)

print()
print("### 原始数据 ###")
for r in LOG:
    print(" ", json.dumps(r, ensure_ascii=False))
