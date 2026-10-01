# Tauri 真实项目实测：debug 速度 vs 体积
# 目标：找出「debug 尽量快 + 本地产物尽量小」的最优组合
import os, subprocess, time, shutil, json, sys

ROOT = r"D:\Code\rust\question\tauri-probe"
CARGO_BIN = r"D:\app\scoop\apps\rustup\current\.cargo\bin"
LOG = []

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

def run(cmd, env, cwd=ROOT):
    t = time.perf_counter()
    p = subprocess.run(cmd, cwd=cwd, env=env, capture_output=True, text=True, errors="replace")
    return time.perf_counter() - t, p

def clean():
    # 用 cargo clean（cargo 官方清理手段），不用 shutil.rmtree
    subprocess.run(["cargo", "clean"], cwd=ROOT, env=env_with(),
                   capture_output=True, text=True, errors="replace")

def measure(label, env, release=False):
    clean()
    cmd = ["cargo", "build"] + (["--release"] if release else [])
    el, p = run(cmd, env)
    if p.returncode != 0:
        print(f"  !! {label} FAILED rc={p.returncode}")
        print("    ", (p.stderr or "")[-1200:])
        return None
    tgt = os.path.join(ROOT, "target")
    profile = "release" if release else "debug"
    exe = os.path.join(tgt, profile, "tauri-probe.exe")
    total = dirsize(tgt)
    deps  = dirsize(os.path.join(tgt, profile, "deps")) if os.path.isdir(os.path.join(tgt, profile, "deps")) else 0
    inc   = dirsize(os.path.join(tgt, profile, "incremental")) if os.path.isdir(os.path.join(tgt, profile, "incremental")) else 0
    exesz = os.path.getsize(exe) if os.path.exists(exe) else 0
    rec = dict(label=label, sec=round(el,1), target_mb=round(total/1048576,1),
               deps_mb=round(deps/1048576,1), inc_mb=round(inc/1048576,1), exe_kb=round(exesz/1024,1))
    LOG.append(rec)
    print(f"  {label:<34} {rec['sec']:>7.1f}s   target {rec['target_mb']:>7.1f}MB   "
          f"deps {rec['deps_mb']:>6.1f}MB   incr {rec['inc_mb']:>6.1f}MB   exe {rec['exe_kb']:>8.1f}KB")
    return rec

def touch_rs():
    # 模拟「改一行业务代码」，测增量重建
    p = os.path.join(ROOT, "src", "lib.rs")
    s = open(p, encoding="utf-8").read()
    s = s.replace("Hello, {}!", "Hello {}!")
    open(p, "w", encoding="utf-8", newline="\n").write(s)

def incr_measure(label, env, release=False):
    touch_rs()
    cmd = ["cargo", "build"] + (["--release"] if release else [])
    el, p = run(cmd, env)
    print(f"  {label:<34} {el:>7.1f}s   (改一行后增量)")
    return el

print("="*100)
print("A. 默认配置（cargo 出厂默认）")
print("="*100)
base = env_with()
measure("默认 debug", base)
incr_measure("默认 debug 增量", base)

print()
print("="*100)
print("B. 优化 debug：opt-level=1 + 无调试信息 + 单CGU  [profile.dev]")
print("="*100)
# 写一个 dev 优化版 profile
prof = os.path.join(ROOT, "Cargo.toml")
orig = open(prof, encoding="utf-8").read()
if "[profile.dev]" in orig:
    orig = orig[:orig.index("[profile.dev]")].rstrip() + "\n"
opt_dev = orig + """
[profile.dev]
opt-level = 1
debug = false
incremental = true
codegen-units = 16
strip = "debuginfo"

[profile.dev.package."*"]
opt-level = 0
debug = false
"""
open(prof, "w", encoding="utf-8", newline="\n").write(opt_dev)
b = env_with()
measure("优化 debug (opt1/无dbg)", b)
incr_measure("优化 debug 增量", b)

print()
print("="*100)
print("C. release（对照，确认体积）")
print("="*100)
open(prof, "w", encoding="utf-8", newline="\n").write(orig)
c = env_with()
measure("默认 release", c, release=True)

json.dump(LOG, open(os.path.join(ROOT,"_bench.json"),"w"), indent=2)
print()
print("saved -> _bench.json")
