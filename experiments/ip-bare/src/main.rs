//! ip-bare — 跨平台本机 IP 查看工具（**零依赖 / no_std / no_main**，直通系统调用）。
//!
//! 功能与 `ip-c`（xmake C 版）逐条对齐：
//!
//! ```text
//! ip-bare                      本机出口 IP + 主机名/平台
//! ip-bare --detail             列出全部网卡单播地址
//! ip-bare --json               单行 JSON 输出（手写，无第三方库）
//! ip-bare --target 1.1.1.1:80  换探测目标（只 connect 不发包）
//! ip-bare -h | --help
//! ```
//!
//! 只用 `core`，无 `alloc`、无 `std`、无任何第三方 crate：
//! Windows 直接链接 kernel32 / ws2_32 / iphlpapi，POSIX 直接链接 libc。
//! 这是「把 Rust 编译成本压到 C 量级」的第一性原理做法——**不付依赖税**。

#![no_std]
#![no_main]
#![allow(
    non_snake_case,
    non_camel_case_types,
    non_upper_case_globals,
    dead_code
)]

use core::{ffi::c_void, ptr, str};

const OS_NAME: &str = if cfg!(windows) {
    "windows"
} else if cfg!(target_os = "android") {
    "android"
} else if cfg!(target_os = "linux") {
    "linux"
} else if cfg!(target_os = "macos") {
    "macos"
} else {
    "unknown"
};

const ARCH_NAME: &str = if cfg!(target_arch = "x86_64") {
    "x86_64"
} else if cfg!(target_arch = "aarch64") {
    "aarch64"
} else if cfg!(target_arch = "x86") {
    "x86"
} else {
    "unknown"
};

// ---------------------------------------------------------------------------
// 平台无关的小工具
// ---------------------------------------------------------------------------

unsafe fn cstr_len(p: *const u8) -> usize {
    let mut n = 0;
    while *p.add(n) != 0 {
        n += 1;
    }
    n
}

unsafe fn cstr<'a>(p: *const u8) -> &'a [u8] {
    core::slice::from_raw_parts(p, cstr_len(p))
}

// --- 无边界检查的原语 -------------------------------------------------------
// 为什么全篇手写 unchecked：**每保留一个下标/切片，就保留一个 panic 调用点**，
// 而 `panic_bounds_check → panic_fmt → core::fmt` 这套格式化机器实测吃掉了
// 约 10KB 的 .text（MAP 里最后 10,253 字节全是它）。去掉全部 panic 调用点后，
// 产物从 23.5KB 掉到 13KB 量级，正好压到 C 的水平。
// 代价：下面每个 unchecked 的合法性都由调用处保证（见各处注释）。

/// 读取第 i 个字节，无边界检查。调用处保证 i < s.len()。
unsafe fn b_at(s: &[u8], i: usize) -> u8 {
    *s.as_ptr().add(i)
}

/// 切 [start, end)，无边界检查。调用处保证 start <= end <= s.len()。
unsafe fn slice_u(s: &[u8], start: usize, end: usize) -> &[u8] {
    core::slice::from_raw_parts(s.as_ptr().add(start), end - start)
}

/// 把 "host:port" 按**最后一个**冒号切开（兼容 IPv6 字面量写法）。
///
/// 手撕而不是用 `str::rfind(':')`：rfind 会拖进 `core::str::pattern::StrSearcher`
/// （它要 `memcmp`，且带 landing pad，dev 档下会引出 `__CxxFrameHandler3`）。
fn split_host_port(s: &[u8]) -> (&[u8], &[u8]) {
    let mut i = s.len();
    while i > 0 {
        i -= 1;
        if unsafe { b_at(s, i) } == b':' {
            // i < len 且 i+1 <= len，切片合法
            return unsafe { (slice_u(s, 0, i), slice_u(s, i + 1, s.len())) };
        }
    }
    (s, b"80")
}

/// 复制字节到定长缓冲区并补 NUL，返回写入长度。dst 至少 1 字节，n < dst.len()。
fn to_cstr(dst: &mut [u8], s: &[u8]) -> usize {
    let n = if s.len() + 1 < dst.len() {
        s.len()
    } else if dst.is_empty() {
        0
    } else {
        dst.len() - 1
    };
    unsafe {
        ptr::copy_nonoverlapping(s.as_ptr(), dst.as_mut_ptr(), n);
        *dst.as_mut_ptr().add(n) = 0;
    }
    n
}

