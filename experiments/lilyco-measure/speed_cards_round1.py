"""lilyco 上一轮「还没打过的牌」单变量对照。

四张牌：
  A 现行为基准（stable）
  B lld-link 链接器      —— 只影响链接阶段 → 只看 cargo build
  C nightly 基准（对照）  —— 给 D/E 做控制组
  D nightly + 并行前端    —— 只影响前端（parse/typecheck）→ 主要看 cargo check
  E nightly + cranelift  —— 只影响 codegen → 只看 cargo build

单变量纪律：每轮只改一件事，其余全部冻结；每轮都 cargo clean 后冷构建。
"""
import json
import os
import re
import subprocess
import time
from pathlib import Path

ROOT = Path("D:/Code/lilyco")
CFG_DIR = ROOT / ".cargo"
CFG = CFG_DIR / "config.toml"
ENV_BASE = {k: v for k, v in os.environ.items() if k not in ("BASH_ENV", "ENV")}
SEP = "\x1f"  # CARGO_ENCODED_RUSTFLAGS 的分隔符

NO_DEBUG = "-Clink-args=/DEBUG:NONE"

CFG_NO_RUSTFLAGS = """[build]
# rustc-wrapper = "sccache"
"""


def cfg_with(linker=None):
    """A/B 共用：都保留 /DEBUG:NONE，B 只是额外换链接器 —— 保证单变量。"""
    lines = ["[target.x86_64-pc-windows-msvc]"]
    lines.append('rustflags = ["-C", "link-args=/DEBUG:NONE"]')
    if linker:
        lines.append(f'linker = "{linker}"')
    lines.append("")
    lines.append("[build]")
    lines.append("# rustc-wrapper = \"sccache\"")
    return "\n".join(lines) + "\n"


def run(args, env, timeout=1800):
    t = time.perf_counter()
    p = subprocess.run(args, cwd=ROOT, env=env, capture_output=True,
                       text=True, encoding="utf-8", errors="replace", timeout=timeout)
    return time.perf_counter() - t, p.returncode, (p.stdout or "") + (p.stderr or "")


def target_mb():
    p = ROOT / "target"
    if not p.is_dir():
        return 0.0
    return sum(f.stat().st_size for f in p.rglob("*") if f.is_file()) / 1048576


VARIANTS = [
    dict(key="A 现行配置（stable 基准）", cargo=["cargo"], cfg=cfg_with(),
         env={}),
    dict(key="B stable + lld-link", cargo=["cargo"], cfg=cfg_with(linker="lld-link"),
         env={}),
    dict(key="C nightly 基准（对照）", cargo=["cargo", "+nightly"], cfg=CFG_NO_RUSTFLAGS,
         env={"CARGO_ENCODED_RUSTFLAGS": NO_DEBUG}),
    dict(key="D nightly + 并行前端 x8", cargo=["cargo", "+nightly"], cfg=CFG_NO_RUSTFLAGS,
         env={"CARGO_ENCODED_RUSTFLAGS": SEP.join([NO_DEBUG, "-Zunstable-options",
                                                   "-Zjobs-frontend=8"])}),
    dict(key="E nightly + cranelift 后端", cargo=["cargo", "+nightly"], cfg=CFG_NO_RUSTFLAGS,
         env={"CARGO_ENCODED_RUSTFLAGS": SEP.join([NO_DEBUG, "-Zcodegen-backend=cranelift"])}),
]


def main():
    orig_cfg = CFG.read_text(encoding="utf-8") if CFG.exists() else None
    results = []
    try:
        for v in VARIANTS:
            print("=" * 78, flush=True)
            print(v["key"], flush=True)
            print("=" * 78, flush=True)

            CFG_DIR.mkdir(exist_ok=True)
            CFG.write_text(v["cfg"], encoding="utf-8")
            env = dict(ENV_BASE)
            env.update(v["env"])

            run(["cargo", "clean"], env)
            dt, rc, out = run(v["cargo"] + ["check"], env)
            warn = [l for l in out.splitlines() if l.strip().startswith("warning:")]
            n = len(re.findall(r"^\s+Checking\s+", out, re.M))
            print(f"  check 冷   {dt:6.1f}s  rc={rc}  Checking {n} 行", flush=True)
            if rc != 0:
                print("  !! check 失败，尾部输出：", flush=True)
                print("     " + "\n     ".join(out.splitlines()[-6:]), flush=True)
            for w in warn[:2]:
                print(f"  warn> {w}", flush=True)
            check_s = dt if rc == 0 else None

            run(["cargo", "clean"], env)
            dt, rc, out = run(v["cargo"] + ["build"], env)
            n = len(re.findall(r"^\s+Compiling\s+", out, re.M))
            print(f"  build 冷   {dt:6.1f}s  rc={rc}  Compiling {n} 行", flush=True)
            if rc != 0:
                print("  !! build 失败，尾部输出：", flush=True)
                print("     " + "\n     ".join(out.splitlines()[-6:]), flush=True)
            build_s = dt if rc == 0 else None

            mb = target_mb()
            print(f"  target/ {mb:.1f} MB", flush=True)
            print(flush=True)

            results.append({"variant": v["key"],
                            "check_cold_s": round(check_s, 1) if check_s else None,
                            "build_cold_s": round(build_s, 1) if build_s else None,
                            "target_mb": round(mb, 1)})
    finally:
        if orig_cfg is not None:
            CFG.write_text(orig_cfg, encoding="utf-8")

    print("=" * 78, flush=True)
    print("汇总", flush=True)
    print("=" * 78, flush=True)
    base_c = next((r["check_cold_s"] for r in results if r["variant"].startswith("A")), None)
    base_b = next((r["build_cold_s"] for r in results if r["variant"].startswith("A")), None)
    for r in results:
        c = f"{r['check_cold_s']:6.1f}s" if r["check_cold_s"] else "  FAIL "
        b = f"{r['build_cold_s']:6.1f}s" if r["build_cold_s"] else "  FAIL "
        print(f"  {r['variant']:32s} check {c}  build {b}  target {r['target_mb']:7.1f} MB",
              flush=True)
    print(flush=True)
    for r in results:
        if r["check_cold_s"] and base_c:
            print(f"  check vs A: {r['variant']:32s} {r['check_cold_s']-base_c:+6.1f}s", flush=True)
        if r["build_cold_s"] and base_b:
            print(f"  build vs A: {r['variant']:32s} {r['build_cold_s']-base_b:+6.1f}s", flush=True)

    (ROOT / "_speed_cards.json").write_text(
        json.dumps(results, indent=2, ensure_ascii=False), encoding="utf-8")
    print("\n  Cargo.toml 未改；.cargo/config.toml 已还原；结果写 _speed_cards.json", flush=True)


if __name__ == "__main__":
    main()
