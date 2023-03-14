// SPDX-License-Identifier: Apache-2.0 or MIT
//
// Copyright 2021 Sony Group Corporation
//

use crate::error::ErrorKind::*;
use crate::error::{Result, SeccompError};
use libseccomp_sys::*;
use std::str::FromStr;

/// Represents a CPU architecture.
/// Seccomp can restrict syscalls on a per-architecture basis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ScmpArch {
    /// The native architecture token
    Native,
    /// The x86 (32-bit) architecture token
    X86,
    /// The x86-64 (64-bit) architecture token
    X8664,
    /// The x32 (32-bit x86_64) architecture token
    X32,
    /// The ARM architecture token
    Arm,
    /// The AARCH64 architecture token
    Aarch64,
    /// The MIPS architecture token
    Mips,
    /// The MIPS (64-bit) architecture token
    Mips64,
    /// The MIPS64N32 architecture token
    Mips64N32,
    /// The MIPSEL architecture token
    Mipsel,
    /// The MIPSEL (64-bit) architecture token
    Mipsel64,
    /// The MIPSEL64N32 architecture token
    Mipsel64N32,
    /// The PowerPC architecture token
    Ppc,
    /// The PowerPC (64-bit) architecture token
    Ppc64,
    /// The PowerPC64LE architecture token
    Ppc64Le,
    /// The S390 architecture token
    S390,
    /// The S390X architecture token
    S390X,
    /// The PA-RISC hppa architecture token
    Parisc,
    /// The PA-RISC (64-bit) hppa architecture token
    Parisc64,
    /// The RISC-V architecture token
    Riscv64,
}

impl ScmpArch {
    pub(crate) fn to_sys(self) -> u32 {
        match self {
            Self::Native => SCMP_ARCH_NATIVE,
            Self::X86 => SCMP_ARCH_X86,
            Self::X8664 => SCMP_ARCH_X86_64,
            Self::X32 => SCMP_ARCH_X32,
            Self::Arm => SCMP_ARCH_ARM,
            Self::Aarch64 => SCMP_ARCH_AARCH64,
            Self::Mips => SCMP_ARCH_MIPS,
            Self::Mips64 => SCMP_ARCH_MIPS64,
            Self::Mips64N32 => SCMP_ARCH_MIPS64N32,
            Self::Mipsel => SCMP_ARCH_MIPSEL,
            Self::Mipsel64 => SCMP_ARCH_MIPSEL64,
            Self::Mipsel64N32 => SCMP_ARCH_MIPSEL64N32,
            Self::Ppc => SCMP_ARCH_PPC,
            Self::Ppc64 => SCMP_ARCH_PPC64,
            Self::Ppc64Le => SCMP_ARCH_PPC64LE,
            Self::S390 => SCMP_ARCH_S390,
            Self::S390X => SCMP_ARCH_S390X,
            Self::Parisc => SCMP_ARCH_PARISC,
            Self::Parisc64 => SCMP_ARCH_PARISC64,
            Self::Riscv64 => SCMP_ARCH_RISCV64,
        }
    }

    pub(crate) fn from_sys(arch: u32) -> Result<Self> {
        match arch {
            SCMP_ARCH_NATIVE => Ok(Self::Native),
            SCMP_ARCH_X86 => Ok(Self::X86),
            SCMP_ARCH_X86_64 => Ok(Self::X8664),
            SCMP_ARCH_X32 => Ok(Self::X32),
            SCMP_ARCH_ARM => Ok(Self::Arm),
            SCMP_ARCH_AARCH64 => Ok(Self::Aarch64),
            SCMP_ARCH_MIPS => Ok(Self::Mips),
            SCMP_ARCH_MIPS64 => Ok(Self::Mips64),
            SCMP_ARCH_MIPS64N32 => Ok(Self::Mips64N32),
            SCMP_ARCH_MIPSEL => Ok(Self::Mipsel),
            SCMP_ARCH_MIPSEL64 => Ok(Self::Mipsel64),
            SCMP_ARCH_MIPSEL64N32 => Ok(Self::Mipsel64N32),
            SCMP_ARCH_PPC => Ok(Self::Ppc),
            SCMP_ARCH_PPC64 => Ok(Self::Ppc64),
            SCMP_ARCH_PPC64LE => Ok(Self::Ppc64Le),
            SCMP_ARCH_S390 => Ok(Self::S390),
            SCMP_ARCH_S390X => Ok(Self::S390X),
            SCMP_ARCH_PARISC => Ok(Self::Parisc),
            SCMP_ARCH_PARISC64 => Ok(Self::Parisc64),
            SCMP_ARCH_RISCV64 => Ok(Self::Riscv64),
            _ => Err(SeccompError::new(ParseError)),
        }
    }

