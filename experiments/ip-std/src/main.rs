//! ip-std — 与 ip-bare / ip-c 同功能的 **常规 Rust 写法**（用 std，零第三方 crate）。
//!
//! 对照组存在的唯一目的：把「std 的成本」和「第三方依赖的成本」分开。
//! 注：std 没有跨平台枚举网卡的 API，`--detail` 在此版本不实现（其余参数完全对齐）。

use std::io::Write;
use std::net::UdpSocket;

fn main() {
    let mut target = String::from("8.8.8.8:80");
    let mut detail = false;
    let mut json = false;

    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--detail" => detail = true,
            "--json" => json = true,
            "--target" => {
                i += 1;
                if i < args.len() {
                    target = args[i].clone();
                }
            }
            "-h" | "--help" => {
                print!(
                    "跨平台 IP 查看\n\nUsage: {} [OPTIONS]\n\nOptions:\n\
                     \x20     --target <ip:port>  探测出口 IP 的目标（只 connect 不发包） [default: 8.8.8.8:80]\n\
                     \x20     --detail            列出全部网卡地址\n\
                     \x20     --json              单行 JSON 输出\n\
                     \x20 -h, --help              打印帮助\n",
                    std::env::args().next().unwrap_or_default()
                );
                return;
            }
            other => {
                println!("未知参数: {other}");
                return;
            }
        }
        i += 1;
    }

    let host = std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .unwrap_or_else(|_| "unknown".to_string());

    let ip = match UdpSocket::bind("0.0.0.0:0") {
        Ok(s) => match s.connect(&target).and_then(|_| s.local_addr()) {
            Ok(a) => a.ip().to_string(),
            Err(_) => "unreachable".to_string(),
        },
        Err(_) => "unreachable".to_string(),
    };

    let out = if json {
        format!(
            "{{\"hostname\": \"{}\", \"os\": \"{}\", \"arch\": \"{}\", \"local_ip\": \"{}\", \
             \"is_loopback\": {}, \"target\": \"{}\"{}\n",
            host,
            std::env::consts::OS,
            std::env::consts::ARCH,
            ip,
            ip == "::1" || ip.starts_with("127."),
            target,
            if detail { ",\n" } else { "}\n" }
        )
    } else {
        format!(
            "{}/{} {} -> {}\n",
            std::env::consts::OS,
            std::env::consts::ARCH,
            host,
            ip
        )
    };
    let o = std::io::stdout();
    let mut o = o.lock();
    let _ = o.write_all(out.as_bytes());
}





