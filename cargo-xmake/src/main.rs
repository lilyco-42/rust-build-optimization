use std::process::ExitCode;

fn main() -> ExitCode {
    let argv: Vec<String> = std::env::args().collect();
    let code = cargo_xmake::main_with(argv);
    // Windows 上退出码只取低 8 位是历史行为，但 std 会处理好；这里保持原样
    ExitCode::from((code.clamp(0, 255)) as u8)
}
