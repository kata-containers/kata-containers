// Copyright (c) 2022 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

#[cfg(target_arch = "aarch64")]
pub use arch_specific::*;

mod arch_specific {
    use crate::check;
    use crate::types::*;
    use crate::utils;
    use anyhow::Result;
    use slog::{info, o, warn};
    use std::path::Path;

    const KVM_DEV: &str = "/dev/kvm";
    #[allow(dead_code)]
    pub const ARCH_CPU_VENDOR_FIELD: &str = "CPU implementer";
    #[allow(dead_code)]
    pub const ARCH_CPU_MODEL_FIELD: &str = "CPU architecture";

    macro_rules! sl {
        () => {
            slog_scope::logger().new(o!("subsystem" => "aarch64"))
        };
    }

    // List of check functions
    static CHECK_LIST: &[CheckItem] = &[CheckItem {
        name: CheckType::Cpu,
        descr: "This parameter performs the host check",
        fp: check,
        perm: PermissionType::NonPrivileged,
    }];

    pub fn check(_args: &str) -> Result<()> {
        info!(sl!(), "check: aarch64");
        if Path::new(KVM_DEV).exists() {
            info!(sl!(), "Kata Containers can run on this host\n");
        } else {
            warn!(
                sl!(),
                "Kata Containers can't run on this host as lack of virtulization support\n"
            );
        }

        Ok(())
    }

    fn normalize_vendor(vendor: &str) -> String {
        match vendor {
            "0x41" => String::from("ARM Limited"),
            _ => String::from("3rd Party Limited"),
        }
    }

    fn normalize_model(model: &str) -> String {
        match model {
            "8" => String::from("v8"),
            "7" | "7M" | "?(12)" | "?(13)" | "?(14)" | "?(15)" | "?(16)" | "?(17)" => {
                String::from("v7")
            }
            "6" | "6TEJ" => String::from("v6"),
            "5" | "5T" | "5TE" | "5TEJ" => String::from("v5"),
            "4" | "4T" => String::from("v4"),
            "3" => String::from("v3"),
            _ => String::from("unknown"),
        }
    }

    pub fn get_cpu_details() -> Result<(String, String)> {
        let (vendor, model) = utils::get_generic_cpu_details(check::PROC_CPUINFO)?;
        let norm_vendor = normalize_vendor(&vendor);
        let norm_model = normalize_model(&model);
        Ok((norm_vendor, norm_model))
    }

    pub fn get_checks() -> Option<&'static [CheckItem<'static>]> {
        Some(CHECK_LIST)
    }

    pub fn host_is_vmcontainer_capable() -> Result<bool> {
        // TODO: Not implemented
        Ok(true)
    }
}
