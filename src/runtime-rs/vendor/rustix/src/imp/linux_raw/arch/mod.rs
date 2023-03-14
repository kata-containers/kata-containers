//! Architecture-specific syscall code.
//!
//! `rustix` has inline assembly sequences using `asm!`, but that requires
//! nightly Rust, so it also has out-of-line ("outline") assembly sequences
//! in .s files. And 32-bit x86 is special (see comments below).
//!
//! This module also has a `choose` submodule which chooses a scheme and is
//! what most of the `rustix` syscalls use.
//!
//! # Safety
//!
//! This contains the inline `asm` statements performing the syscall
//! instructions and FFI declarations declaring the out-of-line ("outline")
//! syscall instructions.

#![allow(unsafe_code)]

// When inline asm is available, use it.
#[cfg(asm)]
pub(in crate::imp) mod inline;
#[cfg(asm)]
pub(in crate::imp) use self::inline as asm;

// When inline asm isn't available, use out-of-line asm.
#[cfg(not(asm))]
pub(in crate::imp) mod outline;
#[cfg(not(asm))]
pub(in crate::imp) use self::outline as asm;

// On most architectures, the architecture syscall instruction is fast, so use
// it directly.
#[cfg(any(
    target_arch = "arm",
    target_arch = "aarch64",
    target_arch = "mips",
    target_arch = "mips64",
    target_arch = "powerpc64",
    target_arch = "riscv64",
    target_arch = "x86_64",
))]
pub(in crate::imp) use self::asm as choose;

// On 32-bit x86, use vDSO wrappers for all syscalls. We could use the
// architecture syscall instruction (`int 0x80`), but the vDSO kernel_vsyscall
// mechanism is much faster.
#[cfg(target_arch = "x86")]
pub(in crate::imp) use super::vdso_wrappers::x86_via_vdso as choose;

// This would be the code for always using `int 0x80` on 32-bit x86.
//#[cfg(target_arch = "x86")]
//pub(in crate::imp) use self::asm as choose;
