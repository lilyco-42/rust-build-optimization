"""Rust vs C vs Go：同一功能的编译速度 / 产物体积 / 构建目录体积对照。

- 冷构建：删掉整个构建目录（Cargo 删 target，xmake 删 build+.xmake，Go 换空 GOCACHE）
- 增量：只 touch 一个源文件再编
- 每个数字跑 N 次取最小值
"""
import os
import shutil
import subprocess
import sys
import tempfile
import time

ROOT = os.path.dirname(os.path.abspath(__file__))
N = int(os.environ.get("N", "3"))


def dirsize(p):
    if not os.path.isdir(p):
        return 0
    total = 0
    for dp, _, fns in os.walk(p):
        for f in fns:
            try:
                total += os.path.getsize(os.path.join(dp, f))
            except OSError:
                pass
    return total


def timed(cmd, cwd, env=None):
    t0 = time.perf_counter()
    p = subprocess.run(cmd, cwd=cwd, env=env, stdout=subprocess.PIPE,
                       stderr=subprocess.STDOUT, text=True)
    return time.perf_counter() - t0, p.returncode, p.stdout


def best(cmd, cwd, env=None, n=N, clean=None):
    """跑 n 次取最小值。clean 每次跑之前都执行——冷构建必须每次都清空。"""
    ts, rc, out = [], 0, ""
    for _ in range(n):
        if clean:
            clean()
        t, rc, out = timed(cmd, cwd, env)
        ts.append(t)
    return min(ts), rc, out


def rm(*paths):
    for p in paths:
        if os.path.isdir(p):
            shutil.rmtree(p, ignore_errors=True)
        elif os.path.isfile(p):
            try:
                os.remove(p)
            except OSError:
                pass


def touch(p):
    os.utime(p, None)


ROWS = []


def row(name, cold, incr, exe, builddir):
    ROWS.append((name, cold, incr, exe, builddir))
    print("| %-26s | %6.2fs | %6.2fs | %9s | %11s |" % (
        name, cold, incr,
        "-" if exe is None else "{:,}".format(exe),
        "-" if builddir is None else "{:,}".format(builddir)))
    sys.stdout.flush()


print("# 编译对照：Rust vs C/xmake vs Go（同一功能，同一台机器）")
print()
print("时间：%s ｜ 每个数字取 %d 次最小值" % (time.strftime("%Y-%m-%d %H:%M:%S"), N))
print()
print("| 项目 | 冷构建 | 增量 | exe 字节 | 构建目录字节 |")
print("|---|---|---|---|---|")

# ---------------------------------------------------------------- C / xmake
ipc = os.path.join(ROOT, "ip-c")
if os.path.isdir(ipc):
    build = os.path.join(ipc, "build")
    cfg = os.path.join(ipc, ".xmake")
    src = os.path.join(ipc, "src", "main.c")
    exe = os.path.join(build, "windows", "x64", "release", "ip-c.exe")

    rm(build, cfg)
    t0, _, _ = timed(["xmake", "f", "-p", "windows", "-a", "x64", "--toolchain=msvc", "-y"], ipc)
    t1, rc, out = timed(["xmake", "-y"], ipc)
    if rc != 0:
        print("!! xmake 失败:", out.strip()[-300:])
    row("C/xmake release(含配置)", t0 + t1, 0.0, None, None)

    cold, rc, _ = best(["xmake", "-y"], ipc, clean=lambda: rm(build))
    touch(src)
    incr, _, _ = best(["xmake", "-y"], ipc)
    row("C/xmake release", cold, incr,
        os.path.getsize(exe) if os.path.exists(exe) else None,
        dirsize(build) + dirsize(cfg))

# ---------------------------------------------------------------- Go
ipgo = os.path.join(ROOT, "ip-go")
if os.path.isdir(ipgo):
    exe = os.path.join(ipgo, "ip-go.exe")
    gocache = tempfile.mkdtemp(prefix="gocache-")
    env = dict(os.environ, GOCACHE=gocache)
    def clean_go():
        rm(exe)
        shutil.rmtree(gocache, ignore_errors=True)
        os.makedirs(gocache, exist_ok=True)

    cold, rc, out = best(["go", "build", "-o", "ip-go.exe", "."], ipgo, env=env,
                         clean=clean_go)
    if rc != 0:
        print("!! go 失败:", out.strip()[-300:])
    touch(os.path.join(ipgo, "main.go"))
    incr, _, _ = best(["go", "build", "-o", "ip-go.exe", "."], ipgo, env=env)
    row("Go（冷 GOCACHE）", cold, incr,
        os.path.getsize(exe) if os.path.exists(exe) else None,
        dirsize(gocache))
    shutil.rmtree(gocache, ignore_errors=True)

# ---------------------------------------------------------------- Rust
def cargo_proj(proj, label, extra_args, exe_rel):
    d = os.path.join(ROOT, proj)
    if not os.path.isdir(d):
        return
    tgt = os.path.join(d, "target")
    cmd = ["cargo", "build"] + extra_args
    cold, rc, out = best(cmd, d, clean=lambda: rm(tgt))
    if rc != 0:
        print("!! %s 失败: %s" % (label, out.strip().splitlines()[-1:] or out[-200:]))
        return
    touch(os.path.join(d, "src", "main.rs"))
    incr, _, _ = best(cmd, d)
    exe = os.path.join(d, exe_rel)
    row(label, cold, incr,
        os.path.getsize(exe) if os.path.exists(exe) else None,
        dirsize(tgt))


cargo_proj("ip-bare", "Rust ip-bare dev（no_std）", [],
           os.path.join("target", "debug", "ip-bare.exe"))
cargo_proj("ip-bare", "Rust ip-bare release", ["--release"],
           os.path.join("target", "release", "ip-bare.exe"))
cargo_proj("ip-std", "Rust ip-std dev（std）", [],
           os.path.join("target", "debug", "ip-std.exe"))
cargo_proj("ip-std", "Rust ip-std release（std）", ["--release"],
           os.path.join("target", "release", "ip-std.exe"))

print()
print("- **冷构建** = 删掉整个构建目录后从头编；**增量** = 只 touch 一个源文件。")
print("- **构建目录字节** = 编译器产出的全部中间产物（C 含 `.xmake` 配置缓存，Go 为 GOCACHE）。")
