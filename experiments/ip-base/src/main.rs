//! ipcheck — 跨平台本机 IP 查看小工具（lilyco 四端：CLI/TUI/Web/MCP）。
//!
//! ```bash
//! ip-base --target 8.8.8.8:80
//! ip-base --detail
//! ip-base --mcp     # MCP stdio，供 AI Agent 直接调用
//! ```
//!
//! 只用 std，不引第三方网络库：UDP connect 目标地址反查本机出口 IP，
//! Windows / Linux / Android 通吃，体积对比不受依赖噪音干扰。

use std::net::{ToSocketAddrs, UdpSocket};
use std::time::Instant;

use lilyco::prelude::*;

/// 跨平台 IP 查看
#[derive(App)]
#[app(run = "run_ip")]
struct IpCheck {
    /// 探测出口 IP 时连接的目标 UDP 地址（只 connect 不发包）
    #[arg(default = "8.8.8.8:80")]
    target: String,

    /// 显示更多诊断信息（主机名/平台/目标解析）
    detail: bool,
}

fn hostname() -> String {
    std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .unwrap_or_else(|_| "unknown".to_string())
}

fn run_ip(app: &IpCheck, ctx: &Context) -> Result<serde_json::Value, AppError> {
    let start = Instant::now();
    ctx.emit(Progress::Started {
        total: Some(3),
        message: None,
    });

    ctx.tick(1, Some(3), format!("connect {}", app.target));
    let sock = UdpSocket::bind("0.0.0.0:0").map_err(|e| AppError::Runtime(format!("bind: {e}")))?;
    sock.connect(app.target.as_str())
        .map_err(|e| AppError::Runtime(format!("connect {}: {e}", app.target)))?;
    let local = sock
        .local_addr()
        .map_err(|e| AppError::Runtime(format!("local_addr: {e}")))?;
    let local_ip = local.ip();

    ctx.tick(2, Some(3), format!("local {local_ip}"));
    let resolved: Vec<String> = if app.detail {
        ctx.tick(3, Some(3), "resolve target");
        app.target
            .to_socket_addrs()
            .map(|it| it.map(|a| a.to_string()).collect())
            .unwrap_or_default()
    } else {
        vec![]
    };

    let out = serde_json::json!({
        "hostname": hostname(),
        "os": std::env::consts::OS,
        "arch": std::env::consts::ARCH,
        "local_ip": local_ip.to_string(),
        "is_loopback": local_ip.is_loopback(),
        "is_ipv4": local_ip.is_ipv4(),
        "target": app.target,
        "target_resolved": resolved,
    });

    ctx.log(
        LogLevel::Info,
        format!(
            "{os}/{arch} {host} -> {ip}",
            os = std::env::consts::OS,
            arch = std::env::consts::ARCH,
            host = hostname(),
            ip = local_ip,
        ),
    );
    ctx.done(out.clone(), start.elapsed().as_millis() as u64);
    Ok(out)
}

fn main() {
    lilyco::run::<IpCheck>();
}



