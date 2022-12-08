// Copyright (c) 2022 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

#[cfg(target_arch = "aarch64")]
pub use arch_specific::*;

mod arch_specific {
    use anyhow::Result;
    use std::path::Path;

    const KVM_DEV: &str = "/dev/kvm";

    pub fn check() -> Result<()> {
        println!("INFO: check: aarch64");
        if Path::new(KVM_DEV).exists() {
            println!("Kata Containers can run on this host\n");
        } else {
            eprintln!("WARNING: Kata Containers can't run on this host as lack of virtulization support\n");
        }

        Ok(())
    }
}
