# 编译时间对照：clang-cl / cl.exe / zig cc vs cargo(MSVC) / cargo-zigbuild
# 全部使用 Python 计时（subprocess + perf_counter），剥掉 BASH_ENV
import os, re, subprocess, time, shutil, statistics

ROOT = r"D:\Code\rust\question"
PROBE = os.path.join(ROOT, "_probe")
CC_SRC = os.path.join(ROOT, "ip-c-msvc", "main.c")

LLVM = r"D:\APP\scoop\apps\llvm\current\bin"
MSVC = r"D:\VS\Product\VC\Tools\MSVC\14.51.36231"
SDK  = r"D:\Windows Kits\10"
VER  = "10.0.26100.0"
ZIG  = r"D:\APP\scoop\apps\zvm\current\data\bin\zig.exe"

env = dict(os.environ)
env.pop("BASH_ENV", None); env.pop("ENV", None)
env["INCLUDE"] = rf"{MSVC}\include;{SDK}\Include\{VER}\ucrt;{SDK}\Include\{VER}\um;{SDK}\Include\{VER}\shared"
env["LIB"]     = rf"{MSVC}\lib\x64;{SDK}\Lib\{VER}\ucrt\x64;{SDK}\Lib\{VER}\um\x64"
env["PATH"]    = rf"{LLVM};{MSVC}\bin\Hostx64\x64;{os.path.dirname(ZIG)};" + env.get("PATH","")

def run(cmd, cwd=None):
    t = time.perf_counter()
    p = subprocess.run(cmd, cwd=cwd, env=env, capture_output=True, text=True,
                       shell=False, errors="replace")
    return (time.perf_counter()-t)*1000, p

def bench(name, cmd, cwd=None, out=None, n=3):
    ts = []
    for _ in range(n):
        if out and os.path.exists(out):
            os.remove(out)
        ms, p = run(cmd, cwd)
        if out and not os.path.exists(out):
            print(f"  !! {name} did not produce {out}; rc={p.returncode}")
            print("     ", (p.stderr or p.stdout)[:400])
            return None
        ts.append(ms)
    sz = os.path.getsize(out) if out and os.path.exists(out) else 0
    best, med = min(ts), statistics.median(ts)
    print(f"{name:<34} best {best:7.0f} ms   med {med:7.0f} ms   {sz:>8} B")
    return best

print("="*82)
print("C 侧：单文件编译 + 链接")
print("="*82)
bench("clang-cl /O2 /MT", [f"{LLVM}\\clang-cl.exe","/nologo","/O2","/GS-","/MT","-w",
      "/std:c11", f"/Fe:{PROBE}\\t_cc_mt.exe", CC_SRC, "ws2_32.lib","iphlpapi.lib"],
      out=f"{PROBE}\\t_cc_mt.exe")
bench("clang-cl /O2 /MD", [f"{LLVM}\\clang-cl.exe","/nologo","/O2","/GS-","/MD","-w",
      "/std:c11", f"/Fe:{PROBE}\\t_cc_md.exe", CC_SRC, "ws2_32.lib","iphlpapi.lib"],
      out=f"{PROBE}\\t_cc_md.exe")
bench("cl /O2 /MT", [f"{MSVC}\\bin\\Hostx64\\x64\\cl.exe","/nologo","/O2","/GS-","/MT",
      "/std:c11", f"/Fe:{PROBE}\\t_cl_mt.exe", CC_SRC, "ws2_32.lib","iphlpapi.lib"],
      out=f"{PROBE}\\t_cl_mt.exe")
bench("zig cc windows-gnu", [ZIG,"cc","-target","x86_64-windows-gnu","-O2","-w",
      "-o",f"{PROBE}\\t_zig_gnu.exe", CC_SRC,"-lws2_32","-liphlpapi"],
      out=f"{PROBE}\\t_zig_gnu.exe")

CARGO = r"D:\APP\scoop\apps\rustup\current\.cargo\bin\cargo.exe"

print()
print("="*82)
print("Rust 侧：cargo 冷构建全量（删 target）")
print("="*82)

def cargo_clean(root, exe):
    shutil.rmtree(os.path.join(root,"target"), ignore_errors=True)

def bench_cargo(name, root, cmd, exe, n=2):
    ts=[]
    for _ in range(n):
        cargo_clean(root, exe)
        t=time.perf_counter()
        p=subprocess.run(cmd, cwd=root, env=env, capture_output=True, text=True, errors="replace")
        ts.append((time.perf_counter()-t)*1000)
        if not os.path.exists(exe):
            print(f"  !! {name} failed: {(p.stderr or p.stdout)[-500:]}")
            return None
    sz=os.path.getsize(exe) if os.path.exists(exe) else 0
    best=min(ts)
    print(f"{name:<34} best {best:7.0f} ms              {sz:>8} B")
    return best

bench_cargo("cargo build --release (MSVC)",
    os.path.join(ROOT,"ip-bare"),
    [CARGO,"build","--release"],
    os.path.join(ROOT,"ip-bare","target","release","ip-bare.exe"))

bench_cargo("cargo zigbuild --release (gnu)",
    os.path.join(ROOT,"ip-bare-gnu"),
    [CARGO,"zigbuild","--target","x86_64-pc-windows-gnu","--release"],
    os.path.join(ROOT,"ip-bare-gnu","target","x86_64-pc-windows-gnu","release","ip-bare-gnu.exe"))
