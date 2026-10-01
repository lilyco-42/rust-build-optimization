//! 三变体共用的 driver（std bin），逐字一致，仅 crate 名不同。
//! 作用：让 lib 的代码真正被链接进可执行文件，从而可测 exe 体积。

fn main() {
    let mut reg = lc_noderive::Registry::new();
    reg.register("files.list", "列出目录内容").unwrap();
    reg.register("files.copy", "复制文件").unwrap();
    reg.register("net.probe", "探测连通性").unwrap();
    reg.register("sys.kill", "终止进程").unwrap();

    let schema = lc_noderive::CommandSchema::new("files.copy", "复制文件")
        .arg(lc_noderive::ArgSchema {
            name: String::from("src"),
            kind: lc_noderive::ArgKind::Path,
            required: true,
            help: String::from("源路径"),
            default: None,
        })
        .arg(lc_noderive::ArgSchema {
            name: String::from("dst"),
            kind: lc_noderive::ArgKind::Path,
            required: true,
            help: String::from("目标路径"),
            default: Some(String::from("/tmp/out")),
        })
        .danger();

    println!("{}", lc_noderive::schema_to_json(&schema));
    println!(
        "{}",
        lc_noderive::progress_to_json(&lc_noderive::Progress::Started {
            total: Some(100),
            message: Some(String::from("开始"))
        })
    );
    println!(
        "{}",
        lc_noderive::progress_to_json(&lc_noderive::Progress::Tick {
            current: 42,
            total: Some(100),
            message: Some(String::from("第 42 帧")),
            percent: Some(0.42)
        })
    );
    println!(
        "{}",
        lc_noderive::progress_to_json(&lc_noderive::Progress::Log {
            level: lc_noderive::LogLevel::Info,
            message: String::from("hello")
        })
    );
    println!(
        "{}",
        lc_noderive::progress_to_json(&lc_noderive::Progress::Telemetry {
            key: String::from("altitude"),
            value: 12.5
        })
    );
    println!(
        "{}",
        lc_noderive::progress_to_json(&lc_noderive::Progress::Done {
            result: String::from("ok"),
            duration_ms: 1234
        })
    );
    println!(
        "{}",
        lc_noderive::progress_to_json(&lc_noderive::Progress::Error {
            code: 7,
            message: String::from("boom"),
            kind: Some(String::from("runtime"))
        })
    );
    println!(
        "{}",
        lc_noderive::error_to_json(&lc_noderive::AppError::Runtime(String::from("boom")))
    );
    println!("{}", lc_noderive::registry_to_json(&reg));
    println!("n={} names={:?}", reg.len(), reg.names());
}
