# 冲 1 GB 专项实验：逐项定位可砍的体积
import os, subprocess, time, json

ROOT = r"D:\Code\rust\question\tauri-probe"
CB = r"D:\app\scoop\apps\rustup\current\.cargo\bin"
OUT = os.path.join(ROOT, "_bench_1g.json")
LOG = []

def save(): json.dump(LOG, open(OUT,"w"), indent=2)

def E():
    e = dict(os.environ)
    e.pop("BASH_ENV", None); e.pop("ENV", None)
    e["PATH"] = CB + ";" + e.get("PATH","")
    return e

def clean():
    subprocess.run(["cargo","clean"], cwd=ROOT, env=E(), capture_output=True, text=True)

def build():
    t = time.perf_counter()
    p = subprocess.run(["cargo","build"], cwd=ROOT, env=E(), capture_output=True,
                       text=True, errors="replace")
    return time.perf_counter()-t, p.returncode

def dsz(p):
    if not os.path.isdir(p): return 0
    tot=0
    for dp,dn,fn in os.walk(p):
        for f in fn:
            try: tot+=os.path.getsize(os.path.join(dp,f))
            except OSError: pass
    return tot

def breakdown():
    d = os.path.join(ROOT,"target","debug")
    if not os.path.isdir(d): return {}
    def extsig(ext):
        tot=0
        for dp,dn,fn in os.walk(d):
            for f in fn:
                if f.endswith(ext):
                    try: tot+=os.path.getsize(os.path.join(dp,f))
                    except OSError: pass
        return tot
    return dict(
        total  = dsz(d)/1048576,
        deps   = dsz(os.path.join(d,"deps"))/1048576,
        incr   = dsz(os.path.join(d,"incremental"))/1048576,
        build  = dsz(os.path.join(d,"build"))/1048576,
        pdb    = extsig(".pdb")/1048576,
        rlib   = extsig(".rlib")/1048576,
        rmeta  = extsig(".rmeta")/1048576,
        lib    = extsig(".lib")/1048576,
    )

def test(label, cargo_toml_patch, lib_patch=None):
    # 写 Cargo.toml
    CT = os.path.join(ROOT,"Cargo.toml")
    base = open(CT, encoding="utf-8").read()
    if "[profile.dev]" in base: base = base[:base.index("[profile.dev]")].rstrip()+"\n"
    if "[lib]" in base and lib_patch is not None:
        i = base.index("[lib]"); j = base.index("\n\n", i)
        base = base[:i] + lib_patch + base[j:]
    open(CT,"w",encoding="utf-8",newline="\n").write(base + "\n" + cargo_toml_patch)

    clean()
    el, rc = build()
    b = breakdown()
    if rc != 0:
        print(f"  {label:<30} FAILED rc={rc}")
        LOG.append(dict(label=label, failed=True)); save(); return None
    b["label"]=label; b["sec"]=round(el,1)
    LOG.append(b); save()
    print(f"  {label:<30} {el:>6.1f}s  total {b['total']:>7.1f}MB  "
          f"deps {b['deps']:>6.1f}  incr {b['incr']:>5.1f}  build {b['build']:>5.1f}  "
          f"| pdb {b['pdb']:>6.1f}  rlib {b['rlib']:>6.1f}  rmeta {b['rmeta']:>6.1f}  lib {b['lib']:>6.1f}")
    return b

LIB_FULL = '[lib]\nname = "tauri_probe_lib"\ncrate-type = ["staticlib", "cdylib", "rlib"]'
LIB_RUST = '[lib]\nname = "tauri_probe_lib"\ncrate-type = ["rlib"]'

P_CURRENT = """
[profile.dev]
opt-level = 1
debug = "line-tables-only"
incremental = true
codegen-units = 16
strip = "debuginfo"
panic = "abort"

[profile.dev.package."*"]
opt-level = 2
debug = false
"""

# 无调试信息版本
P_NODBG = """
[profile.dev]
opt-level = 1
debug = false
incremental = true
codegen-units = 16
strip = "debuginfo"
panic = "abort"

[profile.dev.package."*"]
opt-level = 2
debug = false
"""

# 只 rlib + 无调试信息
P_LEAN = """
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

print("="*118)
print("冲 1GB 专项实验")
print("="*118)
test("0 基线(当前配置)", P_CURRENT, LIB_FULL)
test("1 + 去掉 staticlib/cdylib", P_CURRENT, LIB_RUST)
test("2 + debug=false(完全无调试)", P_NODBG, LIB_RUST)
test("3 + 关增量编译", P_LEAN, LIB_RUST)
