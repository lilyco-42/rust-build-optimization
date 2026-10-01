# Tauri / Rust 项目硬盘体检工具
#
# 用途：找出是谁在吃你的硬盘（只读，不删任何东西）
# 用法：python tauri_disk_audit.py [扫描根目录]
#   例：python tauri_disk_audit.py D:\Code
#
# 安全说明：本脚本【只读】，不会删除任何文件。
#           清理动作会在报告末尾以命令形式给出，由你手动执行。

import os, sys, subprocess, json

def du(path):
    """返回目录总字节数"""
    tot = 0
    for dp, dn, fn in os.walk(path):
        for f in fn:
            try:
                tot += os.path.getsize(os.path.join(dp, f))
            except OSError:
                pass
    return tot

def mb(b):
    return b / 1048576

def find_dirs(root, name, maxdepth=5):
    """在 root 下找名为 name 的目录（限制深度）"""
    out = []
    root = os.path.abspath(root)
    base_depth = root.rstrip("\\/").count(os.sep)
    for dp, dn, fn in os.walk(root):
        depth = dp.count(os.sep) - base_depth
        if depth > maxdepth:
            dn[:] = []
            continue
        if os.path.basename(dp).lower() == name.lower():
            out.append(dp)
            dn[:] = []   # 不往 target 里面继续走
    return out

def main():
    root = sys.argv[1] if len(sys.argv) > 1 else os.getcwd()
    if not os.path.isdir(root):
        print(f"目录不存在: {root}")
        return

    print("=" * 88)
    print(f"硬盘体检：{root}")
    print("=" * 88)

    # ---------- 1. target/ ----------
    print("\n[1] Rust target/ 目录（Top 20）")
    print("-" * 88)
    targets = find_dirs(root, "target", maxdepth=5)
    rows = []
    for t in targets:
        rows.append((du(t), t))
    rows.sort(reverse=True)
    total_target = sum(r[0] for r in rows)
    for sz, p in rows[:20]:
        print(f"  {mb(sz):>9.1f} MB   {p}")
    print(f"  {'-'*60}")
    print(f"  {mb(total_target):>9.1f} MB   合计（{len(rows)} 个 target）")

    # ---------- 2. node_modules ----------
    print("\n[2] node_modules 目录（Top 10）")
    print("-" * 88)
    nms = find_dirs(root, "node_modules", maxdepth=5)
    nrows = sorted(((du(n), n) for n in nms), reverse=True)
    total_nm = sum(r[0] for r in nrows)
    for sz, p in nrows[:10]:
        print(f"  {mb(sz):>9.1f} MB   {p}")
    print(f"  {'-'*60}")
    print(f"  {mb(total_nm):>9.1f} MB   合计（{len(nrows)} 个 node_modules）")

    # ---------- 3. cargo 全局缓存 ----------
    print("\n[3] cargo 全局缓存")
    print("-" * 88)
    cargo_home = os.environ.get("CARGO_HOME") or os.path.expanduser("~/.cargo")
    for sub in ("registry", "git"):
        p = os.path.join(cargo_home, sub)
        if os.path.isdir(p):
            print(f"  {mb(du(p)):>9.1f} MB   {p}")

    # ---------- 4. sccache ----------
    print("\n[4] sccache 缓存")
    print("-" * 88)
    try:
        out = subprocess.run(["sccache", "--show-stats"], capture_output=True,
                             text=True, timeout=15).stdout
        for ln in out.splitlines():
            if any(k in ln for k in ("Cache size", "Max cache size", "Cache location")):
                print("  ", ln.strip())
    except Exception as e:
        print(f"   (读不到: {e})")

    # ---------- 5. 汇总 ----------
    grand = total_target + total_nm
    print("\n" + "=" * 88)
    print("汇总")
    print("=" * 88)
    print(f"  target/       {mb(total_target):>9.1f} MB")
    print(f"  node_modules  {mb(total_nm):>9.1f} MB")
    print(f"  ----------------")
    print(f"  可回收合计    {mb(grand):>9.1f} MB  ({mb(grand)/1024:.2f} GB)")
    print()
    print("优化提示：")
    print("  · 每个 Tauri target/ 默认 4.1 GB，加 [profile.dev] 优化后可降到 1.5 GB")
    print("  · 不再维护的项目直接 cargo clean（不是手动 rmtree）")
    print("  · cargo cache --autoclean 清理全局旧版 crate 源码")

if __name__ == "__main__":
    main()