// ---------------------------------------------------------------------------
// 输出：直接写 fd / handle，不经过任何缓冲层
// ---------------------------------------------------------------------------

struct Out;

impl Out {
    fn w(&self, b: &[u8]) {
        platform::write_all(b);
    }
    fn s(&self, t: &str) {
        self.w(t.as_bytes());
    }
}

fn usage(prog: &[u8]) {
    let o = Out;
    o.s("跨平台 IP 查看\n\nUsage: ");
    o.w(prog);
    o.s(" [OPTIONS]\n\nOptions:\n");
    o.s("      --target <ip:port>  探测出口 IP 的目标（只 connect 不发包） [default: 8.8.8.8:80]\n");
    o.s("      --detail            列出全部网卡地址\n");
    o.s("      --json              单行 JSON 输出\n");
    o.s("  -h, --help              打印帮助\n");
}

// ---------------------------------------------------------------------------
// 平台层
// ---------------------------------------------------------------------------

#[cfg(windows)]
mod platform {
    use super::*;

    pub const AF_UNSPEC: i32 = 0;
    pub const SOCK_DGRAM: i32 = 2;
    pub const NI_NUMERICHOST: i32 = 2;
    pub const INVALID_SOCKET: usize = usize::MAX;

    const STD_OUTPUT_HANDLE: u32 = 0xFFFF_FFF5; // -11
    const CP_UTF8: u32 = 65001;
    const GAA_FLAGS: u32 = 0x000C; // SKIP_MULTICAST | SKIP_DNS_SERVER
    const IF_OPER_STATUS_UP: u32 = 1;

    #[repr(C)]
    pub struct AddrInfo {
        ai_flags: i32,
        pub ai_family: i32,
        pub ai_socktype: i32,
        pub ai_protocol: i32,
        pub ai_addrlen: usize,
        ai_canonname: *mut u8,
        pub ai_addr: *const c_void,
        ai_next: *mut AddrInfo,
    }

    #[repr(C, align(8))]
    pub struct SockAddrStorage {
        _b: [u8; 128],
    }
    impl SockAddrStorage {
        pub fn zeroed() -> Self {
            SockAddrStorage { _b: [0; 128] }
        }
    }

    #[repr(C)]
    pub struct AdapterAddresses {
        _length: u32,
        _if_index: u32,
        pub next: *mut AdapterAddresses,
        _adapter_name: *const u8,
        pub first_unicast: *mut UnicastAddress,
        _a1: *const c_void,
        _a2: *const c_void,
        _a3: *const c_void,
        _dns_suffix: *const u16,
        _description: *const u16,
        pub friendly_name: *const u16,
        // PhysicalAddress 实际只占 8 字节（MAX_ADAPTER_ADDRESS_LENGTH=4），
        // 后面 PhysicalAddressLength=88 / Flags=92 / Mtu=96 / IfType=100 / OperStatus=104
        _physical: [u16; 4],
        _physical_len: u32,
        _flags: u32,
        _mtu: u32,
        _if_type: u32,
        pub oper_status: u32,
    }

    #[repr(C)]
    pub struct UnicastAddress {
        _length: u32,
        _flags: u32,
        pub next: *mut UnicastAddress,
        pub sockaddr: *const c_void,
        pub sockaddr_len: i32,
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn GetStdHandle(n: u32) -> *mut c_void;
        fn WriteFile(
            h: *mut c_void,
            buf: *const u8,
            len: u32,
            written: *mut u32,
            ov: *mut c_void,
        ) -> i32;
        fn GetComputerNameA(buf: *mut u8, size: *mut u32) -> i32;
        fn SetConsoleOutputCP(cp: u32) -> i32;
        fn GetCommandLineA() -> *const u8;
        fn WideCharToMultiByte(
            cp: u32,
            flags: u32,
            wide: *const u16,
            wide_len: i32,
            out: *mut u8,
            out_len: i32,
            def: *const u8,
            used: *mut i32,
        ) -> i32;
        fn ExitProcess(code: u32) -> !;
    }

