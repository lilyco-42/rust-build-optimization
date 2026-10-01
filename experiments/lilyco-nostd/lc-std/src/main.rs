//! 三变体共用的 driver（std bin），逐字一致，仅 crate 名不同。
//! 作用：让 lib 的代码真正被链接进可执行文件，从而可测 exe 体积。

fn main() {
    let mut reg = lc_std::Registry::new();
    reg.register("files.list", "列出目录内容").unwrap();
    reg.register("files.copy", "复制文件").unwrap();
    reg.register("net.probe", "探测连通性").unwrap();
    reg.register("sys.kill", "终止进程").unwrap();

    let schema = lc_std::CommandSchema::new("files.copy", "复制文件")
        .arg(lc_std::ArgSchema {
            name: String::from("src"),
            kind: lc_std::ArgKind::Path,
            required: true,
            help: String::from("源路径"),
            default: None,
        })
        .arg(lc_std::ArgSchema {
            name: String::from("dst"),
            kind: lc_std::ArgKind::Path,
            required: true,
            help: String::from("目标路径"),
            default: Some(String::from("/tmp/out")),
        })
        .danger();

    println!("{}", lc_std::schema_to_json(&schema));
    println!(
        "{}",
        lc_std::progress_to_json(&lc_std::Progress::Started {
            total: Some(100),
            message: Some(String::from("开始"))
        })
    );
    println!(
        "{}",
        lc_std::progress_to_json(&lc_std::Progress::Tick {
            current: 42,
            total: Some(100),
            message: Some(String::from("第 42 帧")),
            percent: Some(0.42)
        })
    );
    println!(
        "{}",
        lc_std::progress_to_json(&lc_std::Progress::Log {
            level: lc_std::LogLevel::Info,
            message: String::from("hello")
        })
    );
    println!(
        "{}",
        lc_std::progress_to_json(&lc_std::Progress::Telemetry {
            key: String::from("altitude"),
            value: 12.5
        })
    );
    println!(
        "{}",
        lc_std::progress_to_json(&lc_std::Progress::Done {
            result: String::from("ok"),
            duration_ms: 1234
        })
    );
    println!(
        "{}",
        lc_std::progress_to_json(&lc_std::Progress::Error {
            code: 7,
            message: String::from("boom"),
            kind: Some(String::from("runtime"))
        })
    );
    println!(
        "{}",
        lc_std::error_to_json(&lc_std::AppError::Runtime(String::from("boom")))
    );
    println!("{}", lc_std::registry_to_json(&reg));
    println!("n={} names={:?}", reg.len(), reg.names());
    match lc_std::parse_command(&lc_std::schema_to_json(&schema)) {
        Ok(c) => println!("roundtrip ok: {} args={}", c.name, c.args.len()),
        Err(e) => println!("roundtrip err: {e}"),
    }
}
