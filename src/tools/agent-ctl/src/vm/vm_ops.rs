// Copyright (c) 2024 Microsoft Corporation
//
// SPDX-License-Identifier: Apache-2.0
//
// Description: Boot UVM for testing container storages/volumes.

use crate::vm::{share_fs_utils, vm_utils, TestVm};
use anyhow::{anyhow, Context, Result};
#[cfg(all(
    feature = "cloud-hypervisor",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
use hypervisor::ch::CloudHypervisor;
use hypervisor::{
    device::{
        device_manager::{do_handle_device, DeviceManager},
        DeviceConfig,
    },
    qemu::Qemu,
    BlockConfig, Hypervisor, VsockConfig,
};
use kata_types::config::{
    hypervisor::register_hypervisor_plugin, hypervisor::TopologyConfigInfo,
    hypervisor::HYPERVISOR_NAME_QEMU, QemuConfig,
};
#[cfg(all(
    feature = "cloud-hypervisor",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
use kata_types::config::{hypervisor::HYPERVISOR_NAME_CH, CloudHypervisorConfig};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

// Clh specific configuration path
#[cfg(all(
    feature = "cloud-hypervisor",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
const CLH_CONFIG_PATH: &str =
    "/opt/kata/share/defaults/kata-containers/runtime-rs/configuration-cloud-hypervisor.toml";

// qemu specific configuration path
const QEMU_CONFIG_PATH: &str =
    "/opt/kata/share/defaults/kata-containers/runtime-rs/configuration-qemu-runtime-rs.toml";

const VM_NAME: &str = "agent-ctl-testvm";
const VM_START_TIMEOUT: i32 = 10_000;

// Boot the test vm.
// In summary, this method
// - parses hypervisor specific kata config file
// - loads hypervisor specific config
// - instantiates a hypervisor object
// - calls prepare_vm
// - instantiates device manager to handle devices
// - calls start_vm to boot pod vm
// - retrieves the agent ttrpc server socket address
pub(crate) async fn boot_vm(name: &str) -> Result<TestVm> {
    #[allow(clippy::needless_late_init)]
    let config_path;
    #[allow(unused_mut)]
    let mut is_hybrid_vsock = false;

    // Register the hypervisor config plugin
    match name {
        #[cfg(all(
            feature = "cloud-hypervisor",
            any(target_arch = "x86_64", target_arch = "aarch64")
        ))]
        HYPERVISOR_NAME_CH => {
            register_hypervisor_plugin(HYPERVISOR_NAME_CH, Arc::new(CloudHypervisorConfig::new()));
            config_path = CLH_CONFIG_PATH;
            is_hybrid_vsock = true;
        }
        &_ => {
            register_hypervisor_plugin(HYPERVISOR_NAME_QEMU, Arc::new(QemuConfig::new()));
            config_path = QEMU_CONFIG_PATH;
        }
    };

    // get the kata configuration toml
    let toml_config = vm_utils::load_config(config_path)?;

    let hypervisor_config = toml_config
        .hypervisor
        .get(name)
        .ok_or_else(|| anyhow!("Failed to get hypervisor config"))
        .context("get hypervisor config")?;

    let hypervisor: Arc<dyn Hypervisor> = match name {
        #[cfg(all(
            feature = "cloud-hypervisor",
            any(target_arch = "x86_64", target_arch = "aarch64")
        ))]
        HYPERVISOR_NAME_CH => {
            let hyp_ch = Arc::new(CloudHypervisor::new());
            hyp_ch
                .set_hypervisor_config(hypervisor_config.clone())
                .await;
            hyp_ch
        }
        &_ => {
            let hyp_qemu = Arc::new(Qemu::new());
            hyp_qemu
                .set_hypervisor_config(hypervisor_config.clone())
                .await;
            hyp_qemu
        }
    };

    // prepare vm
    // we do not pass any network namesapce since we dont want any
    let empty_anno_map: HashMap<String, String> = HashMap::new();
    hypervisor
        .prepare_vm(VM_NAME, None, &empty_anno_map, None)
        .await
        .context(" prepare test vm")?;

    // instantiate device manager
    let topo_config = TopologyConfigInfo::new(&toml_config);
    let dev_manager = Arc::new(RwLock::new(
        DeviceManager::new(hypervisor.clone(), topo_config.as_ref())
            .await
            .context("failed to create device manager")?,
    ));

    // For qemu, we need some additional device handling
    // - vsock device
    // - block device for rootfs if using image
    if name.contains(HYPERVISOR_NAME_QEMU) {
        add_vsock_device(dev_manager.clone())
            .await
            .context("qemu::adding vsock device")?;

        if !hypervisor_config.boot_info.image.is_empty() {
            let blk_config = BlockConfig {
                path_on_host: hypervisor_config.boot_info.image.clone(),
                is_readonly: true,
                driver_option: hypervisor_config.boot_info.vm_rootfs_driver.clone(),
                ..Default::default()
            };
            add_block_device(dev_manager.clone(), blk_config)
                .await
                .context("qemu: handle rootfs")?;
        }
    }

    // setup filesystem sharing using virtio-fs
    let fs_info =
        share_fs_utils::setup_virtio_fs(hypervisor.clone(), dev_manager.clone(), name).await?;

    // start vm
    if let Err(e) = hypervisor.start_vm(VM_START_TIMEOUT).await {
        // shutdown the virtiofs daemon
        let _ = share_fs_utils::shutdown_virtiofsd(fs_info).await;
        return Err(anyhow!("start_vm error: {:?}", e));
    }

    let agent_socket_addr = hypervisor
        .get_agent_socket()
        .await
        .context("get agent socket path")?;

    // return the vm structure
    Ok(TestVm {
        hypervisor_name: name.to_string(),
        hypervisor_instance: hypervisor,
        socket_addr: agent_socket_addr,
        hybrid_vsock: is_hybrid_vsock,
        share_fs: fs_info,
    })
}

pub(crate) async fn stop_vm(instance: TestVm) -> Result<()> {
    share_fs_utils::shutdown_virtiofsd(instance.share_fs).await?;
    instance
        .hypervisor_instance
        .stop_vm()
        .await
        .context("stopping pod vm")
}

async fn add_block_device(dev_mgr: Arc<RwLock<DeviceManager>>, cfg: BlockConfig) -> Result<()> {
    do_handle_device(&dev_mgr, &DeviceConfig::BlockCfg(cfg))
        .await
        .context("handle block device failed")?;
    Ok(())
}

async fn add_vsock_device(dev_mgr: Arc<RwLock<DeviceManager>>) -> Result<()> {
    let vsock_config = VsockConfig {
        guest_cid: libc::VMADDR_CID_ANY,
    };

    do_handle_device(&dev_mgr, &DeviceConfig::VsockCfg(vsock_config))
        .await
        .context("handle vsock device failed")?;
    Ok(())
}
