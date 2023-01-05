// Linux a write syscall

// use::std::arch::asm;

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

use::std::arch::asm;

fn main() {
    let message = String::from("Chaqmoqdan salom");
    syscall(message)
}

#[cfg(target_os ="linux")]
#[inline(never)]

fn syscall(message: String){
    let msg_ptr = message.as_ptr();
    let len = message.len();
    unsafe{
        asm!(
            "mov rax, 0x2000004",
            "mov rdi, 1",
            "syscall",
            in("rsi") msg_ptr,
            in("rdx") len,
            out("rax") _, out("rdi") _, lateout("rsi") _, lateout("rdx") _
        );
    }
}