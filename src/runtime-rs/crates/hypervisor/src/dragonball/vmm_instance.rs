// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::{
    fs::{File, OpenOptions},
    os::unix::{io::IntoRawFd, prelude::AsRawFd},
    sync::{
        mpsc::{channel, Receiver, Sender},
        Arc, Mutex, RwLock,
    },
    thread,
};

use anyhow::{anyhow, Context, Result};
use dragonball::{
    api::v1::{
        BlockDeviceConfigInfo, BootSourceConfig, FsDeviceConfigInfo, FsMountConfigInfo,
        InstanceInfo, InstanceState, VirtioNetDeviceConfigInfo, VmmAction, VmmActionError, VmmData,
        VmmRequest, VmmResponse, VmmService, VsockDeviceConfigInfo,
    },
    vm::VmConfigInfo,
    Vmm,
};
use nix::sched::{setns, CloneFlags};
use seccompiler::BpfProgram;
use vmm_sys_util::eventfd::EventFd;

use crate::ShareFsOperation;

pub enum Request {
    Sync(VmmAction),
}

const DRAGONBALL_VERSION: &str = env!("CARGO_PKG_VERSION");
const REQUEST_RETRY: u32 = 500;
const KVM_DEVICE: &str = "/dev/kvm";

pub struct VmmInstance {
    /// VMM instance info directly accessible from runtime
    vmm_shared_info: Arc<RwLock<InstanceInfo>>,
    to_vmm: Option<Sender<VmmRequest>>,
    from_vmm: Option<Receiver<VmmResponse>>,
    to_vmm_fd: EventFd,
    seccomp: BpfProgram,
    vmm_thread: Option<thread::JoinHandle<Result<i32>>>,
}

impl VmmInstance {
    pub fn new(id: &str) -> Self {
        let vmm_shared_info = Arc::new(RwLock::new(InstanceInfo::new(
            String::from(id),
            DRAGONBALL_VERSION.to_string(),
        )));

        let to_vmm_fd = EventFd::new(libc::EFD_NONBLOCK)
            .unwrap_or_else(|_| panic!("Failed to create eventfd for vmm {}", id));

        VmmInstance {
            vmm_shared_info,
            to_vmm: None,
            from_vmm: None,
            to_vmm_fd,
            seccomp: vec![],
            vmm_thread: None,
        }
    }

    pub fn get_shared_info(&self) -> Arc<RwLock<InstanceInfo>> {
        self.vmm_shared_info.clone()
    }

    fn set_instance_id(&mut self, id: &str) {
        let share_info_lock = self.vmm_shared_info.clone();
        share_info_lock.write().unwrap().id = String::from(id);
    }

    pub fn get_vcpu_tids(&self) -> Vec<(u8, u32)> {
        let info = self.vmm_shared_info.clone();
        let result = info.read().unwrap().tids.clone();
        result
    }

    pub fn run_vmm_server(&mut self, id: &str, netns: Option<String>) -> Result<()> {
        let kvm = OpenOptions::new().read(true).write(true).open(KVM_DEVICE)?;

        let (to_vmm, from_runtime) = channel();
        let (to_runtime, from_vmm) = channel();

        self.set_instance_id(id);

        let vmm_service = VmmService::new(from_runtime, to_runtime);

        self.to_vmm = Some(to_vmm);
        self.from_vmm = Some(from_vmm);

        let api_event_fd2 = self.to_vmm_fd.try_clone().expect("Failed to dup eventfd");
        let vmm = Vmm::new(
            self.vmm_shared_info.clone(),
            api_event_fd2,
            self.seccomp.clone(),
            self.seccomp.clone(),
            Some(kvm.into_raw_fd()),
        )
        .expect("Failed to start vmm");

        self.vmm_thread = Some(
            thread::Builder::new()
                .name("vmm_master".to_owned())
                .spawn(move || {
                    || -> Result<i32> {
                        debug!(sl!(), "run vmm thread start");
                        if let Some(netns_path) = netns {
                            info!(sl!(), "set netns for vmm master {}", &netns_path);
                            let netns_fd = File::open(&netns_path)
                                .with_context(|| format!("open netns path {}", &netns_path))?;
                            setns(netns_fd.as_raw_fd(), CloneFlags::CLONE_NEWNET)
                                .context("set netns ")?;
                        }
                        let exit_code =
                            Vmm::run_vmm_event_loop(Arc::new(Mutex::new(vmm)), vmm_service);
                        debug!(sl!(), "run vmm thread exited: {}", exit_code);
                        Ok(exit_code)
                    }()
                    .map_err(|e| {
                        error!(sl!(), "run vmm thread err. {:?}", e);
                        e
                    })
                })
                .expect("Failed to start vmm event loop"),
        );

        Ok(())
    }

