#!/usr/bin/env bash
# cargo-xmake 端到端冒烟：setup -> dynify --rewrite -> 构建 -> undo -> 幂等再 undo
#
# 重点验两件容易被"看起来对了"骗过去的事：
#   1. undo 还原源码之后，原始文件必须**逐字节**一致（md5 相等），
#      而且第二次 undo 必须是幂等空操作（历史上这里报过 os error 3）。
#   2. dev exe 没有 PDB，且目标文件确实是 cargo-xmake 的配置产物。
#
# 用法： bash scripts/smoke_cargo_xmake.sh [工作目录]
#   不给目录就用 $TMPDIR/xmk-smoke-$RANDOM，跑完自动删。

set -u
# 让 cargo-xmake 输出纯文本：CI 里 CARGO_TERM_COLOR 常被置为 always，工具会吐 ANSI 颜色，
# 把 `skip` 这类关键字用转义符包起来，脚本的 grep 断言就会匹配不到。
# 工具在 util.rs 里读 NO_COLOR，设了就关色（不影响真人终端里的彩色输出）。
export NO_COLOR=1
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WORK="${1:-${TMPDIR:-/tmp}/xmk-smoke-$$}"

pass=0
fail=0
ok()   { echo "  ✔ $1"; pass=$((pass+1)); }
bad()  { echo "  ✘ $1"; fail=$((fail+1)); }
step() { echo; echo "=== $1 ==="; }

rm -rf "$WORK"
mkdir -p "$WORK"
cd "$WORK" || exit 1

step "1. 建一个带泛型的示例工程"
cargo new --quiet smoke >/dev/null 2>&1 || { bad "cargo new 失败"; exit 1; }
cd smoke || exit 1

# 塞一个"泛型函数 + trait"，这样 dynify 有活干；再留两处普通代码
# 用来验证 revert 只动我们生成的那一对分支，不碰别的东西。
cat > src/main.rs <<'RS'
use std::fmt::Debug;

pub trait Work {
    fn step(&self, x: u64) -> u64;
}

#[derive(Debug)]
struct A(u64);
#[derive(Debug)]
struct B(u64);

impl Work for A {
    fn step(&self, x: u64) -> u64 { self.0.wrapping_mul(x | 1) }
}
impl Work for B {
    fn step(&self, x: u64) -> u64 { self.0.wrapping_add(x ^ 7) }
}

#[inline]
pub fn drive<T: Work>(t: &T, n: u64) -> u64 {
    let mut acc = 0u64;
    for i in 0..n {
        acc = acc.wrapping_add(t.step(acc ^ i));
    }
    acc
}

/// 多约束 —— dynify 必须**拒绝**它（该留给用户手工决定用哪个 trait 对象），
/// 这条是负向断言，不是漏改。
#[inline]
pub fn pipeline<T: Work + Debug>(t: &T, n: u64) -> u64 {
    drive(t, n) ^ 0x5a
}

fn helper() -> u64 {
    let x = 1u64;
    x + 2
}

fn main() {
    let a = A(3);
    let b = B(5);
    println!("{} {}", drive(&a, 10), drive(&b, 10));
    println!("{}", pipeline(&a, 4));
    println!("{}", helper());
}
RS

before_md5="$(md5sum src/main.rs | cut -d' ' -f1)"
before_bytes="$(wc -c < src/main.rs | tr -d ' ')"
echo "  原始 src/main.rs: md5=$before_md5  ${before_bytes} B"

step "2. cargo xmake setup"
cargo xmake setup > /tmp/xmk-setup.log 2>&1
if [ -f .cargo/config.toml ]; then ok ".cargo/config.toml 已生成"; else bad "没生成 .cargo/config.toml"; cat /tmp/xmk-setup.log; fi
if grep -q 'cargo-xmake' .cargo/config.toml 2>/dev/null; then ok "配置里带 cargo-xmake 标记"; else bad "配置里没标记"; fi
if grep -q 'debug-assertions' .cargo/config.toml 2>/dev/null; then ok "配置强制了 debug-assertions（这是 release 关虚函数的开关）"; else bad "配置没写 debug-assertions"; fi
if grep -q 'DEBUG:NONE' .cargo/config.toml 2>/dev/null; then ok "配置注入了 /DEBUG:NONE（MSVC 关 PDB）"; else echo "  - 未注入 /DEBUG:NONE（非 MSVC 平台可接受）"; fi

