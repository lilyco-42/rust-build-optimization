# sccache 效果实测：同样的冷构建，有/无 sccache 对比
import os, subprocess, time, json, shutil

ROOT = r"D:\Code\rust\question\tauri-probe"
CARGO_BIN = r"D:\app\scoop\apps\rustup\current\.cargo\bin"
SCCACHE = os.path.join(CARGO_BIN, "sccache.exe")
OUT = os.path.join(ROOT, "_bench_sccache.json")
LOG = []

def save(): json.dump(LOG, open(OUT,"w"), indent=2)

def base_env():
    e = dict(os.environ)
    e.pop("BASH_ENV", None); e.pop("ENV", None)
    e["PATH"] = CARGO_BIN + ";" + e.get("PATH","")
    return e

def clean():
    subprocess.run(["cargo","clean"], cwd=ROOT, env=base_env(),
                   capture_output=True, text=True, errors="replace")

def build(env):
    t = time.perf_counter()
    p = subprocess.run(["cargo","build"], cwd=ROOT, env=env,
                       capture_output=True, text=True, errors="replace")
    return time.perf_counter()-t, p.returncode

def dirsize(p):
    tot=0
    for dp,dn,fn in os.walk(p):
        for f in fn:
            try: tot+=os.path.getsize(os.path.join(dp,f))
            except OSError: pass
    return tot

# 准备：把 Cargo.toml 恢复成无 profile（避免干扰）
CT = os.path.join(ROOT,"Cargo.toml")
S = open(CT,encoding="utf-8").read()
if "[profile.dev]" in S: S = S[:S.index("[profile.dev]")].rstrip()+"\n"
open(CT,"w",encoding="utf-8",newline="\n").write(S)

print("="*90)
print("sccache 实测（默认 debug，全量冷构建）")
print("="*90)

# --- 1) 无 sccache 基线 ---
clean()
el1, rc1 = build(base_env())
sz1 = dirsize(os.path.join(ROOT,"target"))
print(f"  1) 无 sccache 冷构建           {el1:>7.1f}s  rc={rc1}  target {sz1/1048576:.1f}MB")
LOG.append(dict(label="no_sccache_cold", sec=round(el1,1), target_mb=round(sz1/1048576,1))); save()

# --- 2) sccache 预热（第一次编，全是 miss）---
env2 = base_env(); env2["RUSTC_WRAPPER"] = SCCACHE
clean()
el2, rc2 = build(env2)
print(f"  2) sccache 首次（全 miss）     {el2:>7.1f}s  rc={rc2}")
LOG.append(dict(label="sccache_first", sec=round(el2,1))); save()

# --- 3) 删 target 后重建（全 hit —— 这是换分支/clean 的真实场景）---
before = subprocess.run([SCCACHE,"--show-stats"], env=env2, capture_output=True, text=True).stdout
clean()
el3, rc3 = build(env2)
sz3 = dirsize(os.path.join(ROOT,"target"))
after = subprocess.run([SCCACHE,"--show-stats"], env=env2, capture_output=True, text=True).stdout
hits = misses = 0
for ln in after.splitlines():
    if ln.startswith("Cache hits "):    hits = int(ln.split()[-1])
    if ln.startswith("Cache misses "):  misses = int(ln.split()[-1])
print(f"  3) 删target后重建（全 hit）    {el3:>7.1f}s  rc={rc3}  target {sz3/1048576:.1f}MB")
print(f"     sccache: hits={hits}  misses={misses}")
LOG.append(dict(label="sccache_rebuild_allhit", sec=round(el3,1),
                target_mb=round(sz3/1048576,1), hits=hits, misses=misses)); save()

print()
print(f"  => 加速比：{el1/max(el3,0.1):.1f}x  （{el1:.0f}s -> {el3:.0f}s）")
print(f"  => sccache 缓存目录：")
r = subprocess.run([SCCACHE,"--show-stats"], env=env2, capture_output=True, text=True).stdout
for ln in r.splitlines():
    if "Cache size" in ln or "Cache location" in ln or "Max cache size" in ln:
        print("     ", ln.strip())
