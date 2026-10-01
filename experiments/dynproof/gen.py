"""生成「泛型单态化 vs 动态分发」的最小对照工程。

单变量纪律：两个 crate 的源码**逐字节相同**，只有 drive() 的分发方式和签名不同。
  mono/  pub fn drive<T: Work>(t: &T, n: u64) -> u64     → N 份单态化副本
  dyn/   pub fn drive(t: &dyn Work, n: u64) -> u64       → 1 份 + N 个 vtable

其余一律冻结：类型数、每个类型的 step 函数体、drive 的循环体、调用点数量、
profile 配置（dev: opt-level=0 / debug=false / incremental=false / cgu=1 / panic=abort）。
"""
import io
import os

N_TYPES = int(os.environ.get("DYNPROOF_N", "60"))
OUT = os.path.dirname(os.path.abspath(__file__))

PROFILE = """[profile.dev]
opt-level = 0
debug = false
incremental = false
codegen-units = 1
panic = "abort"
strip = "debuginfo"
overflow-checks = false
debug-assertions = false

[profile.release]
opt-level = 3
lto = "fat"
codegen-units = 1
panic = "abort"
strip = "symbols"
incremental = false
"""


def types_src():
    out = []
    for i in range(N_TYPES):
        k = (i * 0x9E3779B9 + 0x85EBCA6B) & 0xFFFFFFFF
        out.append(f"pub struct T{i};")
        out.append(
            f"impl Work for T{i} {{\n"
            f"    fn step(&self, x: u64) -> u64 {{\n"
            f"        let mut a = x ^ {k:#x}_u64;\n"
            f"        a = a.wrapping_mul(0x100000001B3);\n"
            f"        a ^= a >> 29;\n"
            f"        a = a.wrapping_add({i}_u64);\n"
            f"        a.rotate_left(13)\n"
            f"    }}\n"
            f"}}\n"
        )
    return "\n".join(out)


DRIVE_BODY = """
    let mut acc = 0u64;
    for i in 0..n {
        acc = acc.wrapping_mul(31).wrapping_add(t.step(acc ^ i));
        acc ^= acc >> 13;
        if acc % 7 == 3 {
            acc = acc.rotate_left(5);
        }
        acc = acc.wrapping_add(i & 0xFF);
        if acc & 1 == 0 {
            acc ^= acc >> 17;
        } else {
            acc = acc.wrapping_mul(3);
        }
    }
    acc
"""


def calls_src(dispatch):
    lines = ["fn main() {", "    let mut total = 0u64;"]
    for i in range(N_TYPES):
        if dispatch == "mono":
            lines.append(f"    total = total.wrapping_add(drive(&T{i}, 200));")
        else:
            lines.append(f"    total = total.wrapping_add(drive(&T{i}, 200));")
    lines.append('    println!("{total}");')
    lines.append("}")
    return "\n".join(lines)


def gen(dispatch):
    is_mono = dispatch == "mono"
    sig = ("pub fn drive<T: Work>(t: &T, n: u64) -> u64 {"
           if is_mono else
           "pub fn drive(t: &dyn Work, n: u64) -> u64 {")
    src = f"""// 自动生成 —— 请勿手改。见 gen.py
// 本文件与另一变体逐字节相同，唯一差异是 drive 的签名（静态/动态分发）。

pub trait Work {{
    fn step(&self, x: u64) -> u64;
}}

{types_src()}
{sig}
{DRIVE_BODY}
}}

{calls_src(dispatch)}
"""
    d = os.path.join(OUT, dispatch)
    os.makedirs(os.path.join(d, "src"), exist_ok=True)
    io.open(os.path.join(d, "Cargo.toml"), "w", encoding="utf-8", newline="\n").write(
        f'[package]\nname = "dynproof-{dispatch}"\nversion = "0.1.0"\nedition = "2021"\n'
        f'publish = false\n\n[dependencies]\n\n{PROFILE}')
    io.open(os.path.join(d, "src", "main.rs"), "w", encoding="utf-8", newline="\n").write(src)
    return len(src.splitlines())


if __name__ == "__main__":
    n = gen("mono")
    gen("dyn")
    print(f"已生成 mono/ 与 dyn/，各 {n} 行，类型数 {N_TYPES}")