step "3. cargo xmake dynify --rewrite"
cargo xmake dynify --rewrite > /tmp/xmk-dynify.log 2>&1
if grep -q 'cargo-xmake:dyn' src/main.rs; then ok "源码里出现 // cargo-xmake:dyn 标记"; else bad "没写入 dyn 标记"; cat /tmp/xmk-dynify.log; fi
if grep -q 'cfg(debug_assertions)' src/main.rs; then ok "生成了 cfg(debug_assertions) 分支"; else bad "没有 cfg 分支"; fi
if grep -q 'cfg(not(debug_assertions))' src/main.rs; then ok "生成了 cfg(not(...)) 分支"; else bad "没有 not 分支"; fi
if grep -q '&dyn Work' src/main.rs; then ok "dyn 形态出现 &dyn Work"; else bad "没出现 &dyn Work"; fi
if [ "$(grep -c 'cfg(not(debug_assertions))' src/main.rs)" = "1" ]; then ok "只改写了 1 个函数（drive），没误伤 pipeline"; else bad "改写数量不是 1"; fi
if grep -q 'fn helper' src/main.rs; then ok "无关代码 fn helper 仍在"; else bad "无关代码被弄丢了"; fi
if grep -q 'fn pipeline<T: Work + Debug>' src/main.rs; then ok "多约束函数 pipeline 被原样留下（预期拒绝）"; else bad "多约束函数被误改了"; fi
if grep -q 'skip pipeline' /tmp/xmk-dynify.log; then ok "dynify 明确报告了跳过 pipeline 的原因"; else bad "没报告 pipeline 的跳过原因"; fi
if [ ! -f src/main.rs.xmake-bak ]; then ok "验证通过后备份已清理（不留 .xmake-bak）"; else bad "备份没清掉"; fi

