"""测量「泛型单态化 vs 动态分发」的编译代价与运行时代价。

两个 crate 源码只差一行（drive 的签名），其余全部冻结。
每个 crate 有自己的 target 目录（吸取上一轮的教训：绝不复用）。
"""
import json
import os
import re
import subprocess
import time
from pathlib import Path

ROOT = Path(__file__).parent
ENV = {k: v for k, v in os.environ.items() if k not in ("BASH_ENV", "ENV")}


def run(args, cwd, timeout=1800):
    t = time.perf_counter()
    p = subprocess.run(args, cwd=cwd, env=ENV, capture_output=True, text=True,
                       encoding="utf-8", errors="replace", timeout=timeout)
    return time.perf_counter() - t, p.returncode, (p.stdout or "") + (p.stderr or "")


def size_mb(p):
    if not p.exists():
        return 0.0
    seen, total = set(), 0
    for f in p.rglob("*"):
        try:
            if not f.is_file():
                continue
            st = f.stat()
        except OSError:
            continue
        k = (st.st_dev, st.st_ino)
        if k in seen:
            continue
        seen.add(k)
        total += st.st_size
    return round(total / 1048576, 2)


def mono_count(crate, name):
    """数出被单态化的 drive 实例个数（nightly -Zprint-mono-items）。

    注意 `-Zprint-mono-items` 只接受 y/yes/on/true/n/no/off/false ——
    早期这里写的是 `=lazy`，rustc 直接报错，于是这一列一直是 null。
    """
    _, rc, out = run(["cargo", "+nightly", "rustc", "--", "-Zprint-mono-items=yes"],
                     crate)
    if rc != 0:
        return None
    # 行形如：MONO_ITEM fn drive::<T0> @@ dynproof_mono.<hash>-cgu.0[Internal]
    names = re.findall(r"^MONO_ITEM\s+\S+\s+(\S+)\s+@@", out, re.M)
    return sum(1 for n in names if n.startswith("drive::<")) or None


results = []
for variant in ("mono", "dyn"):
    c = ROOT / variant
    name = f"dynproof-{variant}"
    exe = c / "target" / "debug" / f"{name}.exe"
    exe_rel = c / "target" / "release" / f"{name}.exe"
    print("=" * 74, flush=True)
    print(f"{variant}   ({c})", flush=True)
    print("=" * 74, flush=True)

    run(["cargo", "clean"], c)
    dev_s, rc, out = run(["cargo", "build"], c)
    print(f"  dev 冷构建      {dev_s:6.2f}s  rc={rc}", flush=True)
    dev_mb = size_mb(c / "target")
    dev_exe = exe.stat().st_size if exe.exists() else 0
    print(f"  dev target/     {dev_mb:6.2f} MB", flush=True)
    print(f"  dev exe         {dev_exe:6d} B", flush=True)

    n_mono = mono_count(c, name)
    if n_mono is not None:
        print(f"  单态化 drive 实例数 ≈ {n_mono}", flush=True)

    run(["cargo", "clean"], c)
    rel_s, rc, out = run(["cargo", "build", "--release"], c)
    print(f"  release 冷构建  {rel_s:6.2f}s  rc={rc}", flush=True)
    rel_mb = size_mb(c / "target")
    rel_exe = exe_rel.stat().st_size if exe_rel.exists() else 0
    print(f"  release target/ {rel_mb:6.2f} MB   exe {rel_exe} B", flush=True)

    rt = []
    if exe_rel.exists():
        for _ in range(5):
            t = time.perf_counter()
            subprocess.run([str(exe_rel)], capture_output=True, env=ENV)
            rt.append((time.perf_counter() - t) * 1000)
        rt.sort()
        print(f"  release 运行    中位 {rt[2]:.1f} ms", flush=True)

    results.append(dict(variant=variant, dev_build_s=round(dev_s, 2), dev_target_mb=dev_mb,
                        dev_exe_b=dev_exe, mono_instances=n_mono,
                        rel_build_s=round(rel_s, 2), rel_target_mb=rel_mb,
                        rel_exe_b=rel_exe, run_ms=round(rt[2], 1) if rt else None))
    print(flush=True)

print("=" * 74, flush=True)
print("汇总（源码只差 drive 的签名一行）", flush=True)
print("=" * 74, flush=True)
for r in results:
    print(f"  {r['variant']:5s} dev build {r['dev_build_s']:6.2f}s | dev target {r['dev_target_mb']:6.2f} MB"
          f" | dev exe {r['dev_exe_b']:7d} B | rel build {r['rel_build_s']:6.2f}s"
          f" | rel exe {r['rel_exe_b']:8d} B | run {r['run_ms']} ms", flush=True)
if len(results) == 2:
    m, d = results
    print(flush=True)
    print(f"  dyn 相对 mono：dev build {d['dev_build_s']-m['dev_build_s']:+.2f}s "
          f"({(d['dev_build_s']/m['dev_build_s']-1)*100:+.1f}%)  "
          f"dev target {d['dev_target_mb']-m['dev_target_mb']:+.2f} MB  "
          f"rel exe {d['rel_exe_b']-m['rel_exe_b']:+d} B  "
          f"run {d['run_ms']-m['run_ms']:+.1f} ms", flush=True)

(ROOT / "result.json").write_text(json.dumps(results, indent=2, ensure_ascii=False),
                                  encoding="utf-8")
print("\n  结果写入 result.json", flush=True)
