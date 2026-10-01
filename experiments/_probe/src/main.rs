#![no_std]
#![no_main]
use core::{ffi::c_void, ptr};
#[link(name = "kernel32")]
extern "system" {
    fn GetStdHandle(n: u32) -> *mut c_void;
    fn WriteFile(h: *mut c_void, b: *const u8, n: u32, w: *mut u32, o: *mut c_void) -> i32;
    fn ExitProcess(c: u32) -> !;
}
#[panic_handler]
fn p(_: &core::panic::PanicInfo) -> ! { unsafe { ExitProcess(101) } }
#[no_mangle]
pub extern "C" fn mainCRTStartup() -> ! {
    let buf = [1u8, 2, 3, 4, 5];
    let i = core::hint::black_box(2usize);
    let v = buf[i];
    unsafe {
        let h = GetStdHandle(0xFFFF_FFF5u32);
        let mut w = 0u32;
        WriteFile(h, [v].as_ptr(), 1, &mut w, ptr::null_mut());
        ExitProcess(0)
    }
}
