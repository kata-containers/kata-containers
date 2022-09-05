// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use super::vmm_instance::VmmInstance;
use crate::{
    device::Device, hypervisor_persist::HypervisorState, kernel_param::KernelParams, VmmState,
    HYPERVISOR_DRAGONBALL, VM_ROOTFS_DRIVER_BLK,
};
use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use dragonball::{
    api::v1::{BlockDeviceConfigInfo, BootSourceConfig},
    vm::VmConfigInfo,
};
use kata_sys_util::mount;
use kata_types::config::hypervisor::Hypervisor as HypervisorConfig;
use persist::{sandbox_persist::Persist, KATA_PATH};
use std::{collections::HashSet, fs::create_dir_all, path::PathBuf};

const DRAGONBALL_KERNEL: &str = "vmlinux";
const DRAGONBALL_ROOT_FS: &str = "rootfs";

unsafe impl Send for DragonballInner {}
unsafe impl Sync for DragonballInner {}
pub struct DragonballInner {
    /// sandbox id
    pub(crate) id: String,

    /// vm path
    pub(crate) vm_path: String,

    /// jailed flag
    pub(crate) jailed: bool,

    /// chroot base for the jailer
    pub(crate) jailer_root: String,

    /// netns
    pub(crate) netns: Option<String>,

    /// hypervisor config
    pub(crate) config: HypervisorConfig,

    /// vmm state
    pub(crate) state: VmmState,

    /// vmm instance
    pub(crate) vmm_instance: VmmInstance,

    /// hypervisor run dir
    pub(crate) run_dir: String,

    /// pending device
    pub(crate) pending_devices: Vec<Device>,

    /// cached block device
    pub(crate) cached_block_devices: HashSet<String>,
}

impl DragonballInner {
    pub fn new() -> DragonballInner {
        DragonballInner {
            id: "".to_string(),
            vm_path: "".to_string(),
            jailer_root: "".to_string(),
            netns: None,
            config: Default::default(),
            pending_devices: vec![],
            state: VmmState::NotReady,
            jailed: false,
            vmm_instance: VmmInstance::new(""),
            run_dir: "".to_string(),
            cached_block_devices: Default::default(),
        }
    }

    pub(crate) async fn cold_start_vm(&mut self, timeout: i32) -> Result<()> {
        info!(sl!(), "start sandbox cold");

        self.set_vm_base_config().context("set vm base config")?;

        // get rootfs driver
        let rootfs_driver = self.config.blockdev_info.block_device_driver.clone();

        // get kernel params
        let mut kernel_params = KernelParams::new(self.config.debug_info.enable_debug);
        kernel_params.append(&mut KernelParams::new_rootfs_kernel_params(&rootfs_driver));
        kernel_params.append(&mut KernelParams::from_string(
            &self.config.boot_info.kernel_params,
        ));

        // set boot source
        let kernel_path = self.config.boot_info.kernel.clone();
        self.set_boot_source(
            &kernel_path,
            &kernel_params
                .to_string()
                .context("kernel params to string")?,
        )
        .context("set_boot_source")?;

        // get vm rootfs
        let image = {
            let initrd_path = self.config.boot_info.initrd.clone();
            let image_path = self.config.boot_info.image.clone();
            if !initrd_path.is_empty() {
                Ok(initrd_path)
            } else if !image_path.is_empty() {
                Ok(image_path)
            } else {
                Err(anyhow!("failed to get image"))
            }
        }
        .context("get image")?;
        self.set_vm_rootfs(&image, &rootfs_driver)
            .context("set vm rootfs")?;

        // add pending devices
        while let Some(dev) = self.pending_devices.pop() {
            self.add_device(dev).await.context("add_device")?;
        }

        // start vmm and wait ready
        self.start_vmm_instance().context("start vmm instance")?;
        self.wait_vmm_ready(timeout).context("wait vmm")?;

        Ok(())
    }

    pub(crate) fn run_vmm_server(&mut self) -> Result<()> {
        if !self.config.jailer_path.is_empty() {
            self.jailed = true;
        }

        // create jailer root
        create_dir_all(self.jailer_root.as_str())
            .map_err(|e| anyhow!("Failed to create dir {} err : {:?}", self.jailer_root, e))?;

        // create run dir
        self.run_dir = [KATA_PATH, self.id.as_str()].join("/");
        create_dir_all(self.run_dir.as_str())
            .with_context(|| format!("failed to create dir {}", self.run_dir.as_str()))?;

        // run vmm server
        self.vmm_instance
            .run_vmm_server(&self.id, self.netns.clone())
            .context("run vmm server")?;
        self.state = VmmState::VmmServerReady;

        Ok(())
    }

    pub(crate) fn cleanup_resource(&self) {
        if self.jailed {
            self.umount_jail_resource(DRAGONBALL_KERNEL).ok();
            self.umount_jail_resource(DRAGONBALL_ROOT_FS).ok();
            for id in &self.cached_block_devices {
                self.umount_jail_resource(id.as_str()).ok();
            }
        }

        std::fs::remove_dir_all(&self.vm_path)
            .map_err(|err| {
                error!(sl!(), "failed to remove dir all for {}", &self.vm_path);
                err
            })
            .ok();
    }

    fn set_vm_base_config(&mut self) -> Result<()> {
        let serial_path = [&self.run_dir, "console.sock"].join("/");
        let vm_config = VmConfigInfo {
            serial_path: Some(serial_path),
            mem_size_mib: self.config.memory_info.default_memory as usize,
            vcpu_count: self.config.cpu_info.default_vcpus as u8,
            max_vcpu_count: self.config.cpu_info.default_maxvcpus as u8,
            ..Default::default()
        };
        info!(sl!(), "vm config: {:?}", vm_config);

        self.vmm_instance
            .set_vm_configuration(vm_config)
            .context("set vm configuration")
    }

