// Copyright (C) 2022 Alibaba Cloud. All rights reserved.
//
// SPDX-License-Identifier: Apache-2.0

#[cfg(test)]
pub mod tests {
    use crate::api::v1::InstanceInfo;
    use crate::vm::{CpuTopology, KernelConfigInfo, Vm, VmConfigInfo};
    use dbs_utils::epoll_manager::EpollManager;
    use linux_loader::cmdline::Cmdline;
    use std::sync::{Arc, RwLock};
    use vmm_sys_util::tempfile::TempFile;

    pub fn create_vm_for_test() -> Vm {
        // Call for kvm too frequently would cause error in some host kernel.
        let instance_info = Arc::new(RwLock::new(InstanceInfo::default()));
        let epoll_manager = EpollManager::default();
        let mut vm = Vm::new(None, instance_info, epoll_manager).unwrap();
        let kernel_file = TempFile::new().unwrap();
        let cmd_line = Cmdline::new(64).unwrap();
        vm.set_kernel_config(KernelConfigInfo::new(
            kernel_file.into_file(),
            None,
            cmd_line,
        ));

        let vm_config = VmConfigInfo {
            vcpu_count: 1,
            max_vcpu_count: 1,
            cpu_pm: "off".to_string(),
            mem_type: "shmem".to_string(),
            mem_file_path: "".to_string(),
            mem_size_mib: 1,
            serial_path: None,
            cpu_topology: CpuTopology {
                threads_per_core: 1,
                cores_per_die: 1,
                dies_per_socket: 1,
                sockets: 1,
            },
            vpmu_feature: 0,
            pci_hotplug_enabled: false,
        };
        vm.set_vm_config(vm_config);
        vm.init_guest_memory().unwrap();
        vm
    }
}
