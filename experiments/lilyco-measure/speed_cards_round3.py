"""第三轮：cranelift 的「增量」收益 + 修正体积口径（硬链接去重）。

两个必须回答的问题：
  1. cranelift 冷构建 −50%，但它对**增量**（每天几百次的那个）有没有用？
     如果没用，那它对这个磁盘紧张的用户就不划算 —— 冷构建一个月才几次。
  2. 体积口径修正：旧布局下 cargo 把 rlib 以硬链接放到 target/debug/ 顶层，
     按文件大小累加会把同一份文件算两遍。改用 (st_dev, st_ino) 去重。

流程（每个变体）：
  clean -> check 冷 -> build 冷 -> 改 lilyco-core/src/lib.rs -> check 增量 x3 -> build 增量 x3
"""
import json
import os
import shutil
import subprocess
import time
from pathlib import Path

ROOT = Path("D:/Code/lilyco")
CFG_DIR = ROOT / ".cargo"
CFG = CFG_DIR / "config.toml"
TARGET_SRC = ROOT / "lilyco-core" / "src" / "lib.rs"
ENV_BASE = {k: v for k, v in os.environ.items() if k not in ("BASH_ENV", "ENV")}
SEP = "\x1f"
NO_DEBUG = "-Clink-args=/DEBUG:NONE"
CFG_NO_RUSTFLAGS = '[build]\n# rustc-wrapper = "sccache"\n'


def run(args, env, timeout=1800):
    t = time.perf_counter()
    p = subprocess.run(args, cwd=ROOT, env=env, capture_output=True,
                       text=True, encoding="utf-8", errors="replace", timeout=timeout)
    return time.perf_counter() - t, p.returncode


def sizes():
    """返回 (naive_MB, unique_MB)。unique 按 (dev, inode) 去重硬链接。"""
    t = ROOT / "target"
    if not t.is_dir():
        return 0.0, 0.0
    naive = 0
    seen = set()
    uniq = 0
    for f in t.rglob("*"):
        try:
            if not f.is_file():
                continue
            st = f.stat()
        except OSError:
            continue
        naive += st.st_size
        key = (st.st_dev, st.st_ino)
        if key in seen:
            continue
        seen.add(key)
        uniq += st.st_size
    return round(naive / 1048576, 1), round(uniq / 1048576, 1)


def touch(n):
    with open(TARGET_SRC, "a", encoding="utf-8") as f:
        f.write(f"\n// bench touch {n}\n")


VARIANTS = [
    dict(label="A stable 现行配置", cargo=["cargo"], cfg=None, env={}),
    dict(label="E nightly + cranelift", cargo=["cargo", "+nightly"], cfg=CFG_NO_RUSTFLAGS,
         env={"CARGO_ENCODED_RUSTFLAGS": SEP.join([NO_DEBUG, "-Zcodegen-backend=cranelift"])}),
]


def main():
    orig_cfg = CFG.read_text(encoding="utf-8") if CFG.exists() else None
    orig_src = TARGET_SRC.read_text(encoding="utf-8")
    bak = str(TARGET_SRC) + ".bak-speedcards"
    shutil.copy2(TARGET_SRC, bak)
    results = []
    try:
        for v in VARIANTS:
            print("=" * 78, flush=True)
            print(v["label"], flush=True)
            print("=" * 78, flush=True)
            if v["cfg"] is not None:
                CFG_DIR.mkdir(exist_ok=True)
                CFG.write_text(v["cfg"], encoding="utf-8")
            env = {**ENV_BASE, **v["env"]}
            cargo = v["cargo"]

            run(["cargo", "clean"], env)
            ck, rc = run(cargo + ["check"], env)
            print(f"  check 冷   {ck:6.1f}s rc={rc}", flush=True)
            bd, rc = run(cargo + ["build"], env)
            print(f"  build 冷   {bd:6.1f}s rc={rc}", flush=True)

            ci = []
            for i in range(3):
                touch(f"c{i}")
                dt, rc = run(cargo + ["check"], env)
                ci.append(round(dt, 2))
                print(f"    check 增量 #{i+1}: {dt:5.2f}s", flush=True)
            bi = []
            for i in range(3):
                touch(f"b{i}")
                dt, rc = run(cargo + ["build"], env)
                bi.append(round(dt, 2))
                print(f"    build 增量 #{i+1}: {dt:5.2f}s", flush=True)

            nv, uq = sizes()
            print(f"  target/  朴素 {nv} MB | 硬链接去重后 {uq} MB", flush=True)
            results.append({"label": v["label"],
                            "cold_check_s": round(ck, 1), "cold_build_s": round(bd, 1),
                            "check_inc": ci, "check_inc_median": sorted(ci)[1],
                            "build_inc": bi, "build_inc_median": sorted(bi)[1],
                            "target_naive_mb": nv, "target_unique_mb": uq})
    finally:
        TARGET_SRC.write_text(orig_src, encoding="utf-8")
        if orig_cfg is not None:
            CFG.write_text(orig_cfg, encoding="utf-8")

    print(flush=True)
    print("=" * 78, flush=True)
    print("汇总", flush=True)
    print("=" * 78, flush=True)
    for r in results:
        print(f"  {r['label']:24s} 冷 check {r['cold_check_s']:5.1f}s  冷 build {r['cold_build_s']:5.1f}s"
              f" | 增量 check {r['check_inc_median']:5.2f}s  增量 build {r['build_inc_median']:5.2f}s"
              f" | target 去重 {r['target_unique_mb']:7.1f} MB", flush=True)
    (ROOT / "_speed_cards3.json").write_text(
        json.dumps(results, indent=2, ensure_ascii=False), encoding="utf-8")
    print(f"\n  lib.rs 已还原（备份 {bak}）／config.toml 已还原", flush=True)


if __name__ == "__main__":
    main()