    #[link(name = "ws2_32")]
    extern "system" {
        fn WSAStartup(ver: u16, data: *mut u8) -> i32;
        fn WSACleanup() -> i32;
        fn getaddrinfo(
            node: *const u8,
            service: *const u8,
            hints: *const AddrInfo,
            res: *mut *mut AddrInfo,
        ) -> i32;
        fn freeaddrinfo(res: *mut AddrInfo);
        fn socket(af: i32, typ: i32, proto: i32) -> usize;
        fn connect(s: usize, addr: *const c_void, len: i32) -> i32;
        fn getsockname(s: usize, addr: *mut c_void, len: *mut i32) -> i32;
        fn getnameinfo(
            sa: *const c_void,
            salen: i32,
            host: *mut u8,
            hostlen: u32,
            serv: *mut u8,
            servlen: u32,
            flags: i32,
        ) -> i32;
        fn closesocket(s: usize) -> i32;
    }

    #[link(name = "iphlpapi")]
    extern "system" {
        fn GetAdaptersAddresses(
            family: u32,
            flags: u32,
            reserved: *mut c_void,
            addrs: *mut AdapterAddresses,
            size: *mut u32,
        ) -> u32;
    }

    pub fn init() {
        unsafe {
            SetConsoleOutputCP(CP_UTF8);
            let mut wsa = [0u8; 512];
            WSAStartup(0x0202, wsa.as_mut_ptr());
        }
    }

    pub fn cleanup() {
        unsafe {
            WSACleanup();
        }
    }

    pub fn exit(code: u32) -> ! {
        unsafe { ExitProcess(code) }
    }

    pub fn write_all(b: &[u8]) {
        unsafe {
            let h = GetStdHandle(STD_OUTPUT_HANDLE);
            let mut n = 0u32;
            WriteFile(h, b.as_ptr(), b.len() as u32, &mut n, ptr::null_mut());
        }
    }

    pub fn hostname<'a>(buf: &'a mut [u8]) -> &'a [u8] {
        let mut n = buf.len() as u32;
        unsafe {
            if GetComputerNameA(buf.as_mut_ptr(), &mut n) != 0 {
                unsafe { slice_u(buf, 0, n as usize) }  // API 保证 n <= buf.len()
            } else {
                b"unknown"
            }
        }
    }

    /// UDP connect 目标地址后反查本机出口 IP（不发任何包）。成功返回 IP 字节切片。
    pub fn outbound_ip<'a>(host: &[u8], port: &[u8], out: &'a mut [u8]) -> Option<&'a [u8]> {
        let mut hb = [0u8; 256];
        let mut pb = [0u8; 16];
        to_cstr(&mut hb, host);
        to_cstr(&mut pb, port);

        unsafe {
            let hints = AddrInfo {
                ai_flags: 0,
                ai_family: AF_UNSPEC,
                ai_socktype: SOCK_DGRAM,
                ai_protocol: 0,
                ai_addrlen: 0,
                ai_canonname: ptr::null_mut(),
                ai_addr: ptr::null(),
                ai_next: ptr::null_mut(),
            };
            let mut res: *mut AddrInfo = ptr::null_mut();
            if getaddrinfo(hb.as_ptr(), pb.as_ptr(), &hints, &mut res) != 0 || res.is_null() {
                return None;
            }
            let s = socket((*res).ai_family, (*res).ai_socktype, (*res).ai_protocol);
            let mut ok = false;
            if s != INVALID_SOCKET {
                if connect(s, (*res).ai_addr, (*res).ai_addrlen as i32) == 0 {
                    let mut ss = SockAddrStorage::zeroed();
                    let mut len = 128i32;
                    if getsockname(s, &mut ss as *mut _ as *mut c_void, &mut len) == 0 {
                        ok = getnameinfo(
                            &ss as *const _ as *const c_void,
                            len,
                            out.as_mut_ptr(),
                            out.len() as u32,
                            ptr::null_mut(),
                            0,
                            NI_NUMERICHOST,
                        ) == 0;
                    }
                }
                closesocket(s);
            }
            freeaddrinfo(res);
            if ok {
                Some(cstr(out.as_ptr()))
            } else {
                None
            }
        }
    }

    /// 枚举全部处于 up 状态的网卡单播地址。
    pub fn for_each_addr(mut f: impl FnMut(&[u8], &[u8])) {
        #[repr(align(8))]
        struct Buf([u8; 65536]);
        unsafe {
            static mut BUF: Buf = Buf([0u8; 65536]);
            let base = ptr::addr_of_mut!(BUF).cast::<u8>() as *mut AdapterAddresses;
            let mut size: u32 = 65536;
            if GetAdaptersAddresses(AF_UNSPEC as u32, GAA_FLAGS, ptr::null_mut(), base, &mut size)
                != 0
            {
                return;
            }
            let mut a: *mut AdapterAddresses = base;
            while !a.is_null() {
                if (*a).oper_status == IF_OPER_STATUS_UP && !(*a).friendly_name.is_null() {
                    let mut nb = [0u8; 256];
                    WideCharToMultiByte(
                        CP_UTF8,
                        0,
                        (*a).friendly_name,
                        -1,
                        nb.as_mut_ptr(),
                        256,
                        ptr::null(),
                        ptr::null_mut(),
                    );
                    let name = cstr(nb.as_ptr());
                    let mut u = (*a).first_unicast;
                    while !u.is_null() && !(*u).sockaddr.is_null() {
                        let mut ipb = [0u8; 64];
                        if getnameinfo(
                            (*u).sockaddr,
                            (*u).sockaddr_len,
                            ipb.as_mut_ptr(),
                            64,
                            ptr::null_mut(),
                            0,
                            NI_NUMERICHOST,
                        ) == 0
                        {
                            f(name, cstr(ipb.as_ptr()));
                        }
                        u = (*u).next;
                    }
                }
                a = (*a).next;
            }
        }
    }

    /// GetCommandLineA 手撕 argv（跳过第 0 个程序名 token）。
    pub fn args() -> ArgIter<'static> {
        let cl = unsafe { cstr(GetCommandLineA()) };
        let mut it = ArgIter { s: cl, i: 0 };
        it.skip_token();
        it
    }

    /// 第 0 个 token = 程序名（帮助文本用）。
    pub fn argv0() -> &'static [u8] {
        let cl = unsafe { cstr(GetCommandLineA()) };
        let mut it = ArgIter { s: cl, i: 0 };
        it.next_token().unwrap_or(b"ip-bare")
    }
}