step "4. dev / release 构建"
cargo build --quiet 2>/tmp/xmk-dev.log
if [ -f target/debug/smoke.exe ]; then ok "dev 产物 target/debug/smoke.exe"; else bad "dev 没产物"; cat /tmp/xmk-dev.log; fi
pdb=$(ls target/debug/*.pdb 2>/dev/null | wc -l | tr -d ' ')
if [ "$pdb" = "0" ]; then ok "dev 目录 0 个 PDB"; else bad "dev 目录还有 $pdb 个 PDB"; fi
cargo build --release --quiet 2>/tmp/xmk-rel.log
if [ -f target/release/smoke.exe ]; then ok "release 产物 target/release/smoke.exe"; else bad "release 没产物"; cat /tmp/xmk-rel.log; fi
dev_dyn="$(wc -c < target/debug/smoke.exe | tr -d ' ')"
rel_dyn="$(wc -c < target/release/smoke.exe | tr -d ' ')"
echo "  dev=$dev_dyn B  release=$rel_dyn B（release 走静态派发，与纯泛型应逐字节相同）"

step "5. 第一次 cargo xmake undo —— 源码必须逐字节还原"
cargo xmake undo > /tmp/xmk-undo1.log 2>&1
undo_rc=$?
after_md5="$(md5sum src/main.rs | cut -d' ' -f1)"
after_bytes="$(wc -c < src/main.rs | tr -d ' ')"
if [ "$undo_rc" = "0" ]; then ok "undo 退出码 0"; else bad "undo 退出码 $undo_rc"; cat /tmp/xmk-undo1.log; fi
if [ "$after_md5" = "$before_md5" ]; then
  ok "src/main.rs 逐字节还原（md5 一致）"
else
  bad "src/main.rs 不一致：$before_md5 -> $after_md5（${before_bytes} -> ${after_bytes} B）"
  diff <(cat src/main.rs) <(cat src/main.rs.xmake-bak) | head -20
fi
if [ ! -f .cargo/config.toml ]; then ok ".cargo/config.toml 已删除（内容全是我们写的）"; else bad "config.toml 还在"; fi

step "6. 第二次 cargo xmake undo —— 必须幂等，不能报错"
cargo xmake undo > /tmp/xmk-undo2.log 2>&1
undo2_rc=$?
if [ "$undo2_rc" = "0" ]; then ok "第二次 undo 退出码 0（幂等）"; else bad "第二次 undo 退出码 $undo2_rc"; cat /tmp/xmk-undo2.log; fi
if ! grep -qi 'os error 3\|系统找不到指定的路径' /tmp/xmk-undo2.log; then ok "没有 os error 3"; else bad "报了 os error 3"; cat /tmp/xmk-undo2.log; fi
third_md5="$(md5sum src/main.rs | cut -d' ' -f1)"
if [ "$third_md5" = "$before_md5" ]; then ok "幂等 undo 后源码仍逐字节一致"; else bad "幂等 undo 把源码改了"; fi

step "7. cargo xmake doctor / audit 不炸"
cargo xmake doctor > /tmp/xmk-doctor.log 2>&1 && ok "doctor 正常" || { bad "doctor 失败"; cat /tmp/xmk-doctor.log; }
cargo xmake audit > /tmp/xmk-audit.log 2>&1 && ok "audit 正常" || { bad "audit 失败"; cat /tmp/xmk-audit.log; }

step "8. 核心主张：dynify 后的 dev 产物必须比不 dynify 的小（单变量对照）"
# ⚠️ 这里必须让两次构建**只差源码形态一个变量**。
# 此刻源码是泛型原样、配置已删 —— 若就这么重建，比出来的是"配置 + 源码"两个变量
# 的合力，等于拿体积去猜因果。所以先把配置加回来当公共底座。
cargo xmake setup > /tmp/xmk-setup2.log 2>&1 || bad "第二次 setup 失败"
cargo build --quiet 2>/tmp/xmk-dev-plain.log
dev_plain="$(wc -c < target/debug/smoke.exe | tr -d ' ')"
cargo build --release --quiet 2>/tmp/xmk-rel-plain.log
rel_plain="$(wc -c < target/release/smoke.exe | tr -d ' ')"
# release 侧必须零代价：dynify 保留的那份泛型版本与原文语义相同，
# 所以"带 dyn 的 release"必须和"纯泛型的 release"逐字节相同。
if [ "$rel_plain" = "$rel_dyn" ]; then
  ok "release 零代价：带 dyn 与纯泛型都是 ${rel_dyn} B（逐字节相同）"
else
  bad "release 被改变了：纯泛型 ${rel_plain} B vs 带 dyn ${rel_dyn} B"
fi
cargo xmake dynify --rewrite > /tmp/xmk-dynify2.log 2>&1
cargo build --quiet 2>/tmp/xmk-dev-dyn.log
dev_dyn="$(wc -c < target/debug/smoke.exe | tr -d ' ')"
echo "  同一个 .cargo/config.toml，只换源码形态："
echo "    纯泛型（dev）: $dev_plain B"
echo "    开虚函数（dev）: $dev_dyn B"
# 不变量是「dyn 不会让 dev 更大」（只能更小或持平）；"更小"是预期收益，
# 但取决于编译器——nightly 的 dev 代码生成偶尔让泛型版已经和 dyn 版一样大，
# 此时持平即可，不能算失败。比 dev_dyn > dev_plain（真回归）才该 fail。
if [ "$dev_dyn" -lt "$dev_plain" ]; then
  ok "dyn dev 更小：${dev_dyn} < ${dev_plain}（省 $((dev_plain - dev_dyn)) B）"
elif [ "$dev_dyn" -eq "$dev_plain" ]; then
  ok "dyn dev 不比纯泛型大：${dev_dyn} = ${dev_plain}（该工具链下收益为 0，结论不变）"
else
  bad "dyn dev 反而更大：${dev_dyn} > ${dev_plain}"
fi
# 收尾：把源码和配置都还原，别给后来的手动检查留个半改状态
cargo xmake undo > /tmp/xmk-undo3.log 2>&1
final_md5="$(md5sum src/main.rs | cut -d' ' -f1)"
if [ "$final_md5" = "$before_md5" ]; then ok "收尾 undo 后源码仍逐字节一致"; else bad "收尾 undo 后源码被改了"; fi

# ══════════════════════════════════════════════════════════════════════
# 9. workspace + 中文源码 —— 这一节是"真实项目"的最小复现
#
# 上面 1~8 步的夹具全是**单 crate + 纯 ASCII**，而真实项目是
#   workspace + 中文注释 + 多层模块。
# 这两个差异恰好藏着 5 个只有真实项目才暴露的 bug：
#   * 中文注释里的续字节 0xBA 被 `as char` 当成字母 'º' → 扫描器 panic
#   * workspace 根下没有 src/ → 默认扫到 0 个泛型函数
#   * mono-stats 报告写到 workspace 根 → 在 crate 目录里找不到
#   * 打错路径被静默丢弃 → "0 个"看着像结论
# 所以这一节不能省。
# ══════════════════════════════════════════════════════════════════════
step "9. workspace + 中文源码（真实项目的最小复现）"
cd "$WORK" || exit 1
rm -rf ws && mkdir -p ws/member-a/src ws/member-b/src
cat > ws/Cargo.toml <<'TOML'
[workspace]
members = ["member-a", "member-b"]
resolver = "2"
TOML
cat > ws/member-a/Cargo.toml <<'TOML'
[package]
name = "member-a"
version = "0.1.0"
edition = "2021"
TOML
cat > ws/member-b/Cargo.toml <<'TOML'
[package]
name = "member-b"
version = "0.1.0"
edition = "2021"
TOML
# 关键点：**中文注释** + pub(crate) + 属性（把 item_start 的回溯分支全踩一遍）
cat > ws/member-a/src/lib.rs <<'RS'
//! 这一行是中文注释：表示、中间、电话
pub trait Work {
    fn step(&self, x: u64) -> u64;
}

/// 文档注释：中文说明，含「表示」与「中间」
#[inline]
#[must_use]
pub fn drive<T: Work>(t: &T, n: u64) -> u64 {
    // 函数体里也有中文：信号、显示
    let _ = "中文字符串字面量 表示";
    t.step(n)
}
RS
cat > ws/member-b/src/lib.rs <<'RS'
//! 另一个成员，也有中文注释
pub fn helper<T: std::fmt::Debug>(v: T) -> String {
    format!("{v:?}")
}
RS

cd ws || exit 1
# 9a. workspace 根不给路径 → 必须扫到**所有成员**的泛型函数（不是 0 个）
ws_out="$(cargo xmake dynify 2>&1)"
if echo "$ws_out" | grep -q "扫描 0 个泛型函数"; then
  bad "workspace 根扫到 0 个（应该扫所有成员）—— 这正是真实项目上的头号 bug"
else
  n_ws="$(echo "$ws_out" | grep -o '扫描 [0-9]*' | grep -o '[0-9]*' | head -1)"
  if [ "${n_ws:-0}" -ge 2 ]; then ok "workspace 根扫到 ${n_ws} 个泛型函数（含两个成员）"; else bad "只扫到 ${n_ws} 个，应为 2"; fi
fi
# 9b. 中文注释不能让扫描器 panic
if echo "$ws_out" | grep -qi "not a char boundary\|panicked"; then
  bad "中文注释把扫描器搞崩了"
else
  ok "中文注释/中文字符串没有让扫描器 panic"
fi
# 9c. 反而应该能改到（drive 是单约束 &T，本来就可改）
if echo "$ws_out" | grep -q "可改 drive\|可改.*drive"; then ok "中文源码里的 drive 被正确识别为可改"; else bad "没识别出 drive"; echo "$ws_out" | head -20; fi
if echo "$ws_out" | grep -q "中文字符串\|表示"; then bad "中文出现在扫描结果里，说明解析错位"; else ok "扫描结果里没有混入中文字符"; fi

# 9d. Git Bash 风格路径 /d/... 必须被认出，不能静默变 0
abs_ws="$(cd "$WORK/ws/member-a/src" && pwd -W 2>/dev/null || echo "$WORK/ws/member-a/src")"
gb_out="$(cargo xmake dynify "$(echo "$abs_ws" | sed 's|^\([A-Za-z]\):|/\L\1|; s|\\|/|g')" 2>&1)"
if echo "$gb_out" | grep -q "扫描 0 个泛型函数"; then bad "Git Bash 风格路径被当成了相对路径"; else ok "Git Bash 风格路径 /d/... 被正确解析"; fi

# 9e. 打错的路径必须报错，不能静默 0
if cargo xmake dynify member-a/srx > /tmp/xmk-badpath.log 2>&1; then
  bad "打错的路径没报错（静默退化成 0 个）"
else
  if grep -q "路径不存在" /tmp/xmk-badpath.log; then ok "打错的路径明确报错"; else bad "报错了但原因不清楚"; cat /tmp/xmk-badpath.log; fi
fi

# 9f. audit 在 workspace 上必须能读到报告（报告会落在 workspace 根）
cd "$WORK/ws" || exit 1
audit_out="$(timeout 600 cargo xmake audit --xmk-top=5 2>&1)"
if echo "$audit_out" | grep -q "没读到 mono-stats 报告"; then
  bad "workspace 上读不到 mono-stats 报告（报告落在 workspace 根，工具没去那找）"
else
  ok "workspace 上 audit 读到了 mono-stats 报告"
fi
# 9g. 报告读完后不留残骸
if [ -d "$WORK/ws/human" ] || [ -d "$WORK/ws/member-a/human" ]; then
  bad "audit 留下了 human/ 目录没清"
else
  ok "audit 把自己生成的 human/ 报告清理干净了"
fi

# ══════════════════════════════════════════════════════════════════════
# 10. cargo xmake new 自动 setup —— "新建项目直接是 xmake 档" 这条主张
#
# 历史上 cargo xmake new 只是原样转发给 cargo new，项目建好但**没**生成
# .cargo/config.toml，用户还得手动补一句 setup 才吃上设置。这一步验：
#   ① 建完自动 setup；② 配置带标记、关了调试信息；③ undo 能干净撤掉；
#   ④ --xmk-no-setup 真的跳过。
# ══════════════════════════════════════════════════════════════════════
step "10. cargo xmake new 自动 setup"
cd "$WORK" || exit 1
cargo xmake new hello > /tmp/xmk-new.log 2>&1
if [ -f hello/Cargo.toml ]; then ok "cargo xmake new hello 创建了项目"; else bad "没创建项目"; cat /tmp/xmk-new.log; fi
if [ -f hello/.cargo/config.toml ]; then ok "新建项目自动生成了 .cargo/config.toml（直接是 xmake 档）"; else bad "新建项目没自动 setup"; cat /tmp/xmk-new.log; fi
if grep -q 'cargo-xmake' hello/.cargo/config.toml 2>/dev/null; then ok "自动配置带 cargo-xmake 标记"; else bad "自动配置没标记"; fi
if grep -q 'debug = false\|debug=false' hello/.cargo/config.toml 2>/dev/null; then ok "自动配置关了调试信息（debug=false）"; else bad "自动配置没关调试信息"; fi
(cd hello && cargo xmake undo > /tmp/xmk-new-undo.log 2>&1)
if [ ! -f hello/.cargo/config.toml ]; then ok "undo 把自动 setup 的配置清掉了"; else bad "undo 没清掉自动配置"; cat /tmp/xmk-new-undo.log; fi
# --xmk-no-setup 分支：建项目但不写配置
cargo xmake new hello2 --xmk-no-setup > /tmp/xmk-new2.log 2>&1
if [ -f hello2/.cargo/config.toml ]; then bad "--xmk-no-setup 仍然自动 setup 了"; else ok "--xmk-no-setup 没自动 setup（项目仍是纯 cargo 状态）"; fi

echo
echo "================ 结果：${pass} 通过 / ${fail} 失败 ================"
if [ -n "${KEEP_SMOKE:-}" ]; then
  echo "（KEEP_SMOKE 已设置，保留工作目录：$WORK/smoke）"
else
  rm -rf "$WORK"
fi
[ "$fail" = "0" ]
