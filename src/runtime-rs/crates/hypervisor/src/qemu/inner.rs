// Copyright (c) 2022 Red Hat
//
// SPDX-License-Identifier: Apache-2.0
//

use super::cmdline_generator::{get_network_device, QemuCmdLine, QMP_SOCKET_FILE};
use super::qmp::Qmp;
use crate::{
    hypervisor_persist::HypervisorState, utils::enter_netns, HypervisorConfig, MemoryConfig,
    VcpuThreadIds, VsockDevice, HYPERVISOR_QEMU,
};
use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use kata_sys_util::netns::NetnsGuard;
use kata_types::{
    capabilities::{Capabilities, CapabilityBits},
    config::KATA_PATH,
};
use persist::sandbox_persist::Persist;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::convert::TryInto;
use std::path::Path;
use std::process::Stdio;
use tokio::sync::{mpsc, Mutex};
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::{Child, ChildStderr, Command},
};

const VSOCK_SCHEME: &str = "vsock";

#[derive(Debug)]
pub struct QemuInner {
    /// sandbox id
    id: String,

    qemu_process: Mutex<Option<Child>>,
    qmp: Option<Qmp>,

    config: HypervisorConfig,
    devices: Vec<DeviceType>,
    netns: Option<String>,

    exit_notify: Option<mpsc::Sender<()>>,
}

impl QemuInner {
    pub fn new(exit_notify: mpsc::Sender<()>) -> QemuInner {
        QemuInner {
            id: "".to_string(),
            qemu_process: Mutex::new(None),
            qmp: None,
            config: Default::default(),
            devices: Vec::new(),
            netns: None,

            exit_notify: Some(exit_notify),
        }
    }

    pub(crate) async fn prepare_vm(&mut self, id: &str, netns: Option<String>) -> Result<()> {
        info!(sl!(), "Preparing QEMU VM");
        self.id = id.to_string();
        self.netns = netns;

        let vm_path = [KATA_PATH, self.id.as_str()].join("/");
        std::fs::create_dir_all(vm_path)?;

        Ok(())
    }

    pub(crate) async fn start_vm(&mut self, _timeout: i32) -> Result<()> {
        info!(sl!(), "Starting QEMU VM");
        let netns = self.netns.clone().unwrap_or_default();

        // CAUTION: since 'cmdline' contains file descriptors that have to stay
        // open until spawn() is called to launch qemu later in this function,
        // 'cmdline' has to live at least until spawn() is called
        let mut cmdline = QemuCmdLine::new(&self.id, &self.config)?;

        for device in &mut self.devices {
            match device {
                DeviceType::ShareFs(share_fs_dev) => {
                    if share_fs_dev.config.fs_type == "virtio-fs" {
                        cmdline.add_virtiofs_share(
                            &share_fs_dev.config.sock_path,
                            &share_fs_dev.config.mount_tag,
                            share_fs_dev.config.queue_size,
                        );
                    }
                }
                DeviceType::Vsock(vsock_dev) => {
                    let fd = vsock_dev.init_config().await?;
                    cmdline.add_vsock(fd, vsock_dev.config.guest_cid)?;
                }
                DeviceType::Block(block_dev) => {
                    if block_dev.config.path_on_host == self.config.boot_info.initrd {
                        // If this block device represents initrd we ignore it here, it
                        // will be handled elsewhere by adding `-initrd` to the qemu
                        // command line.
                        continue;
                    }
                    match block_dev.config.driver_option.as_str() {
                        "nvdimm" => cmdline.add_nvdimm(
                            &block_dev.config.path_on_host,
                            block_dev.config.is_readonly,
                        )?,
                        "ccw" => cmdline.add_block_device(
                            block_dev.device_id.as_str(),
                            &block_dev.config.path_on_host,
                            self.config.blockdev_info.block_device_cache_direct,
                        )?,
                        unsupported => {
                            info!(sl!(), "unsupported block device driver: {}", unsupported)
                        }
                    }
                }
                DeviceType::Network(network) => {
                    // we need ensure add_network_device happens in netns.
                    let _netns_guard = NetnsGuard::new(&netns).context("new netns guard")?;

                    cmdline.add_network_device(
                        &network.config.host_dev_name,
                        network.config.guest_mac.clone().unwrap(),
                    )?;
                }
                _ => info!(sl!(), "qemu cmdline: unsupported device: {:?}", device),
            }
        }
        // To get access to the VM console for debugging, enable the following
        // line and replace its argument appropriately (open a terminal, run
        // `tty` in it to get its device file path and use it as the argument).
        //cmdline.add_serial_console("/dev/pts/23");

        // Add a console to the devices of the cmdline
        let console_socket_path = Path::new(&self.get_jailer_root().await?).join("console.sock");
        cmdline.add_console(console_socket_path.to_str().unwrap());

        info!(sl!(), "qemu args: {}", cmdline.build().await?.join(" "));
        let mut command = Command::new(&self.config.path);
        command.args(cmdline.build().await?);

        info!(sl!(), "qemu cmd: {:?}", command);

        // we need move the qemu process into Network Namespace.
        unsafe {
            let _pre_exec = command.pre_exec(move || {
                let _ = enter_netns(&netns);

                Ok(())
            });
        }

        let mut qemu_process = command.stderr(Stdio::piped()).spawn()?;
        let stderr = qemu_process.stderr.take().unwrap();
        self.qemu_process = Mutex::new(Some(qemu_process));

        info!(sl!(), "qemu process started");

        let exit_notify: mpsc::Sender<()> = self
            .exit_notify
            .take()
            .ok_or_else(|| anyhow!("no exit notify"))?;

        tokio::spawn(log_qemu_stderr(stderr, exit_notify));

        match Qmp::new(QMP_SOCKET_FILE) {
            Ok(qmp) => self.qmp = Some(qmp),
            Err(e) => {
                error!(sl!(), "couldn't initialise QMP: {:?}", e);
                return Err(e);
            }
        }

        Ok(())
    }