#[cfg(windows)]
pub struct ArgIter<'a> {
    s: &'a [u8],
    i: usize,
}

#[cfg(windows)]
impl<'a> ArgIter<'a> {
    fn skip_token(&mut self) {
        self.next_token();
    }
    fn next_token(&mut self) -> Option<&'a [u8]> {
        while self.i < self.s.len()
            && (unsafe { b_at(self.s, self.i) } == b' '
                || unsafe { b_at(self.s, self.i) } == b'\t')
        {
            self.i += 1;
        }
        if self.i >= self.s.len() {
            return None;
        }
        let start = self.i;
        if unsafe { b_at(self.s, self.i) } == b'"' {
            self.i += 1;
            let s = self.i;
            while self.i < self.s.len() && unsafe { b_at(self.s, self.i) } != b'"' {
                self.i += 1;
            }
            let end = self.i;
            self.i += 1; // 吃掉右引号
            return Some(unsafe { slice_u(self.s, s, end) });
        }
        while self.i < self.s.len()
            && unsafe { b_at(self.s, self.i) } != b' '
            && unsafe { b_at(self.s, self.i) } != b'\t'
        {
            self.i += 1;
        }
        Some(unsafe { slice_u(self.s, start, self.i) })
    }
}

#[cfg(windows)]
impl<'a> Iterator for ArgIter<'a> {
    type Item = &'a [u8];
    fn next(&mut self) -> Option<&'a [u8]> {
        self.next_token()
    }
}

// ---------------------------------------------------------------------------
// POSIX 分支（libc 直通）
// ---------------------------------------------------------------------------

#[cfg(not(windows))]
mod platform {
    use super::*;

    pub const AF_UNSPEC: i32 = 0;
    pub const SOCK_DGRAM: i32 = 2;
    pub const NI_NUMERICHOST: i32 = 2;

    #[repr(C)]
    pub struct AddrInfo {
        ai_flags: i32,
        pub ai_family: i32,
        pub ai_socktype: i32,
        pub ai_protocol: i32,
        pub ai_addrlen: u32,
        ai_canonname: *mut u8,
        pub ai_addr: *const c_void,
        ai_next: *mut AddrInfo,
    }