    pub(crate) fn umount_jail_resource(&self, jailed_path: &str) -> Result<()> {
        let path = [self.jailer_root.as_str(), jailed_path].join("/");
        nix::mount::umount2(path.as_str(), nix::mount::MntFlags::MNT_DETACH)
            .with_context(|| format!("umount path {}", &path))
    }

    pub(crate) fn get_resource(&self, src: &str, dst: &str) -> Result<String> {
        if self.jailed {
            self.jail_resource(src, dst)
        } else {
            Ok(src.to_string())
        }
    }

    fn jail_resource(&self, src: &str, dst: &str) -> Result<String> {
        info!(sl!(), "jail resource: src {} dst {}", src, dst);
        if src.is_empty() || dst.is_empty() {
            return Err(anyhow!("invalid param src {} dst {}", src, dst));
        }

        let jailed_location = [self.jailer_root.as_str(), dst].join("/");
        mount::bind_mount_unchecked(src, jailed_location.as_str(), false).context("bind_mount")?;

        let mut abs_path = String::from("/");
        abs_path.push_str(dst);
        Ok(abs_path)
    }

    fn set_boot_source(&mut self, kernel_path: &str, kernel_params: &str) -> Result<()> {
        info!(
            sl!(),
            "kernel path {} kernel params {}", kernel_path, kernel_params
        );

        let mut boot_cfg = BootSourceConfig {
            kernel_path: self
                .get_resource(kernel_path, DRAGONBALL_KERNEL)
                .context("get resource")?,
            ..Default::default()
        };

        if !kernel_params.is_empty() {
            boot_cfg.boot_args = Some(kernel_params.to_string());
        }

        self.vmm_instance
            .put_boot_source(boot_cfg)
            .context("put boot source")
    }

    fn set_vm_rootfs(&mut self, path: &str, driver: &str) -> Result<()> {
        info!(sl!(), "set vm rootfs {} {}", path, driver);
        let jail_drive = self
            .get_resource(path, DRAGONBALL_ROOT_FS)
            .context("get resource")?;

        if driver == VM_ROOTFS_DRIVER_BLK {
            let blk_cfg = BlockDeviceConfigInfo {
                path_on_host: PathBuf::from(jail_drive),
                drive_id: DRAGONBALL_ROOT_FS.to_string(),
                is_root_device: false,
                // Add it as a regular block device
                // This allows us to use a partitioned root block device
                // is_read_only
                is_read_only: true,
                is_direct: false,
                ..Default::default()
            };

            self.vmm_instance
                .insert_block_device(blk_cfg)
                .context("inert block device")
        } else {
            Err(anyhow!(
                "Unknown vm_rootfs driver {} path {:?}",
                driver,
                path
            ))
        }
    }

    fn start_vmm_instance(&mut self) -> Result<()> {
        info!(sl!(), "Starting VM");
        self.vmm_instance
            .instance_start()
            .context("Failed to start vmm")?;
        self.state = VmmState::VmRunning;
        Ok(())
    }

    // wait_vmm_ready will wait for timeout seconds for the VMM to be up and running.
    // This does not mean that the VM is up and running. It only indicates that the VMM is up and
    // running and able to handle commands to setup and launch a VM
    fn wait_vmm_ready(&mut self, timeout: i32) -> Result<()> {
        if timeout < 0 {
            return Err(anyhow!("Invalid param timeout {}", timeout));
        }

        let time_start = std::time::Instant::now();
        loop {
            match self.vmm_instance.is_running() {
                Ok(_) => return Ok(()),
                Err(err) => {
                    let time_now = std::time::Instant::now();
                    if time_now.duration_since(time_start).as_millis() > timeout as u128 {
                        return Err(anyhow!(
                            "waiting vmm ready timeout {} err: {:?}",
                            timeout,
                            err
                        ));
                    }
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
            }
        }
    }

    pub fn set_hypervisor_config(&mut self, config: HypervisorConfig) {
        self.config = config;
    }

    pub fn hypervisor_config(&self) -> HypervisorConfig {
        self.config.clone()
    }
}

#[async_trait]
impl Persist for DragonballInner {
    type State = HypervisorState;
    type ConstructorArgs = ();

    /// Save a state of hypervisor
    async fn save(&self) -> Result<Self::State> {
        Ok(HypervisorState {
            hypervisor_type: HYPERVISOR_DRAGONBALL.to_string(),
            id: self.id.clone(),
            vm_path: self.vm_path.clone(),
            jailed: self.jailed,
            jailer_root: self.jailer_root.clone(),
            netns: self.netns.clone(),
            config: self.hypervisor_config(),
            run_dir: self.run_dir.clone(),
            cached_block_devices: self.cached_block_devices.clone(),
            ..Default::default()
        })
    }

    /// Restore hypervisor
    async fn restore(
        _hypervisor_args: Self::ConstructorArgs,
        hypervisor_state: Self::State,
    ) -> Result<Self> {
        Ok(DragonballInner {
            id: hypervisor_state.id,
            vm_path: hypervisor_state.vm_path,
            jailed: hypervisor_state.jailed,
            jailer_root: hypervisor_state.jailer_root,
            netns: hypervisor_state.netns,
            config: hypervisor_state.config,
            state: VmmState::NotReady,
            vmm_instance: VmmInstance::new(""),
            run_dir: hypervisor_state.run_dir,
            pending_devices: vec![],
            cached_block_devices: hypervisor_state.cached_block_devices,
        })
    }
}
