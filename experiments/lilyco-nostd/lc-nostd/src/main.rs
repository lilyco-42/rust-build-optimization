//! 三变体共用的 driver（std bin），逐字一致，仅 crate 名不同。
//! 作用：让 lib 的代码真正被链接进可执行文件，从而可测 exe 体积。

fn main() {
    let mut reg = lc_nostd::Registry::new();
    reg.register("files.list", "列出目录内容").unwrap();
    reg.register("files.copy", "复制文件").unwrap();
    reg.register("net.probe", "探测连通性").unwrap();
    reg.register("sys.kill", "终止进程").unwrap();

    let schema = lc_nostd::CommandSchema::new("files.copy", "复制文件")
        .arg(lc_nostd::ArgSchema {
            name: String::from("src"),
            kind: lc_nostd::ArgKind::Path,
            required: true,
            help: String::from("源路径"),
            default: None,
        })
        .arg(lc_nostd::ArgSchema {
            name: String::from("dst"),
            kind: lc_nostd::ArgKind::Path,
            required: true,
            help: String::from("目标路径"),
            default: Some(String::from("/tmp/out")),
        })
        .danger();

    println!("{}", lc_nostd::schema_to_json(&schema));
    println!(
        "{}",
        lc_nostd::progress_to_json(&lc_nostd::Progress::Started {
            total: Some(100),
            message: Some(String::from("开始"))
        })
    );
    println!(
        "{}",
        lc_nostd::progress_to_json(&lc_nostd::Progress::Tick {
            current: 42,
            total: Some(100),
            message: Some(String::from("第 42 帧")),
            percent: Some(0.42)
        })
    );
    println!(
        "{}",
        lc_nostd::progress_to_json(&lc_nostd::Progress::Log {
            level: lc_nostd::LogLevel::Info,
            message: String::from("hello")
        })
    );
    println!(
        "{}",
        lc_nostd::progress_to_json(&lc_nostd::Progress::Telemetry {
            key: String::from("altitude"),
            value: 12.5
        })
    );
    println!(
        "{}",
        lc_nostd::progress_to_json(&lc_nostd::Progress::Done {
            result: String::from("ok"),
            duration_ms: 1234
        })
    );
    println!(
        "{}",
        lc_nostd::progress_to_json(&lc_nostd::Progress::Error {
            code: 7,
            message: String::from("boom"),
            kind: Some(String::from("runtime"))
        })
    );
    println!(
        "{}",
        lc_nostd::error_to_json(&lc_nostd::AppError::Runtime(String::from("boom")))
    );
    println!("{}", lc_nostd::registry_to_json(&reg));
    println!("n={} names={:?}", reg.len(), reg.names());
    match lc_nostd::parse_command(&lc_nostd::schema_to_json(&schema)) {
        Ok(c) => println!("roundtrip ok: {} args={}", c.name, c.args.len()),
        Err(e) => println!("roundtrip err: {e}"),
    }
}