    #[repr(C, align(8))]
    pub struct SockAddrStorage {
        _b: [u8; 128],
    }
    impl SockAddrStorage {
        pub fn zeroed() -> Self {
            SockAddrStorage { _b: [0; 128] }
        }
    }

    #[repr(C)]
    pub struct IfAddrs {
        pub next: *mut IfAddrs,
        pub name: *const u8,
        _flags: u32,
        _pad: u32,
        pub addr: *const c_void,
        _netmask: *const c_void,
        _dstaddr: *const c_void,
        _data: *mut c_void,
    }

    #[link(name = "c")]
    extern "C" {
        fn gethostname(name: *mut u8, len: usize) -> i32;
        fn getifaddrs(ifap: *mut *mut IfAddrs) -> i32;
        fn freeifaddrs(ifa: *mut IfAddrs);
        fn getaddrinfo(
            node: *const u8,
            service: *const u8,
            hints: *const AddrInfo,
            res: *mut *mut AddrInfo,
        ) -> i32;
        fn freeaddrinfo(res: *mut AddrInfo);
        fn socket(af: i32, typ: i32, proto: i32) -> i32;
        fn connect(s: i32, addr: *const c_void, len: u32) -> i32;
        fn getsockname(s: i32, addr: *mut c_void, len: *mut u32) -> i32;
        fn getnameinfo(
            sa: *const c_void,
            salen: u32,
            host: *mut u8,
            hostlen: usize,
            serv: *mut u8,
            servlen: usize,
            flags: i32,
        ) -> i32;
        fn close(fd: i32) -> i32;
        fn write(fd: i32, buf: *const u8, n: usize) -> isize;
        fn exit(code: i32) -> !;
    }

    pub fn init() {}
    pub fn cleanup() {}

    pub fn exit(code: u32) -> ! {
        unsafe { exit(code as i32) }
    }

    pub fn write_all(b: &[u8]) {
        unsafe {
            write(1, b.as_ptr(), b.len());
        }
    }

    pub fn hostname<'a>(buf: &'a mut [u8]) -> &'a [u8] {
        unsafe {
            if gethostname(buf.as_mut_ptr(), buf.len()) == 0 {
                let n = cstr_len(buf.as_ptr());
                unsafe { slice_u(buf, 0, n) }  // n = 第一个 NUL 的下标，必 < len
            } else {
                b"unknown"
            }
        }
    }

    pub fn outbound_ip<'a>(host: &str, port: &str, out: &'a mut [u8]) -> Option<&'a [u8]> {
        let mut hb = [0u8; 256];
        let mut pb = [0u8; 16];
        to_cstr(&mut hb, host);
        to_cstr(&mut pb, port);
        unsafe {
            let hints = AddrInfo {
                ai_flags: 0,
                ai_family: AF_UNSPEC,
                ai_socktype: SOCK_DGRAM,
                ai_protocol: 0,
                ai_addrlen: 0,
                ai_canonname: ptr::null_mut(),
                ai_addr: ptr::null(),
                ai_next: ptr::null_mut(),
            };
            let mut res: *mut AddrInfo = ptr::null_mut();
            if getaddrinfo(hb.as_ptr(), pb.as_ptr(), &hints, &mut res) != 0 || res.is_null() {
                return None;
            }
            let s = socket((*res).ai_family, (*res).ai_socktype, (*res).ai_protocol);
            let mut ok = false;
            if s >= 0 {
                if connect(s, (*res).ai_addr, (*res).ai_addrlen) == 0 {
                    let mut ss = SockAddrStorage::zeroed();
                    let mut len = 128u32;
                    if getsockname(s, &mut ss as *mut _ as *mut c_void, &mut len) == 0 {
                        ok = getnameinfo(
                            &ss as *const _ as *const c_void,
                            len,
                            out.as_mut_ptr(),
                            out.len(),
                            ptr::null_mut(),
                            0,
                            NI_NUMERICHOST,
                        ) == 0;
                    }
                }
                close(s);
            }
            freeaddrinfo(res);
            if ok {
                Some(cstr(out.as_ptr()))
            } else {
                None
            }
        }
    }

    pub fn for_each_addr(mut f: impl FnMut(&[u8], &[u8])) {
        unsafe {
            let mut ifap: *mut IfAddrs = ptr::null_mut();
            if getifaddrs(&mut ifap) != 0 || ifap.is_null() {
                return;
            }
            let mut p = ifap;
            while !p.is_null() {
                let fam = *( (*p).addr as *const u16 );
                if !(*p).addr.is_null() && (fam == 2 || fam == 10) {
                    let len: u32 = if fam == 2 { 16 } else { 28 };
                    let mut ipb = [0u8; 64];
                    if getnameinfo(
                        (*p).addr,
                        len,
                        ipb.as_mut_ptr(),
                        64,
                        ptr::null_mut(),
                        0,
                        NI_NUMERICHOST,
                    ) == 0
                    {
                        f(cstr((*p).name), cstr(ipb.as_ptr()));
                    }
                }
                p = (*p).next;
            }
            freeifaddrs(ifap);
        }
    }
}

