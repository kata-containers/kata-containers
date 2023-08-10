// Copyright (C) 2022 Alibaba Cloud. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use std::fs::File;

/// Structure to hold guest kernel configuration information.
pub struct KernelConfigInfo {
    /// The descriptor to the kernel file.
    kernel_file: File,
    /// The descriptor to the initrd file, if there is one
    initrd_file: Option<File>,
    /// The commandline for guest kernel.
    cmdline: linux_loader::cmdline::Cmdline,
}

impl KernelConfigInfo {
    /// Create a KernelConfigInfo instance.
    pub fn new(
        kernel_file: File,
        initrd_file: Option<File>,
        cmdline: linux_loader::cmdline::Cmdline,
    ) -> Self {
        KernelConfigInfo {
            kernel_file,
            initrd_file,
            cmdline,
        }
    }

    /// Get a mutable reference to the kernel file.
    pub fn kernel_file_mut(&mut self) -> &mut File {
        &mut self.kernel_file
    }

    /// Get an immutable reference to the initrd file.
    pub fn initrd_file(&self) -> Option<&File> {
        self.initrd_file.as_ref()
    }

    /// Get a mutable reference to the initrd file.
    pub fn initrd_file_mut(&mut self) -> Option<&mut File> {
        self.initrd_file.as_mut()
    }

    /// Get a shared reference to the guest kernel boot parameter object.
    pub fn kernel_cmdline(&self) -> &linux_loader::cmdline::Cmdline {
        &self.cmdline
    }

    /// Get a mutable reference to the guest kernel boot parameter object.
    pub fn kernel_cmdline_mut(&mut self) -> &mut linux_loader::cmdline::Cmdline {
        &mut self.cmdline
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vmm_sys_util::tempfile::TempFile;

    #[test]
    fn test_kernel_config_info() {
        let kernel = TempFile::new().unwrap();
        let initrd = TempFile::new().unwrap();
        let mut cmdline = linux_loader::cmdline::Cmdline::new(1024).unwrap();
        cmdline.insert_str("ro").unwrap();
        let mut info = KernelConfigInfo::new(kernel.into_file(), Some(initrd.into_file()), cmdline);

        assert_eq!(info.cmdline.as_cstring().unwrap().as_bytes(), b"ro");
        assert!(info.initrd_file_mut().is_some());
    }
}
