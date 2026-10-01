"""第四轮（定稿）：隔离 target 目录，消除第三轮的两个实验设计缺陷。

第三轮的教训：
  X1 三个工具链共用一个 target/ —— stable 与 nightly 产物布局不同（nightly 1.100
     已默认启用新的 build-dir 布局，连 deps/ 都不存在），cargo clean 清不干净，互相污染。
     → 本轮每个变体给独立的 CARGO_TARGET_DIR。
  X2 体积是在 6 轮增量之后测的 —— 把 target/debug/incremental/ 的历史也算进去了
     （这就是 stable 从 935 MB 涨到 1566 MB 的原因）。
     → 本轮体积只在「冷构建刚结束」时测。

另外修正一个口径错误：旧布局下 cargo 把 rlib 以硬链接放到 target/debug/ 顶层，
按文件大小累加会把同一份文件算两遍 → 用 (st_dev, st_ino) 去重。
"""
import json
import os
import shutil
import subprocess
import time
from pathlib import Path

ROOT = Path("D:/Code/lilyco")
CFG = ROOT / ".cargo" / "config.toml"
SRC = ROOT / "lilyco-core" / "src" / "lib.rs"
BASEDIR = Path("D:/Code/_wbspeed")
ENVBASE = {k: v for k, v in os.environ.items() if k not in ("BASH_ENV", "ENV")}
SEP = "\x1f"
NO_DEBUG = "-Clink-args=/DEBUG:NONE"
CFG_NONE = '[build]\n# rustc-wrapper = "sccache"\n'


def run(args, env, timeout=2400):
    t = time.perf_counter()
    p = subprocess.run(args, cwd=ROOT, env=env, capture_output=True,
                       text=True, encoding="utf-8", errors="replace", timeout=timeout)
    return time.perf_counter() - t, p.returncode, (p.stdout or "") + (p.stderr or "")


def sizes(tdir):
    naive, seen, uniq = 0, set(), 0
    for f in tdir.rglob("*"):
        try:
            if not f.is_file():
                continue
            st = f.stat()
        except OSError:
            continue
        naive += st.st_size
        k = (st.st_dev, st.st_ino)
        if k in seen:
            continue
        seen.add(k)
        uniq += st.st_size
    return round(naive / 1048576, 1), round(uniq / 1048576, 1)


def touch(n):
    with open(SRC, "a", encoding="utf-8") as f:
        f.write(f"\n// bench4 {n}\n")


def clean_tdir(cargo, env):
    """用 cargo clean 清空隔离 target（不用 rmtree：会被 safe-delete 拦）。"""
    subprocess.run(cargo + ["clean"], cwd=ROOT, env=env, capture_output=True)


VARIANTS = [
    dict(label="A stable 现行配置", cargo=["cargo"], cfg=None, extra={}),
    dict(label="E nightly + cranelift", cargo=["cargo", "+nightly"], cfg=CFG_NONE,
         extra={"CARGO_ENCODED_RUSTFLAGS": SEP.join([NO_DEBUG, "-Zcodegen-backend=cranelift"])}),
]


def main():
    orig_cfg = CFG.read_text(encoding="utf-8")
    orig_src = SRC.read_text(encoding="utf-8")
    shutil.copy2(SRC, str(SRC) + ".bak-round4")
    results = []
    try:
        for v in VARIANTS:
            label = v["label"]
            tdir = BASEDIR / label.split()[0]          # A / E，按变体隔离
            tdir.mkdir(parents=True, exist_ok=True)
            print("=" * 78, flush=True)
            print(f"{label}   (target: {tdir})", flush=True)
            print("=" * 78, flush=True)

            if v["cfg"] is not None:
                CFG.write_text(v["cfg"], encoding="utf-8")
            env = {**ENVBASE, **v["extra"], "CARGO_TARGET_DIR": str(tdir)}
            cargo = v["cargo"]

            clean_tdir(cargo, env)
            ck, rc, out = run(cargo + ["check"], env)
            print(f"  check 冷（空 target）        {ck:6.1f}s rc={rc}", flush=True)

            clean_tdir(cargo, env)
            bd, rc, out = run(cargo + ["build"], env)
            print(f"  build 冷（空 target）        {bd:6.1f}s rc={rc}", flush=True)

            nv, uq = sizes(tdir)
            print(f"  target/ 朴素 {nv} MB | 去重 {uq} MB   ← 冷构建刚结束", flush=True)

            run(cargo + ["check"], env)               # 预热 rmeta
            ci = []
            for i in range(3):
                touch(f"c{i}")
                dt, rc, _ = run(cargo + ["check"], env)
                ci.append(round(dt, 2))
                print(f"    check 增量 #{i+1}: {dt:5.2f}s", flush=True)
            bi = []
            for i in range(3):
                touch(f"b{i}")
                dt, rc, _ = run(cargo + ["build"], env)
                bi.append(round(dt, 2))
                print(f"    build 增量 #{i+1}: {dt:5.2f}s", flush=True)

            results.append({"label": label,
                            "cold_check_s": round(ck, 1), "cold_build_s": round(bd, 1),
                            "target_naive_mb": nv, "target_unique_mb": uq,
                            "check_inc": ci, "check_inc_median": sorted(ci)[1],
                            "build_inc": bi, "build_inc_median": sorted(bi)[1]})
    finally:
        SRC.write_text(orig_src, encoding="utf-8")
        CFG.write_text(orig_cfg, encoding="utf-8")

    print(flush=True)
    print("=" * 78, flush=True)
    print("定稿汇总（隔离 target，冷构建后立即测体积）", flush=True)
    print("=" * 78, flush=True)
    for r in results:
        print(f"  {r['label']:24s} 冷 check {r['cold_check_s']:5.1f}s  冷 build {r['cold_build_s']:5.1f}s"
              f" | 增量 check {r['check_inc_median']:5.2f}s  增量 build {r['build_inc_median']:5.2f}s"
              f" | target 去重 {r['target_unique_mb']:7.1f} MB", flush=True)
    if len(results) == 2:
        a, e = results
        print(flush=True)
        print(f"  cranelift 相对现行：冷 check {e['cold_check_s']-a['cold_check_s']:+.1f}s  "
              f"冷 build {e['cold_build_s']-a['cold_build_s']:+.1f}s  "
              f"增量 check {e['check_inc_median']-a['check_inc_median']:+.2f}s  "
              f"增量 build {e['build_inc_median']-a['build_inc_median']:+.2f}s  "
              f"target {e['target_unique_mb']-a['target_unique_mb']:+.1f} MB", flush=True)

    (ROOT / "_speed_cards4.json").write_text(
        json.dumps(results, indent=2, ensure_ascii=False), encoding="utf-8")
    print("\n  lib.rs / .cargo/config.toml 已还原", flush=True)

    for d in BASEDIR.iterdir():
        subprocess.run(["cargo", "clean"], cwd=ROOT,
                       env={**ENVBASE, "CARGO_TARGET_DIR": str(d)},
                       capture_output=True)
    print(f"  已 cargo clean 掉 {BASEDIR} 下的隔离 target", flush=True)


if __name__ == "__main__":
    main()