// ---------------------------------------------------------------------------
// 主流程
// ---------------------------------------------------------------------------

/// 解析 argv（去掉第 0 个程序名），统一成 `&[&[u8]]` 后处理。
fn run(prog: &[u8], args: &[&[u8]]) -> u32 {
    let out = Out;
    platform::init();

    let mut target: &[u8] = b"8.8.8.8:80";
    let mut detail = false;
    let mut json = false;

    let mut i = 0;
    while i < args.len() {
        let a = unsafe { *args.as_ptr().add(i) };  // i < args.len() 由循环条件保证
        if a == b"--detail" {
            detail = true;
        } else if a == b"--json" {
            json = true;
        } else if a == b"--target" {
            i += 1;
            if i < args.len() {
                target = unsafe { *args.as_ptr().add(i) };
            }
        } else if a == b"-h" || a == b"--help" {
            usage(prog);
            platform::cleanup();
            return 0;
        } else {
            out.s("未知参数: ");
            out.w(a);
            out.s("\n");
            usage(prog);
            platform::cleanup();
            return 1;
        }
        i += 1;
    }

    let mut hbuf = [0u8; 256];
    let host = platform::hostname(&mut hbuf);

    let mut ipbuf = [0u8; 64];
    let (h, p) = split_host_port(target);
    let ip: &[u8] = match platform::outbound_ip(h, p, &mut ipbuf) {
        Some(x) => x,
        None => b"unreachable",
    };

    if json {
        out.s("{\"hostname\": \"");
        out.w(host);
        out.s("\", \"os\": \"");
        out.s(OS_NAME);
        out.s("\", \"arch\": \"");
        out.s(ARCH_NAME);
        out.s("\", \"local_ip\": \"");
        out.w(ip);
        out.s("\", \"is_loopback\": ");
        out.s(if is_loopback(ip) { "true" } else { "false" });
        out.s(", \"target\": \"");
        out.w(target);
        // 与 C 的 printf("\"target\": \"%s\"%s\n", target, detail ? "," : "}") 对齐：
        // 要先把 target 的右引号补上，再给 , 或 }
        out.s(if detail { "\",\n" } else { "\"}\n" });
        if detail {
            out.s("  \"interfaces\": [\n");
            let mut first = true;
            platform::for_each_addr(|name, aip| {
                out.s(if first { "    {" } else { "    ,{" });
                first = false;
                out.s("\"iface\": \"");
                out.w(name);
                out.s("\", \"ip\": \"");
                out.w(aip);
                out.s("\"}\n");
            });
            out.s("  ]\n}\n");
        }
    } else {
        out.s(OS_NAME);
        out.s("/");
        out.s(ARCH_NAME);
        out.s(" ");
        out.w(host);
        out.s(" -> ");
        out.w(ip);
        out.s("\n");
        if detail {
            out.s("interfaces:\n");
            platform::for_each_addr(|name, aip| {
                out.s("  ");
                out.w(name);
                // 与 C 的 printf("  %-24s %s\n") 逐字节一致：名字左对齐占 24，再补 1 个空格
                let pad = if name.len() < 24 { 25 - name.len() } else { 1 };
                for _ in 0..pad {
                    out.s(" ");
                }
                out.w(aip);
                out.s("\n");
            });
        }
    }

    platform::cleanup();
    0
}

fn is_loopback(ip: &[u8]) -> bool {
    ip == b"::1" || (ip.len() >= 4 && unsafe { slice_u(ip, 0, 4) } == &b"127."[..])
}

// ---------------------------------------------------------------------------
// 入口 / panic
// ---------------------------------------------------------------------------