    pub fn put_boot_source(&self, boot_source_cfg: BootSourceConfig) -> Result<()> {
        self.handle_request(Request::Sync(VmmAction::ConfigureBootSource(
            boot_source_cfg,
        )))
        .context("Failed to configure boot source")?;
        Ok(())
    }

    pub fn instance_start(&self) -> Result<()> {
        self.handle_request(Request::Sync(VmmAction::StartMicroVm))
            .context("Failed to start MicroVm")?;
        Ok(())
    }

    pub fn is_uninitialized(&self) -> bool {
        let share_info = self
            .vmm_shared_info
            .read()
            .expect("Failed to read share_info due to poisoned lock");
        matches!(share_info.state, InstanceState::Uninitialized)
    }

    pub fn is_running(&self) -> Result<()> {
        let share_info_lock = self.vmm_shared_info.clone();
        let share_info = share_info_lock
            .read()
            .expect("Failed to read share_info due to poisoned lock");
        if let InstanceState::Running = share_info.state {
            return Ok(());
        }
        Err(anyhow!("vmm is not running"))
    }

    pub fn get_machine_info(&self) -> Result<Box<VmConfigInfo>> {
        if let Ok(VmmData::MachineConfiguration(vm_config)) =
            self.handle_request(Request::Sync(VmmAction::GetVmConfiguration))
        {
            return Ok(vm_config);
        }
        Err(anyhow!("Failed to get machine info"))
    }

    pub fn insert_block_device(&self, device_cfg: BlockDeviceConfigInfo) -> Result<()> {
        self.handle_request_with_retry(Request::Sync(VmmAction::InsertBlockDevice(
            device_cfg.clone(),
        )))
        .with_context(|| format!("Failed to insert block device {:?}", device_cfg))?;
        Ok(())
    }

    pub fn remove_block_device(&self, id: &str) -> Result<()> {
        info!(sl!(), "remove block device {}", id);
        self.handle_request(Request::Sync(VmmAction::RemoveBlockDevice(id.to_string())))
            .with_context(|| format!("Failed to remove block device {:?}", id))?;
        Ok(())
    }

    pub fn set_vm_configuration(&self, vm_config: VmConfigInfo) -> Result<()> {
        self.handle_request(Request::Sync(VmmAction::SetVmConfiguration(
            vm_config.clone(),
        )))
        .with_context(|| format!("Failed to set vm configuration {:?}", vm_config))?;
        Ok(())
    }

    pub fn insert_network_device(&self, net_cfg: VirtioNetDeviceConfigInfo) -> Result<()> {
        self.handle_request_with_retry(Request::Sync(VmmAction::InsertNetworkDevice(
            net_cfg.clone(),
        )))
        .with_context(|| format!("Failed to insert network device {:?}", net_cfg))?;
        Ok(())
    }

    pub fn insert_vsock(&self, vsock_cfg: VsockDeviceConfigInfo) -> Result<()> {
        self.handle_request(Request::Sync(VmmAction::InsertVsockDevice(
            vsock_cfg.clone(),
        )))
        .with_context(|| format!("Failed to insert vsock device {:?}", vsock_cfg))?;
        Ok(())
    }

