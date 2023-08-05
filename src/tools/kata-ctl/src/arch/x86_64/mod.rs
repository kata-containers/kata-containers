// Copyright (c) 2022 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

#![allow(dead_code)]

#[cfg(target_arch = "x86_64")]
pub use arch_specific::*;

mod arch_specific {
    use crate::check;
    use crate::types::*;
    use crate::utils;
    use anyhow::{anyhow, Context, Result};
    use slog::{info, o, warn};

    const CPUINFO_DELIMITER: &str = "\nprocessor";
    const CPUINFO_FLAGS_TAG: &str = "flags";
    const CPU_FLAGS_INTEL: &[&str] = &["lm", "sse4_1", "vmx"];
    const CPU_ATTRIBS_INTEL: &[&str] = &["GenuineIntel"];
    const VMM_FLAGS: &[&str] = &["hypervisor"];

    pub const ARCH_CPU_VENDOR_FIELD: &str = check::GENERIC_CPU_VENDOR_FIELD;
    pub const ARCH_CPU_MODEL_FIELD: &str = check::GENERIC_CPU_MODEL_FIELD;

    macro_rules! sl {
         () => {
             slog_scope::logger().new(o!("subsystem" => "x86_64"))
         };
    }

    // List of check functions
    static CHECK_LIST: &[CheckItem] = &[
        CheckItem {
            name: CheckType::Cpu,
            descr: "This parameter performs the cpu check",
            fp: check_cpu,
            perm: PermissionType::NonPrivileged,
        },
        CheckItem {
            name: CheckType::KernelModules,
            descr: "This parameter performs the kvm check",
            fp: check_kernel_modules,
            perm: PermissionType::NonPrivileged,
        },
        CheckItem {
            name: CheckType::KvmIsUsable,
            descr: "This parameter performs check to see if KVM is usable",
            fp: check_kvm_is_usable,
            perm: PermissionType::Privileged,
        },
    ];

    static MODULE_LIST: &[KernelModule] = &[
        KernelModule {
            name: "kvm",
            params: &[KernelParam {
                name: "kvmclock_periodic_sync",
                value: KernelParamType::Simple("Y"),
            }],
        },
        KernelModule {
            name: "kvm_intel",
            params: &[KernelParam {
                name: "unrestricted_guest",
                value: KernelParamType::Predicate(unrestricted_guest_param_check),
            }],
        },
        KernelModule {
            name: "vhost",
            params: &[],
        },
        KernelModule {
            name: "vhost_net",
            params: &[],
        },
        KernelModule {
            name: "vhost_vsock",
            params: &[],
        },
    ];