// --- 自带 CRT 内存原语 -------------------------------------------------------
// LLVM 会把大块清零/拷贝/strlen 风格的循环降级成 memset/memcpy/strlen 调用，
// 而 MSVC 14.51 的 legacy msvcrt.lib 已经不再导出这三个符号，于是链接报 LNK2019。
// 与其拖进整个 CRT，不如自己实现：用 volatile 逐字节读写，避免 LLVM 再把它
// 识别回 memset/memcpy 调用（自递归）。三个函数加起来几十字节。

#[no_mangle]
pub unsafe extern "C" fn memset(dst: *mut c_void, c: i32, n: usize) -> *mut c_void {
    let d = dst as *mut u8;
    let v = c as u8;
    let mut i = 0;
    while i < n {
        ptr::write_volatile(d.add(i), v);
        i += 1;
    }
    dst
}

#[no_mangle]
pub unsafe extern "C" fn memcpy(dst: *mut c_void, src: *const c_void, n: usize) -> *mut c_void {
    let d = dst as *mut u8;
    let s = src as *const u8;
    let mut i = 0;
    while i < n {
        ptr::write_volatile(d.add(i), ptr::read_volatile(s.add(i)));
        i += 1;
    }
    dst
}

#[no_mangle]
pub unsafe extern "C" fn memcmp(a: *const c_void, b: *const c_void, n: usize) -> i32 {
    let x = a as *const u8;
    let y = b as *const u8;
    let mut i = 0;
    while i < n {
        let p = ptr::read_volatile(x.add(i));
        let q = ptr::read_volatile(y.add(i));
        if p != q {
            return p as i32 - q as i32;
        }
        i += 1;
    }
    0
}

#[no_mangle]
pub unsafe extern "C" fn strlen(s: *const i8) -> usize {
    let p = s as *const u8;
    let mut n = 0;
    while ptr::read_volatile(p.add(n)) != 0 {
        n += 1;
    }
    n
}

/// MSVC 的 SEH personality 符号。
///
/// 官方预编译的 `core` rlib 是按 **panic=unwind** 编的，它内部的 panic / 格式化代码
/// 带 landing pad，会静态引用 `__CxxFrameHandler3`。本工程 `panic = "abort"`
/// （panic_handler 直接 ExitProcess，栈**永不展开**），所以这个符号解析得到即可，
/// 运行期不会被调用。若返回，语义取 `ExceptionContinueSearch = 1`。
///
/// 没有它，dev 档（opt-level=0，core 代码没被内联掉）会报 LNK2001。
/// release 档因为 LTO 把带 landing pad 的代码都删了，反而不需要。
#[no_mangle]
pub extern "C" fn __CxxFrameHandler3(
    _exception_record: *mut c_void,
    _registration: *mut c_void,
    _context: *mut c_void,
    _dispatcher: *mut c_void,
) -> i32 {
    1
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    // 极小化 panic 处理：不格式化任何东西，直接带 101 退出。
    platform::exit(101)
}

/// 最多 8 个参数，栈上固定数组，无堆分配。
const MAX_ARGS: usize = 8;

#[cfg(windows)]
#[no_mangle]
pub extern "C" fn mainCRTStartup() -> ! {
    let mut buf: [&[u8]; MAX_ARGS] = [b""; MAX_ARGS];
    let mut n = 0;
    let prog = platform::argv0();
    for a in platform::args() {
        if n >= MAX_ARGS {
            break;
        }
        buf[n] = a;
        n += 1;
    }
    platform::exit(run(prog, &buf[..n]))
}

#[cfg(not(windows))]
#[no_mangle]
pub extern "C" fn main(argc: i32, argv: *const *const u8) -> i32 {
    let mut buf: [&[u8]; MAX_ARGS] = [b""; MAX_ARGS];
    let mut n = 0;
    let prog: &[u8] = unsafe {
        if argc > 0 && !argv.is_null() && !(*argv).is_null() {
            cstr(*argv)
        } else {
            b"ip-bare"
        }
    };
    unsafe {
        let take = if argc < 0 { 0 } else { argc as usize };
        let mut k = 1; // 跳过程序名
        while k < take && n < MAX_ARGS {
            let p = *argv.add(k);
            if p.is_null() {
                break;
            }
            buf[n] = cstr(p);
            n += 1;
            k += 1;
        }
    }
    run(prog, &buf[..n]) as i32
}