    pub(crate) async fn stop_vm(&mut self) -> Result<()> {
        info!(sl!(), "Stopping QEMU VM");

        let mut qemu_process = self.qemu_process.lock().await;
        if let Some(qemu_process) = qemu_process.as_mut() {
            let is_qemu_running = qemu_process.id().is_some();
            if is_qemu_running {
                info!(sl!(), "QemuInner::stop_vm(): kill()'ing qemu");
                qemu_process.kill().await.map_err(anyhow::Error::from)
            } else {
                info!(
                    sl!(),
                    "QemuInner::stop_vm(): qemu process isn't running (likely stopped already)"
                );
                Ok(())
            }
        } else {
            Err(anyhow!("qemu process has not been started yet"))
        }
    }

    pub(crate) async fn wait_vm(&self) -> Result<i32> {
        let mut qemu_process = self.qemu_process.lock().await;

        if let Some(mut qemu_process) = qemu_process.take() {
            let status = qemu_process.wait().await?;
            Ok(status.code().unwrap_or(0))
        } else {
            Err(anyhow!("the process has been reaped"))
        }
    }

    pub(crate) fn pause_vm(&self) -> Result<()> {
        info!(sl!(), "Pausing QEMU VM");
        todo!()
    }

    pub(crate) fn resume_vm(&self) -> Result<()> {
        info!(sl!(), "Resuming QEMU VM");
        todo!()
    }

    pub(crate) async fn save_vm(&self) -> Result<()> {
        todo!()
    }

    pub(crate) async fn get_agent_socket(&self) -> Result<String> {
        info!(sl!(), "QemuInner::get_agent_socket()");
        let guest_cid = match &self.get_agent_vsock_dev() {
            Some(device) => device.config.guest_cid,
            None => return Err(anyhow!("uninitialized agent vsock".to_owned())),
        };

        Ok(format!("{}://{}", VSOCK_SCHEME, guest_cid))
    }

    pub(crate) async fn disconnect(&mut self) {
        info!(sl!(), "QemuInner::disconnect()");
        todo!()
    }

    pub(crate) async fn get_thread_ids(&self) -> Result<VcpuThreadIds> {
        info!(sl!(), "QemuInner::get_thread_ids()");
        //todo!()
        let vcpu_thread_ids: VcpuThreadIds = VcpuThreadIds {
            vcpus: HashMap::new(),
        };
        Ok(vcpu_thread_ids)
    }

