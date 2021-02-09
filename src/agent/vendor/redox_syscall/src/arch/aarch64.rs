use super::error::{Error, Result};

pub unsafe fn syscall0(mut a: usize) -> Result<usize> {
    llvm_asm!("svc   0"
          : "={x0}"(a)
          : "{x8}"(a)
          : "x0", "x8"
          : "volatile");

    Error::demux(a)
}

pub unsafe fn syscall1(mut a: usize, b: usize) -> Result<usize> {
    llvm_asm!("svc   0"
          : "={x0}"(a)
          : "{x8}"(a), "{x0}"(b)
          : "x0", "x8"
          : "volatile");

    Error::demux(a)
}

// Clobbers all registers - special for clone
pub unsafe fn syscall1_clobber(mut a: usize, b: usize) -> Result<usize> {
    llvm_asm!("svc   0"
          : "={x0}"(a)
          : "{x8}"(a), "{x0}"(b)
          : "memory",
            "x0", "x1",  "x2",  "x3",  "x4",  "x5", "x6", "x7",
            "x8",  "x9", "x10", "x11", "x12", "x13", "x14", "x15",
            "x16", "x17","x18", "x19", "x20", "x21", "x22", "x23",
            "x24", "x25", "x26", "x27", "x28", "x29", "x30"
          : "volatile");

    Error::demux(a)
}

pub unsafe fn syscall2(mut a: usize, b: usize, c: usize) -> Result<usize> {
    llvm_asm!("svc   0"
          : "={x0}"(a)
          : "{x8}"(a), "{x0}"(b), "{x1}"(c)
          : "x0", "x1", "x8"
          : "volatile");

    Error::demux(a)
}

pub unsafe fn syscall3(mut a: usize, b: usize, c: usize, d: usize) -> Result<usize> {
    llvm_asm!("svc   0"
          : "={x0}"(a)
          : "{x8}"(a), "{x0}"(b), "{x1}"(c), "{x2}"(d)
          : "x0", "x1", "x2", "x8"
          : "volatile");

    Error::demux(a)
}

pub unsafe fn syscall4(mut a: usize, b: usize, c: usize, d: usize, e: usize) -> Result<usize> {
    llvm_asm!("svc   0"
          : "={x0}"(a)
          : "{x8}"(a), "{x0}"(b), "{x1}"(c), "{x2}"(d), "{x3}"(e)
          : "x0", "x1", "x2", "x3", "x8"
          : "volatile");

    Error::demux(a)
}

pub unsafe fn syscall5(mut a: usize, b: usize, c: usize, d: usize, e: usize, f: usize)
    -> Result<usize> {
    llvm_asm!("svc   0"
          : "={x0}"(a)
          : "{x8}"(a), "{x0}"(b), "{x1}"(c), "{x2}"(d), "{x3}"(e), "{x4}"(f)
          : "x0", "x1", "x2", "x3", "x4", "x8"
          : "volatile");

    Error::demux(a)
}