    pub fn get_checks() -> Option<&'static [CheckItem<'static>]> {
        Some(CHECK_LIST)
    }

    // check cpu
    fn check_cpu(_args: &str) -> Result<()> {
        info!(sl!(), "check CPU: x86_64");

        let cpu_info =
            kata_sys_util::cpu::get_single_cpu_info(check::PROC_CPUINFO, CPUINFO_DELIMITER)?;

        let cpu_flags =
            kata_sys_util::cpu::get_cpu_flags(&cpu_info, CPUINFO_FLAGS_TAG).map_err(|e| {
                anyhow!(
                    "Error parsing CPU flags, file {:?}, {:?}",
                    check::PROC_CPUINFO,
                    e
                )
            })?;

        // perform checks
        // TODO: Perform checks based on hypervisor type
        // TODO: Add more information to output (see kata-check in go tool); adjust formatting
        let missing_cpu_attributes = check::check_cpu_attribs(&cpu_info, CPU_ATTRIBS_INTEL)?;
        if !missing_cpu_attributes.is_empty() {
            warn!(sl!(), "Missing CPU attributes {:?}", missing_cpu_attributes);
        }
        let missing_cpu_flags = check::check_cpu_flags(&cpu_flags, CPU_FLAGS_INTEL)?;
        if !missing_cpu_flags.is_empty() {
            warn!(sl!(), "Missing CPU flags {:?}", missing_cpu_flags);
        }

        Ok(())
    }

    pub fn get_cpu_details() -> Result<(String, String)> {
        utils::get_generic_cpu_details(check::PROC_CPUINFO)
    }

    // check if kvm is usable
    fn check_kvm_is_usable(_args: &str) -> Result<()> {
        info!(sl!(), "check if kvm is usable: x86_64");

        let result = check::check_kvm_is_usable_generic();

        result.context("KVM check failed")
    }

    fn running_on_vmm() -> Result<bool> {
        match kata_sys_util::cpu::get_single_cpu_info(check::PROC_CPUINFO, CPUINFO_DELIMITER) {
            Ok(cpu_info) => {
                // check if the 'hypervisor' flag exist in the cpu features
                let missing_hypervisor_flag = check::check_cpu_attribs(&cpu_info, VMM_FLAGS)?;

                if missing_hypervisor_flag.is_empty() {
                    return Ok(true);
                }
            }
            Err(e) => {
                return Err(anyhow!(
                    "Unable to determine if the OS is running on a VM: {}: {}",
                    e,
                    check::PROC_CPUINFO
                ));
            }
        }

        Ok(false)
    }

    // check the host kernel parameter value is valid
    // and check if we are running inside a VMM
    fn unrestricted_guest_param_check(
        module: &str,
        param_name: &str,
        param_value_host: &str,
    ) -> Result<()> {
        let expected_param_value: char = 'Y';

        let running_on_vmm_alt = running_on_vmm()?;

        // Kernel param "unrestricted_guest" is not required when running under a hypervisor
        if running_on_vmm_alt {
            return Ok(());
        }

        if param_value_host == expected_param_value.to_string() {
            Ok(())
        } else {
            let error_msg = format!(
                "Kernel Module: '{:}' parameter '{:}' should have value '{:}', but found '{:}.'.",
                module, param_name, expected_param_value, param_value_host
            );

            let action_msg = format!("Remove the '{:}' module using `rmmod` and then reload using `modprobe`, setting '{:}={:}'",
                module,
                param_name,
                expected_param_value
            );

            Err(anyhow!("{} {}", error_msg, action_msg))
        }
    }

    fn check_kernel_params(kernel_module: &KernelModule) -> Result<()> {
        const MODULES_PATH: &str = "/sys/module";

        for param in kernel_module.params {
            let module_param_path = format!(
                "{}/{}/parameters/{}",
                MODULES_PATH, kernel_module.name, param.name
            );

            // Here the currently loaded kernel parameter value
            // is retrieved and returned on success
            let param_value_host = std::fs::read_to_string(module_param_path)
                .map(|val| val.replace('\n', ""))
                .map_err(|_err| {
                    anyhow!(
                        "'{:}' kernel module parameter `{:}` not found.",
                        kernel_module.name,
                        param.name
                    )
                })?;

            check_kernel_param(
                kernel_module.name,
                param.name,
                &param_value_host,
                param.value.clone(),
            )
            .map_err(|e| anyhow!(e.to_string()))?;
        }
        Ok(())
    }

    fn check_kernel_param(
        module: &str,
        param_name: &str,
        param_value_host: &str,
        param_type: KernelParamType,
    ) -> Result<()> {
        match param_type {
            KernelParamType::Simple(param_value_req) => {
                if param_value_host != param_value_req {
                    return Err(anyhow!(
                        "Kernel module '{}': parameter '{}' should have value '{}', but found '{}'",
                        module,
                        param_name,
                        param_value_req,
                        param_value_host
                    ));
                }
                Ok(())
            }
            KernelParamType::Predicate(pred_func) => {
                pred_func(module, param_name, param_value_host)
            }
        }
    }

    fn check_kernel_modules(_args: &str) -> Result<()> {
        info!(sl!(), "check kernel modules for: x86_64");

        for module in MODULE_LIST {
            let module_loaded = check::check_kernel_module_loaded(module);

            match module_loaded {
                Ok(_) => {
                    let check = check_kernel_params(module);
                    match check {
                        Ok(_v) => info!(sl!(), "{} Ok", module.name),
                        Err(e) => return Err(e),
                    }
                }
                Err(err) => {
                    warn!(sl!(), "{:}", err.replace('\n', ""))
                }
            }
        }
        Ok(())
    }

    pub fn host_is_vmcontainer_capable() -> Result<bool> {
        let mut count = 0;
        if check_cpu("check_cpu").is_err() {
            count += 1;
        };

        if check_kernel_modules("check_modules").is_err() {
            count += 1;
        };

        if count == 0 {
            return Ok(true);
        };

        Err(anyhow!("System is not capable of running a VM"))
    }
}