    pub(crate) async fn get_vmm_master_tid(&self) -> Result<u32> {
        info!(sl!(), "QemuInner::get_vmm_master_tid()");
        if let Some(qemu_process) = self.qemu_process.lock().await.as_ref() {
            if let Some(qemu_pid) = qemu_process.id() {
                info!(
                    sl!(),
                    "QemuInner::get_vmm_master_tid(): returning {}", qemu_pid
                );
                Ok(qemu_pid)
            } else {
                Err(anyhow!("QemuInner::get_vmm_master_tid(): qemu process isn't running (likely stopped already)"))
            }
        } else {
            Err(anyhow!("qemu process not running"))
        }
    }

    pub(crate) async fn get_ns_path(&self) -> Result<String> {
        info!(sl!(), "QemuInner::get_ns_path()");
        Ok(format!(
            "/proc/{}/task/{}/ns",
            std::process::id(),
            std::process::id()
        ))
    }

    pub(crate) async fn cleanup(&self) -> Result<()> {
        info!(sl!(), "QemuInner::cleanup()");
        let vm_path = [KATA_PATH, self.id.as_str()].join("/");
        std::fs::remove_dir_all(vm_path)?;
        Ok(())
    }

    pub(crate) async fn resize_vcpu(
        &mut self,
        old_vcpus: u32,
        mut new_vcpus: u32,
    ) -> Result<(u32, u32)> {
        info!(
            sl!(),
            "QemuInner::resize_vcpu(): {} -> {}", old_vcpus, new_vcpus
        );

        // TODO The following sanity checks apparently have to be performed by
        // any hypervisor - wouldn't it make sense to move them to the caller?
        if new_vcpus == old_vcpus {
            return Ok((old_vcpus, new_vcpus));
        }

        if new_vcpus == 0 {
            return Err(anyhow!("resize to 0 vcpus requested"));
        }

        if new_vcpus > self.config.cpu_info.default_maxvcpus {
            warn!(
                sl!(),
                "Cannot allocate more vcpus than the max allowed number of vcpus. The maximum allowed amount of vcpus will be used instead.");
            new_vcpus = self.config.cpu_info.default_maxvcpus;
        }

        if let Some(ref mut qmp) = self.qmp {
            match new_vcpus.cmp(&old_vcpus) {
                Ordering::Greater => {
                    let hotplugged = qmp.hotplug_vcpus(new_vcpus - old_vcpus)?;
                    new_vcpus = old_vcpus + hotplugged;
                }
                Ordering::Less => {
                    let hotunplugged = qmp.hotunplug_vcpus(old_vcpus - new_vcpus)?;
                    new_vcpus = old_vcpus - hotunplugged;
                }
                Ordering::Equal => {}
            }
        }

        Ok((old_vcpus, new_vcpus))
    }

    pub(crate) async fn get_pids(&self) -> Result<Vec<u32>> {
        info!(sl!(), "QemuInner::get_pids()");
        todo!()
    }

    pub(crate) async fn check(&self) -> Result<()> {
        todo!()
    }

    pub(crate) async fn get_jailer_root(&self) -> Result<String> {
        Ok("".into())
    }

    pub(crate) async fn capabilities(&self) -> Result<Capabilities> {
        let mut caps = Capabilities::default();
        caps.set(CapabilityBits::FsSharingSupport);
        Ok(caps)
    }

    pub fn set_hypervisor_config(&mut self, config: HypervisorConfig) {
        self.config = config;
    }

    pub fn hypervisor_config(&self) -> HypervisorConfig {
        info!(sl!(), "QemuInner::hypervisor_config()");
        self.config.clone()
    }

    pub(crate) async fn get_hypervisor_metrics(&self) -> Result<String> {
        todo!()
    }

    pub(crate) fn set_capabilities(&mut self, _flag: CapabilityBits) {
        todo!()
    }

    pub(crate) fn set_guest_memory_block_size(&mut self, size: u32) {
        if let Some(ref mut qmp) = self.qmp {
            info!(
                sl!(),
                "QemuInner::set_guest_memory_block_size(): block size set to {}", size
            );
            qmp.set_guest_memory_block_size(size.into());
        } else {
            warn!(
                sl!(),
                "QemuInner::set_guest_memory_block_size(): QMP not initialized"
            );
        }
    }