    /// Returns the system's native architecture.
    ///
    /// This function corresponds to
    /// [`seccomp_arch_native`](https://man7.org/linux/man-pages/man3/seccomp_arch_native.3.html).
    ///
    /// # Panics
    ///
    /// This function panics if it can not get the native architecture.
    pub fn native() -> Self {
        Self::from_sys(unsafe { seccomp_arch_native() }).expect("Could not get native architecture")
    }
}

impl FromStr for ScmpArch {
    type Err = SeccompError;

    /// Converts string seccomp architecture to `ScmpArch`.
    ///
    /// # Arguments
    ///
    /// * `arch` - A string architecture, e.g. `SCMP_ARCH_*`.
    ///
    /// See the [`seccomp_arch_add(3)`] man page for details on valid architecture values.
    ///
    /// [`seccomp_arch_add(3)`]: https://www.man7.org/linux/man-pages/man3/seccomp_arch_add.3.html
    ///
    /// # Errors
    ///
    /// If an invalid architecture is specified, an error will be returned.
    fn from_str(arch: &str) -> Result<Self> {
        match arch {
            "SCMP_ARCH_NATIVE" => Ok(Self::Native),
            "SCMP_ARCH_X86" => Ok(Self::X86),
            "SCMP_ARCH_X86_64" => Ok(Self::X8664),
            "SCMP_ARCH_X32" => Ok(Self::X32),
            "SCMP_ARCH_ARM" => Ok(Self::Arm),
            "SCMP_ARCH_AARCH64" => Ok(Self::Aarch64),
            "SCMP_ARCH_MIPS" => Ok(Self::Mips),
            "SCMP_ARCH_MIPS64" => Ok(Self::Mips64),
            "SCMP_ARCH_MIPS64N32" => Ok(Self::Mips64N32),
            "SCMP_ARCH_MIPSEL" => Ok(Self::Mipsel),
            "SCMP_ARCH_MIPSEL64" => Ok(Self::Mipsel64),
            "SCMP_ARCH_MIPSEL64N32" => Ok(Self::Mipsel64N32),
            "SCMP_ARCH_PPC" => Ok(Self::Ppc),
            "SCMP_ARCH_PPC64" => Ok(Self::Ppc64),
            "SCMP_ARCH_PPC64LE" => Ok(Self::Ppc64Le),
            "SCMP_ARCH_S390" => Ok(Self::S390),
            "SCMP_ARCH_S390X" => Ok(Self::S390X),
            "SCMP_ARCH_PARISC" => Ok(Self::Parisc),
            "SCMP_ARCH_PARISC64" => Ok(Self::Parisc64),
            "SCMP_ARCH_RISCV64" => Ok(Self::Riscv64),
            _ => Err(SeccompError::new(ParseError)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_arch() {
        let test_data = [
            ("SCMP_ARCH_NATIVE", ScmpArch::Native),
            ("SCMP_ARCH_X86", ScmpArch::X86),
            ("SCMP_ARCH_X86_64", ScmpArch::X8664),
            ("SCMP_ARCH_X32", ScmpArch::X32),
            ("SCMP_ARCH_ARM", ScmpArch::Arm),
            ("SCMP_ARCH_AARCH64", ScmpArch::Aarch64),
            ("SCMP_ARCH_MIPS", ScmpArch::Mips),
            ("SCMP_ARCH_MIPS64", ScmpArch::Mips64),
            ("SCMP_ARCH_MIPS64N32", ScmpArch::Mips64N32),
            ("SCMP_ARCH_MIPSEL", ScmpArch::Mipsel),
            ("SCMP_ARCH_MIPSEL64", ScmpArch::Mipsel64),
            ("SCMP_ARCH_MIPSEL64N32", ScmpArch::Mipsel64N32),
            ("SCMP_ARCH_PPC", ScmpArch::Ppc),
            ("SCMP_ARCH_PPC64", ScmpArch::Ppc64),
            ("SCMP_ARCH_PPC64LE", ScmpArch::Ppc64Le),
            ("SCMP_ARCH_S390", ScmpArch::S390),
            ("SCMP_ARCH_S390X", ScmpArch::S390X),
            ("SCMP_ARCH_PARISC", ScmpArch::Parisc),
            ("SCMP_ARCH_PARISC64", ScmpArch::Parisc64),
            ("SCMP_ARCH_RISCV64", ScmpArch::Riscv64),
        ];

        for data in test_data {
            assert_eq!(
                ScmpArch::from_sys(ScmpArch::from_str(data.0).unwrap().to_sys()).unwrap(),
                data.1
            );
        }
        assert!(ScmpArch::from_str("SCMP_INVALID_FLAG").is_err());
        assert!(ScmpArch::from_sys(1).is_err());
    }
}
