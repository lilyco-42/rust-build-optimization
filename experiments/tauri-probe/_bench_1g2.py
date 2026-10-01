# 冲 1GB 第二波：测试真正的杀手锏
import os, subprocess, time, json

ROOT = r"D:\Code\rust\question\tauri-probe"
CB = r"D:\app\scoop\apps\rustup\current\.cargo\bin"
OUT = os.path.join(ROOT, "_bench_1g2.json")
LOG = []

def save(): json.dump(LOG, open(OUT,"w"), indent=2)

def E(extra=None):
    e = dict(os.environ)
    e.pop("BASH_ENV", None); e.pop("ENV", None)
    e["PATH"] = CB + ";" + e.get("PATH","")
    if extra: e.update(extra)
    return e

def clean():
    subprocess.run(["cargo","clean"], cwd=ROOT, env=E(), capture_output=True, text=True)

def build(extra_env=None):
    t = time.perf_counter()
    p = subprocess.run(["cargo","build"], cwd=ROOT, env=E(extra_env),
                       capture_output=True, text=True, errors="replace")
    return time.perf_counter()-t, p.returncode, p

def dsz(p):
    if not os.path.isdir(p): return 0
    tot=0
    for dp,dn,fn in os.walk(p):
        for f in fn:
            try: tot+=os.path.getsize(os.path.join(dp,f))
            except OSError: pass
    return tot

def extsig(d, ext):
    tot=0
    for dp,dn,fn in os.walk(d):
        for f in fn:
            if f.endswith(ext):
                try: tot+=os.path.getsize(os.path.join(dp,f))
                except OSError: pass
    return tot

def bd():
    d = os.path.join(ROOT,"target","debug")
    if not os.path.isdir(d): return {}
    return dict(total=dsz(d)/1048576,
                deps=dsz(os.path.join(d,"deps"))/1048576,
                incr=dsz(os.path.join(d,"incremental"))/1048576,
                build=dsz(os.path.join(d,"build"))/1048576,
                pdb=extsig(d,".pdb")/1048576,
                rlib=extsig(d,".rlib")/1048576,
                rmeta=extsig(d,".rmeta")/1048576)

def set_ct(text):
    CT = os.path.join(ROOT,"Cargo.toml")
    base = open(CT, encoding="utf-8").read()
    if "[profile.dev]" in base: base = base[:base.index("[profile.dev]")].rstrip()+"\n"
    if "[lib]" in base:
        i = base.index("[lib]"); j = base.index("\n\n", i)
        base = base[:i] + '[lib]\nname = "tauri_probe_lib"\ncrate-type = ["rlib"]' + base[j:]
    open(CT,"w",encoding="utf-8",newline="\n").write(base + "\n" + text)

def test(label, prof, extra_env=None):
    set_ct(prof)
    clean()
    el, rc, p = build(extra_env)
    if rc != 0:
        print(f"  {label:<34} FAILED rc={rc}")
        print("   ", (p.stderr or "")[-600:].replace("\n","\n    "))
        LOG.append(dict(label=label, failed=True)); save(); return None
    b = bd(); b["label"]=label; b["sec"]=round(el,1)
    LOG.append(b); save()
    print(f"  {label:<34} {el:>6.1f}s  total {b['total']:>7.1f}MB  deps {b['deps']:>6.1f}  "
          f"incr {b['incr']:>5.1f}  build {b['build']:>5.1f}  pdb {b['pdb']:>6.1f}  "
          f"rlib {b['rlib']:>6.1f}  rmeta {b['rmeta']:>6.1f}")
    return b

P_BEST = """
[profile.dev]
opt-level = 1
debug = false
incremental = false
codegen-units = 16
strip = "debuginfo"
panic = "abort"

[profile.dev.package."*"]
opt-level = 2
debug = false
"""

print("="*124)
print("冲 1GB 第二波")
print("="*124)

test("A 当前最佳 (1,245MB 基线)", P_BEST)

# B: 关掉 WASM 无关的默认特性？先看 windows 的 feature 能否收敛
# 关键：windows crate 有 features，tauri 默认拉了一大堆
test("B + 减少 windows feature", """
[profile.dev]
opt-level = 1
debug = false
incremental = false
codegen-units = 16
strip = "debuginfo"
panic = "abort"

[profile.dev.package."*"]
opt-level = 2
debug = false

# windows crate 只给上游用，不需要 debug 符号
[profile.dev.package.windows]
debug = false
opt-level = "z"

[profile.dev.package.windows-sys]
debug = false
opt-level = "z"

[profile.dev.package.webview2-com]
debug = false

[profile.dev.package.webview2-com-sys]
debug = false
""")

# C: 只编 lib，不编 bin（Tauri 的 bin 只是个壳）
print()
print("  注：cargo build 会同时编 lib 和 bin。试试只编 lib 看差多少")
CT = os.path.join(ROOT,"Cargo.toml")
set_ct(P_BEST)
clean()
t=time.perf_counter()
p=subprocess.run(["cargo","build","--lib"],cwd=ROOT,env=E(),capture_output=True,text=True,errors="replace")
el=time.perf_counter()-t
b=bd(); b["label"]="C 只编 --lib"; b["sec"]=round(el,1)
LOG.append(b); save()
print(f"  {'C 只编 --lib':<34} {el:>6.1f}s  total {b['total']:>7.1f}MB  deps {b['deps']:>6.1f}  "
      f"incr {b['incr']:>5.1f}  build {b['build']:>5.1f}  pdb {b['pdb']:>6.1f}  "
      f"rlib {b['rlib']:>6.1f}  rmeta {b['rmeta']:>6.1f}")
