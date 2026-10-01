import os, subprocess, time, json
ROOT = r"D:\Code\rust\question\tauri-probe"
CARGO_BIN = r"D:\app\scoop\apps\rustup\current\.cargo\bin"
SCCACHE = os.path.join(CARGO_BIN, "sccache.exe")

e = dict(os.environ)
e.pop("BASH_ENV", None); e.pop("ENV", None)
e["PATH"] = CARGO_BIN + ";" + e.get("PATH","")
e["RUSTC_WRAPPER"] = SCCACHE

def stats():
    o = subprocess.run([SCCACHE,"--show-stats"], env=e, capture_output=True, text=True).stdout
    d = {}
    for ln in o.splitlines():
        parts = ln.split()
        if len(parts) >= 3 and parts[0] == "Cache" and parts[1] in ("hits","misses","size"):
            try: d[parts[1]] = int(parts[-1].replace(",",""))
            except ValueError:
                try: d[parts[1]] = int(parts[2].replace(",",""))
                except ValueError: pass
    return d, o

def clean():
    subprocess.run(["cargo","clean"], cwd=ROOT, env=e, capture_output=True, text=True)

def build():
    t = time.perf_counter()
    p = subprocess.run(["cargo","build"], cwd=ROOT, env=e, capture_output=True, text=True, errors="replace")
    return time.perf_counter()-t, p.returncode

before, _ = stats()
print(f"重建前 sccache: hits={before.get('hits')} misses={before.get('misses')}")

print("删 target，测【全命中】重建 ...")
clean()
el, rc = build()
after, raw = stats()
print(f"  => 重建耗时 {el:.1f} s   rc={rc}")
print(f"  => hits={after.get('hits')}  misses={after.get('misses')}")
print()
print("=== sccache 完整统计 ===")
for ln in raw.splitlines()[:14]:
    print("  ", ln.strip())

json.dump(dict(rebuild_sec=round(el,1), hits=after.get('hits'), misses=after.get('misses')),
          open(os.path.join(ROOT,"_sccache_result.json"),"w"), indent=2)
