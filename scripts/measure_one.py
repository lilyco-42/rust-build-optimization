"""Measure one Rust project: cargo build --release time + exe size + target dir size."""
import os
import subprocess
import sys
import time

proj = sys.argv[1] if len(sys.argv) > 1 else "ip-base"
root = os.path.dirname(os.path.abspath(__file__))
cwd = os.path.join(root, proj)
exe = os.path.join(cwd, "target", "release", proj + ".exe")

t0 = time.perf_counter()
p = subprocess.run(
    ["cargo", "build", "--release"],
    cwd=cwd,
    stdout=subprocess.PIPE,
    stderr=subprocess.STDOUT,
    text=True,
)
dt = time.perf_counter() - t0

tail = "\n".join(p.stdout.strip().splitlines()[-5:])

exe_size = os.path.getsize(exe) if os.path.exists(exe) else -1

total = 0
tgt = os.path.join(cwd, "target")
for dp, _, fns in os.walk(tgt):
    for f in fns:
        try:
            total += os.path.getsize(os.path.join(dp, f))
        except OSError:
            pass

print(f"=== {proj} ===")
print(f"exit={p.returncode} wall={dt:.1f}s")
print(f"exe={exe_size} bytes ({exe_size / 1048576:.2f} MiB)")
print(f"target_dir={total} bytes ({total / 1073741824:.2f} GiB)")
print("--- cargo tail ---")
print(tail)
