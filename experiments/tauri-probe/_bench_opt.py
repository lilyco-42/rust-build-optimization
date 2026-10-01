# 依赖 opt-level 对「编译速度 vs 体积」的影响 —— 为「debug 要快 + 产物要小」找最优点
import os, subprocess, time, json, shutil

ROOT = r"D:\Code\rust\question\tauri-probe"
CB = r"D:\app\scoop\apps\rustup\current\.cargo\bin"
OUT = os.path.join(ROOT, "_bench_opt.json")
CFG = os.path.join(ROOT, ".cargo", "config.toml")
LOG = []

def save(): json.dump(LOG, open(OUT, "w"), indent=2)

def E():
    e = dict(os.environ)
    e.pop("BASH_ENV", None); e.pop("ENV", None)
    e["PATH"] = CB + ";" + e.get("PATH", "")
    return e

def clean():
    subprocess.run(["cargo", "clean"], cwd=ROOT, env=E(), capture_output=True, text=True)

def build():
    t = time.perf_counter()
    p = subprocess.run(["cargo", "build"], cwd=ROOT, env=E(), capture_output=True,
                       text=True, errors="replace")
    return time.perf_counter() - t, p.returncode

def dsz(p):
    if not os.path.isdir(p): return 0
    tot = 0
    for dp, dn, fn in os.walk(p):
        for f in fn:
            try: tot += os.path.getsize(os.path.join(dp, f))
            except OSError: pass
    return tot

def extsig(d, ext):
    tot = 0
    for dp, dn, fn in os.walk(d):
        for f in fn:
            if f.endswith(ext):
                try: tot += os.path.getsize(os.path.join(dp, f))
                except OSError: pass
    return tot

def bd():
    d = os.path.join(ROOT, "target", "debug")
    if not os.path.isdir(d): return {}
    return dict(total=dsz(d)/1048576,
                rlib=extsig(d, ".rlib")/1048576,
                rmeta=extsig(d, ".rmeta")/1048576,
                pdb=extsig(d, ".pdb")/1048576)

def write_cargo(dep_opt, extra=""):
    ct = os.path.join(ROOT, "Cargo.toml")
    base = open(ct, encoding="utf-8").read()
    if "[profile.dev]" in base:
        base = base[:base.index("[profile.dev]")].rstrip() + "\n"
    # 去掉已有的 build-override
    if "[profile.dev.build-override]" in base:
        base = base[:base.index("[profile.dev.build-override]")].rstrip() + "\n"
    if "[lib]" in base:
        i = base.index("[lib]"); j = base.index("\n\n", i)
        base = base[:i] + '[lib]\nname = "tauri_probe_lib"\ncrate-type = ["rlib"]' + base[j:]
    tmpl = """
[profile.dev]
opt-level = 1
debug = false
incremental = true
codegen-units = 16
strip = "debuginfo"
panic = "abort"

[profile.dev.package."*"]
opt-level = {dep_opt}
debug = false
{extra}
"""
    open(ct, "w", encoding="utf-8", newline="\n").write(base + tmpl.format(dep_opt=dep_opt, extra=extra))

# 关键：禁用 sccache，保证公平（否则命中缓存会掩盖真实编译时间）
CFG_SAVE = None
if os.path.exists(CFG):
    CFG_SAVE = open(CFG, encoding="utf-8").read()
    open(CFG, "w", encoding="utf-8", newline="\n").write(
        '# 实验期间禁用 sccache\n'
        '[target.x86_64-pc-windows-msvc]\n'
        'rustflags = ["-C", "link-args=/DEBUG:NONE"]\n')

def test(label, dep_opt, extra=""):
    write_cargo(dep_opt, extra)
    clean()
    el, rc = build()
    if rc != 0:
        print(f"  {label:<40} FAILED rc={rc}")
        LOG.append(dict(label=label, failed=True)); save(); return None
    b = bd(); b["label"] = label; b["sec"] = round(el, 1)
    LOG.append(b); save()
    print(f"  {label:<40} {el:>6.1f}s  total {b['total']:>7.1f}MB  "
          f"rlib {b['rlib']:>6.1f}  rmeta {b['rmeta']:>6.1f}")
    return b

try:
    print("=" * 110)
    print("依赖 opt-level 对比（已禁用 sccache，每次 cargo clean）")
    print("=" * 110)
    print("  自己代码固定 opt-level=1；变量只有依赖的 opt-level\n")

    test("A 依赖 opt-level=0 (编译最快?)", 0)
    test("B 依赖 opt-level=1", 1)
    test("C 依赖 opt-level=2", 2)
    test("D 依赖 opt-level=\"z\" (体积最小?)", '"z"')
    test("E 混合: windows* 用 z，其余 0", 0, extra='''
[profile.dev.package.windows]
opt-level = "z"
[profile.dev.package.windows-sys]
opt-level = "z"
[profile.dev.package.webview2-com]
opt-level = "z"
[profile.dev.package.webview2-com-sys]
opt-level = "z"
''')
finally:
    if CFG_SAVE is not None:
        open(CFG, "w", encoding="utf-8", newline="\n").write(CFG_SAVE)
        print("\n(.cargo/config.toml 已恢复)")

print("\n### 汇总 ###")
for r in LOG:
    if r.get("failed"): continue
    print(f"  {r['label']:<40} {r['sec']:>6.1f}s  {r['total']:>7.1f}MB")