    pub(crate) fn guest_memory_block_size(&self) -> u32 {
        if let Some(qmp) = &self.qmp {
            qmp.guest_memory_block_size() as u32
        } else {
            warn!(
                sl!(),
                "QemuInner::guest_memory_block_size(): QMP not initialized"
            );
            0
        }
    }

    pub(crate) fn resize_memory(
        &mut self,
        mut new_total_mem_mb: u32,
    ) -> Result<(u32, MemoryConfig)> {
        info!(
            sl!(),
            "QemuInner::resize_memory(): asked to resize memory to {} MB", new_total_mem_mb
        );

        // stick to the apparent de facto convention and represent megabytes
        // as u32 and bytes as u64
        fn bytes_to_megs(bytes: u64) -> u32 {
            (bytes / (1 << 20)) as u32
        }
        fn megs_to_bytes(bytes: u32) -> u64 {
            bytes as u64 * (1 << 20)
        }

        let qmp = match self.qmp {
            Some(ref mut qmp) => qmp,
            None => {
                warn!(sl!(), "QemuInner::resize_memory(): QMP not initialized");
                return Err(anyhow!("QMP not initialized"));
            }
        };

        let coldplugged_mem = megs_to_bytes(self.config.memory_info.default_memory);
        let new_total_mem = megs_to_bytes(new_total_mem_mb);

        if new_total_mem < coldplugged_mem {
            return Err(anyhow!(
                "asked to resize to {} M but that is less than cold-plugged memory size ({})",
                new_total_mem_mb,
                bytes_to_megs(coldplugged_mem)
            ));
        }

        let guest_mem_block_size = qmp.guest_memory_block_size();

        let mut new_hotplugged_mem = new_total_mem - coldplugged_mem;

        info!(
            sl!(),
            "new hotplugged mem before alignment: {} B ({} MB)",
            new_hotplugged_mem,
            bytes_to_megs(new_hotplugged_mem)
        );

        let is_unaligned = new_hotplugged_mem % guest_mem_block_size != 0;
        if is_unaligned {
            new_hotplugged_mem = ch_config::convert::checked_next_multiple_of(
                new_hotplugged_mem,
                guest_mem_block_size,
            )
            .ok_or(anyhow!(format!(
                "alignment of {} B to the block size of {} B failed",
                new_hotplugged_mem, guest_mem_block_size
            )))?
        }
        let new_hotplugged_mem = new_hotplugged_mem;

        info!(
            sl!(),
            "new hotplugged mem after alignment: {} B ({} MB)",
            new_hotplugged_mem,
            bytes_to_megs(new_hotplugged_mem)
        );

        let max_total_mem = megs_to_bytes(self.config.memory_info.default_maxmemory);
        if coldplugged_mem + new_hotplugged_mem > max_total_mem {
            return Err(anyhow!(
                "requested memory ({} M) is greater than maximum allowed ({} M)",
                bytes_to_megs(coldplugged_mem + new_hotplugged_mem),
                bytes_to_megs(max_total_mem)
            ));
        }

        let cur_hotplugged_memory = qmp.hotplugged_memory_size()?;
        info!(
            sl!(),
            "hotplug memory {} -> {}", cur_hotplugged_memory, new_hotplugged_mem
        );

        match new_hotplugged_mem.cmp(&cur_hotplugged_memory) {
            Ordering::Greater => {
                info!(
                    sl!(),
                    "hotplugging {} B of memory",
                    new_hotplugged_mem - cur_hotplugged_memory
                );
                qmp.hotplug_memory(new_hotplugged_mem - cur_hotplugged_memory)
                    .context("qemu hotplug memory")?;
                info!(
                    sl!(),
                    "hotplugged memory after hotplugging: {}",
                    qmp.hotplugged_memory_size()?
                );

                new_total_mem_mb = bytes_to_megs(coldplugged_mem + new_hotplugged_mem);
            }
            Ordering::Less => {
                info!(
                    sl!(),
                    "hotunplugging {} B of memory",
                    cur_hotplugged_memory - new_hotplugged_mem
                );
                let res =
                    qmp.hotunplug_memory((cur_hotplugged_memory - new_hotplugged_mem).try_into()?);
                if let Err(err) = res {
                    info!(sl!(), "hotunplugging failed: {:?}", err);
                } else {
                    new_total_mem_mb = bytes_to_megs(coldplugged_mem + new_hotplugged_mem);
                }
                info!(
                    sl!(),
                    "hotplugged memory after hotunplugging: {}",
                    qmp.hotplugged_memory_size()?
                );
            }
            Ordering::Equal => info!(
                sl!(),
                "VM already has the requested amount of memory, nothing to do"
            ),
        }

        Ok((new_total_mem_mb, MemoryConfig::default()))
    }
}

