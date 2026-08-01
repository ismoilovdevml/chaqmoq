// CPU communication with OS

// MMU communicate

// use std::arch::asm;

// fn main() {
//     let t = 100;
//     let t_ptr: *const usize = &t;
//     let x = dereference(t_ptr);

//     println!("{}", x);
// }

// fn dereference(ptr: *const usize) -> usize {
//     let mut res: usize;
//     unsafe { asm!("mov {0}, [{1}]", out(reg) res, in(reg) ptr) };
//     res
// }





use std::arch::asm;
fn main() {
    let t = 99999999999999 as *const usize;
    let x = dereference(t);

    println!("{}", x);
}
fn dereference(ptr: *const usize) -> usize {
    let mut res: usize;
    unsafe { asm!("mov {0}, [{1}]", out(reg) res, in(reg) ptr) };
    res
}
