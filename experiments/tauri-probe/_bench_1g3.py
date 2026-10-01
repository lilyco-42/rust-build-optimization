# 冲 1GB 第三波：用 opt-level="z" 压依赖 rlib（真正的杀手锏）
import os, subprocess, time, json

ROOT = r"D:\Code\rust\question\tauri-probe"
CB = r"D:\app\scoop\apps\rustup\current\.cargo\bin"
OUT = os.path.join(ROOT, "_bench_1g3.json")
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
                incr=dsz(os.path.join(d,"incremental"))/1048576,
                build=dsz(os.path.join(d,"build"))/1048576,
                pdb=extsig(d,".pdb")/1048576, rlib=extsig(d,".rlib")/1048576,
                rmeta=extsig(d,".rmeta")/1048576)

def set_ct(text, ct="rlib"):
    CT = os.path.join(ROOT,"Cargo.toml")
    base = open(CT, encoding="utf-8").read()
    if "[profile.dev]" in base: base = base[:base.index("[profile.dev]")].rstrip()+"\n"
    if "[lib]" in base:
        i = base.index("[lib]"); j = base.index("\n\n", i)
        base = base[:i] + ('[lib]\nname = "tauri_probe_lib"\ncrate-type = ["%s"]' % ct) + base[j:]
    open(CT,"w",encoding="utf-8",newline="\n").write(base + "\n" + text)

def test(label, prof, ct="rlib"):
    set_ct(prof, ct)
    clean()
    el, rc, p = build()
    if rc != 0:
        print(f"  {label:<40} FAILED")
        print("   ", (p.stderr or "")[-500:].replace("\n","\n    "))
        LOG.append(dict(label=label, failed=True)); save(); return None
    b = bd(); b["label"]=label; b["sec"]=round(el,1)
    LOG.append(b); save()
    flag = "  ✅ <=1GB" if b['total'] <= 1024 else ""
    print(f"  {label:<40} {el:>6.1f}s  total {b['total']:>7.1f}MB  deps {b['deps']:>6.1f}  "
          f"build {b['build']:>5.1f}  pdb {b['pdb']:>6.1f}  rlib {b['rlib']:>6.1f}  "
          f"rmeta {b['rmeta']:>6.1f}{flag}")
    return b

print("="*128)
print("冲 1GB 第三波：opt-level 对依赖 rlib 的影响")
print("="*128)

# A: 依赖 opt-level=2（前面的最佳）
test("A 依赖 opt=2 (基线 1245MB)", """
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
""")

# B: 依赖 opt-level="z" ← 关键测试
test("B 依赖 opt=z", """
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
""")

# C: 依赖 opt-level=1
test("C 依赖 opt=1", """
[profile.dev]
opt-level = 1
debug = false
incremental = false
codegen-units = 16
strip = "debuginfo"
panic = "abort"

[profile.dev.package."*"]
opt-level = 1
debug = false
""")
