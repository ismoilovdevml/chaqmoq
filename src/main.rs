// Linux a write syscall

// use std::arch::asm;

// fn main() {
//     let message = String::from("Chaqmoqdan salom, linux syscall");
//     syscall(message)
// }

// #[cfg(target_os = "linux")]
// #[inline(never)]

// fn syscall(message: String) {
//     let msg_ptr = message.as_ptr();
//     let len = message.len();
//     unsafe {
//         asm!(
//             "mov rax, 1",
//             "mov rdi, 1",
//             "syscall",
//             in("rsi") msg_ptr,
//             in("rdx") len,
//             out("rax") _, out("rdi") _, lateout("rsi") _, lateout("rdx") _
//         );
//     }
// }

// MacOS a write syscall

// use std::arch::asm;

// fn main() {
//     let message = String::from("Chaqmoqdan salom, macos syscall");
//     syscall(message)
// }

// #[cfg(target_os = "linux")]
// #[inline(never)]

// fn syscall(message: String){
//     let msg_ptr = message.as_ptr();
//     let len = message.len();
//     unsafe{
//         asm!(
//             "mov rax, 0x2000004",
//             "mov rdi, 1",
//             "syscall",
//             in("rsi") msg_ptr,
//             in("rdx") len,
//             out("rax") _, out("rdi") _, lateout("rsi") _, lateout("rdx") _
//         );
//     }
// }
// Linux and MacOS syscall

// use std::io;

// fn main() {
//     let sys_message = String::from("Chaqmoqdan salom, Linux and MacOS syscall");
//     syscall(sys_message).unwrap();
// }

// #[cfg(not(target_os = "windows"))]
// #[link(name = "c")]

// extern "C" {
//     fn write(fd: u32, buf: *const u8, count: usize) -> i32;
// }

// #[cfg(not(target_os = "windows"))]

// fn syscall(message: String) -> io::Result<()> {
//     let msg_ptr = message.as_ptr();
//     let len= message.len();
//     let res = unsafe { write(1, msg_ptr, len)};

//     if res == -1 {
//         return Err(io::Error::last_os_error());
//     }
//     Ok(())
// }

// Windows write syscall

// use std::io;

// fn main() {
//     let sys_message = String::from("Chaqmoqdan salom, windows syscall");
//     syscall(sys_message).unwrap();
// }

// #[cfg(target_os = "windows")]
// #[link(name = "kernel32")]

// extern "stdcall" {
//     fn GetStdHandle(nStdHandle: i32) -> i32;

//     fn WriteConsoleW(
//         hConsoleOutput: i32,
//         lpBuffer: *const u16,
//         numberOfCharsToWrite: u32,
//         lpReserved: *const std::ffi::c_void,
//     ) -> i32;
// }

// #[cfg(target_os = "windows")]

// fn syscall(message: String) -> io::Result<()> {
//     let msg: Vec<u16> = message.encode_utf16().collect();
//     let msg_ptr = msg.as_ptr();
//     let len = msg.len()as u32;
    
//     let mut output: u32 = 0;
//         let handle = unsafe { GetStdHandle(-11) };
//         if handle == -1 {
//             return Err(io::Error::last_os_error())
//         }

//         let res = unsafe {
//             WriteConsoleW(handle, msg_ptr, len, &mut output, std::ptr::null())
//         };

//         if res == 0 {
//             return Err(io::Error::last_os_error());
//         }
//     assert_eq!(output as usize, len);
//     Ok(())
// }

// cross-platform syscall (linux, macos, windows)

use std::io;

fn main() {
    let sys_message = String::from("Chaqmoqdan salom, cross-platform syscall ");
    syscall(sys_message).unwrap();
}

#[cfg(not(target_os = "windows"))]
#[link(name = "c")]
extern "C" {
    fn write(fd: u32, buf: *const u8, count: usize) -> i32;
}

#[cfg(not(target_os = "windows"))]
fn syscall(message: String) -> io::Result<()> {
    let msg_ptr = message.as_ptr();
    let len = message.len();
    let res = unsafe { write(1, msg_ptr, len)};

    if res == -1 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

#[cfg(target_os = "windows")]
#[link(name = "kernel32")]
extern "stdcall" {
    fn GetStdHandle(nStdHandle: i32) -> i32;
    fn WriteConsoleW(
        hConsoleOutput: i32,
        lpBuffer: *const u16,
        numberOfCharsWritten: *mut u32,
        lpReserved: *const std::ffi::c_void,
    ) -> i32;
}

#[cfg(target_os = "windows")]
fn syscall(message: String) -> io::Result<()> {
    let msg: Vec<u16> = message.encode_utf16().collect();
    let msg_ptr = msg.as_ptr();
    let len = msg.len() as u32;

    let mut output: u32 = 0;
    let handle = unsafe { GetStdHandle(-11) };
    if handle == -1 {
        return Err(io::Error::last_os_error());
    }

    let res = unsafe { WriteConsoleW(handle, msg_ptr, len, &mut output, std::ptr::null())};

    if res == 0 {
        return Err(io::Error::last_os_error());
    }

    assert_eq!(output, len);
    Ok(())
}