    pub fn insert_fs(&self, fs_cfg: &FsDeviceConfigInfo) -> Result<()> {
        self.handle_request(Request::Sync(VmmAction::InsertFsDevice(fs_cfg.clone())))
            .with_context(|| format!("Failed to insert {} fs device {:?}", fs_cfg.mode, fs_cfg))?;
        Ok(())
    }

    pub fn patch_fs(&self, cfg: &FsMountConfigInfo, op: ShareFsOperation) -> Result<()> {
        self.handle_request(Request::Sync(VmmAction::ManipulateFsBackendFs(cfg.clone())))
            .with_context(|| {
                format!(
                    "Failed to {:?} backend {:?} at {} mount config {:?}",
                    op, cfg.fstype, cfg.mountpoint, cfg
                )
            })?;
        Ok(())
    }

    pub fn pause(&self) -> Result<()> {
        todo!()
    }

    pub fn resume(&self) -> Result<()> {
        todo!()
    }

    pub fn pid(&self) -> u32 {
        std::process::id()
    }

    pub fn stop(&mut self) -> Result<()> {
        self.handle_request(Request::Sync(VmmAction::ShutdownMicroVm))
            .map_err(|e| {
                warn!(sl!(), "Failed to shutdown MicroVM. {}", e);
                e
            })
            .ok();
        // vmm is not running, join thread will be hang.
        if self.is_uninitialized() || self.vmm_thread.is_none() {
            debug!(sl!(), "vmm-master thread is uninitialized or has exited.");
            return Ok(());
        }
        debug!(sl!(), "join vmm-master thread exit.");

        // vmm_thread must be exited, otherwise there will be other sync issues.
        // unwrap is safe, if vmm_thread is None, impossible run to here.
        self.vmm_thread.take().unwrap().join().ok();
        info!(sl!(), "vmm-master thread join succeed.");
        Ok(())
    }

    fn send_request(&self, vmm_action: VmmAction) -> Result<VmmResponse> {
        if let Some(ref to_vmm) = self.to_vmm {
            to_vmm
                .send(Box::new(vmm_action.clone()))
                .with_context(|| format!("Failed to send  {:?} via channel ", vmm_action))?;
        } else {
            return Err(anyhow!("to_vmm is None"));
        }

        //notify vmm action
        if let Err(e) = self.to_vmm_fd.write(1) {
            return Err(anyhow!("failed to notify vmm: {}", e));
        }

        if let Some(from_vmm) = self.from_vmm.as_ref() {
            match from_vmm.recv() {
                Err(e) => Err(anyhow!("vmm recv err: {}", e)),
                Ok(vmm_outcome) => Ok(vmm_outcome),
            }
        } else {
            Err(anyhow!("from_vmm is None"))
        }
    }

    fn handle_request(&self, req: Request) -> Result<VmmData> {
        let Request::Sync(vmm_action) = req;
        match self.send_request(vmm_action) {
            Ok(vmm_outcome) => match *vmm_outcome {
                Ok(vmm_data) => Ok(vmm_data),
                Err(vmm_action_error) => Err(anyhow!("vmm action error: {:?}", vmm_action_error)),
            },
            Err(e) => Err(e),
        }
    }

    fn handle_request_with_retry(&self, req: Request) -> Result<VmmData> {
        let Request::Sync(vmm_action) = req;
        for count in 0..REQUEST_RETRY {
            match self.send_request(vmm_action.clone()) {
                Ok(vmm_outcome) => match *vmm_outcome {
                    Ok(vmm_data) => {
                        info!(
                            sl!(),
                            "success to send {:?} after retry {}", &vmm_action, count
                        );
                        return Ok(vmm_data);
                    }
                    Err(vmm_action_error) => {
                        if let VmmActionError::UpcallNotReady = vmm_action_error {
                            std::thread::sleep(std::time::Duration::from_millis(10));
                            continue;
                        } else {
                            return Err(vmm_action_error.into());
                        }
                    }
                },
                Err(err) => {
                    return Err(err);
                }
            }
        }
        return Err(anyhow::anyhow!(
            "After {} attempts, it still doesn't work.",
            REQUEST_RETRY
        ));
    }
}