async fn log_qemu_stderr(stderr: ChildStderr, exit_notify: mpsc::Sender<()>) -> Result<()> {
    info!(sl!(), "starting reading qemu stderr");

    let stderr_reader = BufReader::new(stderr);
    let mut stderr_lines = stderr_reader.lines();

    while let Some(buffer) = stderr_lines
        .next_line()
        .await
        .context("next_line() failed on qemu stderr")?
    {
        info!(sl!(), "qemu stderr: {:?}", buffer);
    }

    // Notfiy the waiter the process exit.
    let _ = exit_notify.try_send(());

    info!(sl!(), "finished reading qemu stderr");
    Ok(())
}

use crate::device::DeviceType;

// device manager part of Hypervisor
impl QemuInner {
    pub(crate) async fn add_device(&mut self, mut device: DeviceType) -> Result<DeviceType> {
        info!(sl!(), "QemuInner::add_device() {}", device);
        let is_qemu_ready_to_hotplug = self.qmp.is_some();
        if is_qemu_ready_to_hotplug {
            // hypervisor is running already
            device = self.hotplug_device(device)?;
        } else {
            // store the device to coldplug it later, on hypervisor launch
            self.devices.push(device.clone());
        }
        Ok(device)
    }

    pub(crate) async fn remove_device(&mut self, device: DeviceType) -> Result<()> {
        info!(sl!(), "QemuInner::remove_device() {} ", device);
        Err(anyhow!(
            "QemuInner::remove_device({}): Not yet implemented",
            device
        ))
    }

    fn hotplug_device(&mut self, device: DeviceType) -> Result<DeviceType> {
        let qmp = match self.qmp {
            Some(ref mut qmp) => qmp,
            None => return Err(anyhow!("QMP not initialized")),
        };

        match device {
            DeviceType::Network(ref network_device) => {
                let (netdev, virtio_net_device) = get_network_device(
                    &self.config,
                    &network_device.config.host_dev_name,
                    network_device.config.guest_mac.clone().unwrap(),
                )?;
                qmp.hotplug_network_device(&netdev, &virtio_net_device)?
            }
            _ => info!(sl!(), "hotplugging of {:#?} is unsupported", device),
        }
        Ok(device)
    }
}

// private helpers
impl QemuInner {
    fn get_agent_vsock_dev(&self) -> Option<&VsockDevice> {
        self.devices.iter().find_map(|dev| {
            if let DeviceType::Vsock(vsock_dev) = dev {
                Some(vsock_dev)
            } else {
                None
            }
        })
    }

    pub(crate) async fn update_device(&mut self, device: DeviceType) -> Result<()> {
        info!(sl!(), "QemuInner::update_device() {:?}", &device);

        Ok(())
    }
}

#[async_trait]
impl Persist for QemuInner {
    type State = HypervisorState;
    type ConstructorArgs = mpsc::Sender<()>;

    /// Save a state of hypervisor
    async fn save(&self) -> Result<Self::State> {
        Ok(HypervisorState {
            hypervisor_type: HYPERVISOR_QEMU.to_string(),
            id: self.id.clone(),
            config: self.hypervisor_config(),
            ..Default::default()
        })
    }

    /// Restore hypervisor
    async fn restore(exit_notify: mpsc::Sender<()>, hypervisor_state: Self::State) -> Result<Self> {
        Ok(QemuInner {
            id: hypervisor_state.id,
            qemu_process: Mutex::new(None),
            qmp: None,
            config: hypervisor_state.config,
            devices: Vec::new(),
            netns: None,

            exit_notify: Some(exit_notify),
        })
    }
}
