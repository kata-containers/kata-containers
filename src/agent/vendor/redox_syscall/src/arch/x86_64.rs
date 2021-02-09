use super::error::{Error, Result};

pub unsafe fn syscall0(mut a: usize) -> Result<usize> {
    llvm_asm!("syscall"
        : "={rax}"(a)
        : "{rax}"(a)
        : "rcx", "r11", "memory"
        : "intel", "volatile");

    Error::demux(a)
}

pub unsafe fn syscall1(mut a: usize, b: usize) -> Result<usize> {
    llvm_asm!("syscall"
        : "={rax}"(a)
        : "{rax}"(a), "{rdi}"(b)
        : "rcx", "r11", "memory"
        : "intel", "volatile");

    Error::demux(a)
}

// Clobbers all registers - special for clone
pub unsafe fn syscall1_clobber(mut a: usize, b: usize) -> Result<usize> {
    llvm_asm!("syscall"
        : "={rax}"(a)
        : "{rax}"(a), "{rdi}"(b)
        : "memory", "rbx", "rcx", "rdx", "rsi", "rdi", "r8",
          "r9", "r10", "r11", "r12", "r13", "r14", "r15"
        : "intel", "volatile");

    Error::demux(a)
}

pub unsafe fn syscall2(mut a: usize, b: usize, c: usize) -> Result<usize> {
    llvm_asm!("syscall"
        : "={rax}"(a)
        : "{rax}"(a), "{rdi}"(b), "{rsi}"(c)
        : "rcx", "r11", "memory"
        : "intel", "volatile");

    Error::demux(a)
}

pub unsafe fn syscall3(mut a: usize, b: usize, c: usize, d: usize) -> Result<usize> {
    llvm_asm!("syscall"
        : "={rax}"(a)
        : "{rax}"(a), "{rdi}"(b), "{rsi}"(c), "{rdx}"(d)
        : "rcx", "r11", "memory"
        : "intel", "volatile");

    Error::demux(a)
}

pub unsafe fn syscall4(mut a: usize, b: usize, c: usize, d: usize, e: usize) -> Result<usize> {
    llvm_asm!("syscall"
        : "={rax}"(a)
        : "{rax}"(a), "{rdi}"(b), "{rsi}"(c), "{rdx}"(d), "{r10}"(e)
        : "rcx", "r11", "memory"
        : "intel", "volatile");

    Error::demux(a)
}

pub unsafe fn syscall5(mut a: usize, b: usize, c: usize, d: usize, e: usize, f: usize)
                       -> Result<usize> {
    llvm_asm!("syscall"
        : "={rax}"(a)
        : "{rax}"(a), "{rdi}"(b), "{rsi}"(c), "{rdx}"(d), "{r10}"(e), "{r8}"(f)
        : "rcx", "r11", "memory"
        : "intel", "volatile");

    Error::demux(a)
}
