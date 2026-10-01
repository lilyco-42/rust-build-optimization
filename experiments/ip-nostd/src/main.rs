//! ip-nostd — no_std 最小验证版，对标 ip-c 默认行为（本机出口 IP + 主机名）。
//! 只要默认路径：UDP connect 8.8.8.8:80（只 connect 不发包），反查本机出口 IP。
//! 不支持 --detail/--json/--target（要上 getaddrinfo + 枚举网卡，FFI 量翻几倍）。
//! 纯 core，无 alloc，无 std，无第三方依赖。

#![no_std]
#![no_main]

use core::{ffi::c_void, fmt, ptr, slice, str};

const AF_INET: i32 = 2;
const SOCK_DGRAM: i32 = 2;
const IPPROTO_UDP: i32 = 17;
const INVALID_SOCKET: usize = usize::MAX;
const STD_OUTPUT_HANDLE: u32 = 0xFFFF_FFF5; // -11

#[repr(C)]
struct SockAddrIn {
    sin_family: u16,
    sin_port: u16,
    sin_addr: u32,
    sin_zero: [u8; 8],
}

#[link(name = "kernel32")]
#[link(name = "ws2_32")]
extern "system" {
    fn GetStdHandle(nStdHandle: u32) -> *mut c_void;
    fn WriteFile(
        hFile: *mut c_void,
        lpBuffer: *const u8,
        nNumberOfBytesToWrite: u32,
        lpNumberOfBytesWritten: *mut u32,
        lpOverlapped: *mut c_void,
    ) -> i32;
    fn GetComputerNameA(lpBuffer: *mut u8, lpnSize: *mut u32) -> i32;
    fn ExitProcess(uExitCode: u32) -> !;
    fn WSAStartup(wVersionRequested: u16, lpWSAData: *mut u8) -> i32;
    fn WSACleanup() -> i32;
    fn socket(af: i32, typ: i32, protocol: i32) -> usize;
    fn connect(s: usize, name: *const SockAddrIn, namelen: i32) -> i32;
    fn getsockname(s: usize, name: *mut SockAddrIn, namelen: *mut i32) -> i32;
    fn closesocket(s: usize) -> i32;
    fn inet_addr(cp: *const u8) -> u32;
    fn inet_ntoa(addr: u32) -> *const u8;
    fn htons(hostshort: u16) -> u16;
}

#[panic_handler]
fn on_panic(_: &core::panic::PanicInfo) -> ! {
    unsafe { ExitProcess(1) }
}

// no_std + msvc：[0; N] 初始化要调 memset/memcpy，msvcrt 没链进来就手写
#[no_mangle]
pub unsafe extern "C" fn memset(dest: *mut c_void, c: i32, n: usize) -> *mut c_void {
    let d = dest as *mut u8;
    let mut i = 0;
    while i < n {
        *d.add(i) = c as u8;
        i += 1;
    }
    dest
}

#[no_mangle]
pub unsafe extern "C" fn memcpy(
    dest: *mut c_void,
    src: *const c_void,
    n: usize,
) -> *mut c_void {
    let d = dest as *mut u8;
    let s = src as *const u8;
    let mut i = 0;
    while i < n {
        *d.add(i) = *s.add(i);
        i += 1;
    }
    dest
}

struct Stdout {
    handle: *mut c_void,
}

impl fmt::Write for Stdout {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let bytes = s.as_bytes();
        let mut done: u32 = 0;
        unsafe {
            WriteFile(
                self.handle,
                bytes.as_ptr(),
                bytes.len() as u32,
                &mut done,
                ptr::null_mut(),
            );
        }
        Ok(())
    }
}

fn cstr_len(p: *const u8, max: usize) -> usize {
    let mut n = 0;
    unsafe {
        while n < max && *p.add(n) != 0 {
            n += 1;
        }
    }
    n
}

fn run() {
    let handle = unsafe { GetStdHandle(STD_OUTPUT_HANDLE) };
    let mut out = Stdout { handle };

    // 主机名
    let mut host: [u8; 256] = [0; 256];
    let mut host_len: u32 = 256;
    let host_ok = unsafe { GetComputerNameA(host.as_mut_ptr(), &mut host_len) != 0 };
    let host_bytes: &[u8] = if host_ok {
        unsafe { slice::from_raw_parts(host.as_ptr(), (host_len as usize).min(255)) }
    } else {
        b"unknown"
    };
    let host_str = str::from_utf8(host_bytes).unwrap_or("unknown");

    // 架构名（编译期确定，core 可用）
    let arch: &str = if cfg!(target_arch = "x86_64") {
        "x86_64"
    } else if cfg!(target_arch = "aarch64") {
        "aarch64"
    } else if cfg!(target_arch = "x86") {
        "x86"
    } else {
        "unknown"
    };

    // 出口 IP（IPv4 默认路径：8.8.8.8:80）
    let mut ip_text: &str = "unreachable";
    let mut ip_buf: [u8; 32] = [0; 32];
    let ip_len: usize;
    unsafe {
        let mut wsa: [u8; 408] = [0; 408];
        if WSAStartup(0x0202, wsa.as_mut_ptr()) == 0 {
            let s = socket(AF_INET, SOCK_DGRAM, IPPROTO_UDP);
            if s != INVALID_SOCKET {
                let server = SockAddrIn {
                    sin_family: AF_INET as u16,
                    sin_port: htons(80),
                    sin_addr: inet_addr(b"8.8.8.8\0".as_ptr()),
                    sin_zero: [0; 8],
                };
                if connect(s, &server, size_of::<SockAddrIn>() as i32) == 0 {
                    let mut local = SockAddrIn {
                        sin_family: 0,
                        sin_port: 0,
                        sin_addr: 0,
                        sin_zero: [0; 8],
                    };
                    let mut nlen = size_of::<SockAddrIn>() as i32;
                    if getsockname(s, &mut local, &mut nlen) == 0 {
                        let p = inet_ntoa(local.sin_addr);
                        if !p.is_null() {
                            let n = cstr_len(p, 31).min(31);
                            ptr::copy_nonoverlapping(p, ip_buf.as_mut_ptr(), n);
                            ip_len = n;
                            ip_text =
                                str::from_utf8(slice::from_raw_parts(ip_buf.as_ptr(), ip_len))
                                    .unwrap_or("unreachable");
                        }
                    }
                }
                closesocket(s);
            }
            WSACleanup();
        }
    }

    let _ = fmt::write(
        &mut out,
        format_args!("windows/{arch} {host_str} -> {ip_text}\n"),
    );
    unsafe { ExitProcess(0) }
}

#[no_mangle]
pub extern "system" fn mainCRTStartup() -> ! {
    run();
    unsafe { ExitProcess(0) }
}
