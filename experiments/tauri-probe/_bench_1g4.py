# 冲 1GB 第四波：pdb + build.rs 侧优化
import os, subprocess, time, json

ROOT = r"D:\Code\rust\question\tauri-probe"
CB = r"D:\app\scoop\apps\rustup\current\.cargo\bin"
OUT = os.path.join(ROOT, "_bench_1g4.json")
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
    p = subprocess.run(["cargo","build"], cwd=ROOT, env=E(extra_env), capture_output=True,
                       text=True, errors="replace")
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
    return dict(total=dsz(d)/1048576, deps=dsz(os.path.join(d,"deps"))/1048576,
                build=dsz(os.path.join(d,"build"))/1048576,
                pdb=extsig(d,".pdb")/1048576, rlib=extsig(d,".rlib")/1048576,
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
        print(f"  {label:<38} FAILED")
        print("   ", (p.stderr or "")[-400:].replace("\n","\n    "))
        LOG.append(dict(label=label, failed=True)); save(); return None
    b = bd(); b["label"]=label; b["sec"]=round(el,1)
    LOG.append(b); save()
    flag = "  ✅✅ <=1GB 达成" if b['total'] <= 1024 else ""
    print(f"  {label:<38} {el:>6.1f}s  total {b['total']:>7.1f}MB  deps {b['deps']:>6.1f}  "
          f"build {b['build']:>5.1f}  pdb {b['pdb']:>6.1f}  rlib {b['rlib']:>6.1f}  "
          f"rmeta {b['rmeta']:>6.1f}{flag}")
    return b

P_Z = """
[profile.dev]
opt-level = 1
debug = false
incremental = false
codegen-units = 16
strip = "debuginfo"
panic = "abort"

[profile.dev.package."*"]
opt-level = "z"
debug = false
"""

# build.rs 侧的 profile —— build script 也可单独设置
P_Z_BS = P_Z + """
# build script 本身也用 z，减小 build/ 目录
[profile.dev.build-override]
opt-level = "z"
debug = false
strip = "symbols"
codegen-units = 16
"""

print("="*124)
print("冲 1GB 第四波")
print("="*124)

test("A 依赖 opt=z (1,132MB 基线)", P_Z)
test("B + build-override opt=z", P_Z_BS)
test("C + MSVC /DEBUG:NONE 关 PDB", P_Z_BS,
     {"RUSTFLAGS": "-C link-args=/DEBUG:NONE"})
