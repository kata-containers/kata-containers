// Copyright (c) 2022 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

#[cfg(target_arch = "aarch64")]
pub use arch_specific::*;

mod arch_specific {
    use crate::types::*;
    use anyhow::Result;
    use std::path::Path;

    const KVM_DEV: &str = "/dev/kvm";

    // List of check functions
    static CHECK_LIST: &[CheckItem] = &[CheckItem {
        name: CheckType::CheckCpu,
        descr: "This parameter performs the host check",
        fp: check,
        perm: PermissionType::NonPrivileged,
    }];

    pub fn check(_args: &str) -> Result<()> {
        println!("INFO: check: aarch64");
        if Path::new(KVM_DEV).exists() {
            println!("Kata Containers can run on this host\n");
        } else {
            eprintln!("WARNING: Kata Containers can't run on this host as lack of virtulization support\n");
        }

        Ok(())
    }

    pub fn get_checks() -> Option<&'static [CheckItem<'static>]> {
        Some(CHECK_LIST)
    }
}
