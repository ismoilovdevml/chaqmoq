// Linux a write syscall

// use std::arch::asm;

// fn main() {
//     let message = String::from("Chaqmoqdan salom");
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
//     let message = String::from("Chaqmoqdan salom");
//     syscall(message)
// }

// #[cfg(target_os ="linux")]
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

// Linux and MacOS write syscall

use std::io;

fn main() {
    let sys_message = String::from("Chaqmoqdan salom syscall");
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
    let len= message.len();
    let res = unsafe { write(1, msg_ptr, len)};

    if res == -1 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}