// Copyright (c) 2019-2023 Alibaba Cloud
// Copyright (c) 2019-2023 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::{collections::HashMap, sync::Arc};

use anyhow::{anyhow, Context, Result};
use kata_sys_util::rand::RandomBytes;
use tokio::sync::{Mutex, RwLock};

use crate::{
    device::VhostUserBlkDevice, BlockConfig, BlockDevice, Hypervisor, VfioDevice, VhostUserConfig,
    KATA_BLK_DEV_TYPE, KATA_MMIO_BLK_DEV_TYPE, VIRTIO_BLOCK_MMIO, VIRTIO_BLOCK_PCI,
};

use super::{
    util::{get_host_path, get_virt_drive_name, DEVICE_TYPE_BLOCK},
    Device, DeviceConfig, DeviceType,
};

pub type ArcMutexDevice = Arc<Mutex<dyn Device>>;

/// block_index and released_block_index are used to search an available block index
/// in Sandbox.
///
/// @block_driver to be used for block device;
/// @block_index generally default is 1 for <vdb>;
/// @released_block_index for blk devices removed and indexes will released at the same time.
#[derive(Clone, Debug, Default)]
struct SharedInfo {
    block_driver: String,
    block_index: u64,
    released_block_index: Vec<u64>,
}

impl SharedInfo {
    async fn new(hypervisor: Arc<dyn Hypervisor>) -> Self {
        // get hypervisor block driver
        let block_driver = match hypervisor
            .hypervisor_config()
            .await
            .blockdev_info
            .block_device_driver
            .as_str()
        {
            // convert the block driver to kata type
            VIRTIO_BLOCK_MMIO => KATA_MMIO_BLK_DEV_TYPE.to_string(),
            VIRTIO_BLOCK_PCI => KATA_BLK_DEV_TYPE.to_string(),
            _ => "".to_string(),
        };

        SharedInfo {
            block_driver,
            block_index: 1,
            released_block_index: vec![],
        }
    }

    // declare the available block index
    fn declare_device_index(&mut self) -> Result<u64> {
        let current_index = if let Some(index) = self.released_block_index.pop() {
            index
        } else {
            self.block_index
        };
        self.block_index += 1;

        Ok(current_index)
    }

    fn release_device_index(&mut self, index: u64) {
        self.released_block_index.push(index);
        self.released_block_index.sort_by(|a, b| b.cmp(a));
    }
}

// Device manager will manage the lifecycle of sandbox device
pub struct DeviceManager {
    devices: HashMap<String, ArcMutexDevice>,
    hypervisor: Arc<dyn Hypervisor>,
    shared_info: SharedInfo,
}

impl DeviceManager {
    pub async fn new(hypervisor: Arc<dyn Hypervisor>) -> Result<Self> {
        let devices = HashMap::<String, ArcMutexDevice>::new();
        Ok(DeviceManager {
            devices,
            hypervisor: hypervisor.clone(),
            shared_info: SharedInfo::new(hypervisor.clone()).await,
        })
    }

    pub async fn try_add_device(&mut self, device_id: &str) -> Result<()> {
        // find the device
        let device = self
            .devices
            .get(device_id)
            .context("failed to find device")?;
        let mut device_guard = device.lock().await;
        // attach device
        let result = device_guard.attach(self.hypervisor.as_ref()).await;
        // handle attach error
        if let Err(e) = result {
            match device_guard.get_device_info().await {
                DeviceType::Block(device) => {
                    self.shared_info.release_device_index(device.config.index);
                }
                DeviceType::Vfio(device) => {
                    // safe here:
                    // Only when vfio dev_type is `b`, virt_path MUST be Some(X),
                    // and needs do release_device_index. otherwise, let it go.
                    if device.config.dev_type == DEVICE_TYPE_BLOCK {
                        self.shared_info
                            .release_device_index(device.config.virt_path.unwrap().0);
                    }
                }
                DeviceType::VhostUserBlk(device) => {
                    self.shared_info.release_device_index(device.config.index);
                }
                _ => {
                    debug!(sl!(), "no need to do release device index.");
                }
            }

            drop(device_guard);
            self.devices.remove(device_id);

            return Err(e);
        }

        Ok(())
    }

    pub async fn try_remove_device(&mut self, device_id: &str) -> Result<()> {
        if let Some(dev) = self.devices.get(device_id) {
            let mut device_guard = dev.lock().await;
            let result = match device_guard.detach(self.hypervisor.as_ref()).await {
                Ok(index) => {
                    if let Some(i) = index {
                        // release the declared block device index
                        self.shared_info.release_device_index(i);
                    }
                    Ok(())
                }
                Err(e) => Err(e),
            };

            // if detach success, remove it from device manager
            if result.is_ok() {
                drop(device_guard);
                self.devices.remove(device_id);
            }

            return result;
        }

        Err(anyhow!(
            "device with specified ID hasn't been created. {}",
            device_id
        ))
    }

    async fn get_device_info(&self, device_id: &str) -> Result<DeviceType> {
        if let Some(dev) = self.devices.get(device_id) {
            return Ok(dev.lock().await.get_device_info().await);
        }

        Err(anyhow!(
            "device with specified ID hasn't been created. {}",
            device_id
        ))
    }

    async fn find_device(&self, host_path: String) -> Option<String> {
        for (device_id, dev) in &self.devices {
            match dev.lock().await.get_device_info().await {
                DeviceType::Block(device) => {
                    if device.config.path_on_host == host_path {
                        return Some(device_id.to_string());
                    }
                }
                DeviceType::Vfio(device) => {
                    if device.config.host_path == host_path {
                        return Some(device_id.to_string());
                    }
                }
                DeviceType::VhostUserBlk(device) => {
                    if device.config.socket_path == host_path {
                        return Some(device_id.to_string());
                    }
                }
                _ => {
                    // TODO: support find other device type
                    continue;
                }
            }
        }

        None
    }

    fn get_dev_virt_path(&mut self, dev_type: &str) -> Result<Option<(u64, String)>> {
        let virt_path = if dev_type == DEVICE_TYPE_BLOCK {
            // generate virt path
            let current_index = self.shared_info.declare_device_index()?;
            let drive_name = get_virt_drive_name(current_index as i32)?;
            let virt_path_name = format!("/dev/{}", drive_name);

            Some((current_index, virt_path_name))
        } else {
            // only dev_type is block, otherwise, it's None.
            None
        };

        Ok(virt_path)
    }

    async fn new_device(&mut self, device_config: &DeviceConfig) -> Result<String> {
        // device ID must be generated by manager instead of device itself
        // in case of ID collision
        let device_id = self.new_device_id()?;
        let dev: ArcMutexDevice = match device_config {
            DeviceConfig::BlockCfg(config) => {
                // try to find the device, if found and just return id.
                if let Some(device_matched_id) = self.find_device(config.path_on_host.clone()).await
                {
                    return Ok(device_matched_id);
                }

                self.create_block_device(config, device_id.clone())
                    .await
                    .context("failed to create device")?
            }
            DeviceConfig::VfioCfg(config) => {
                let mut vfio_dev_config = config.clone();
                let dev_host_path = vfio_dev_config.host_path.clone();
                if let Some(device_matched_id) = self.find_device(dev_host_path).await {
                    return Ok(device_matched_id);
                }

                let virt_path = self.get_dev_virt_path(vfio_dev_config.dev_type.as_str())?;
                vfio_dev_config.virt_path = virt_path;

                Arc::new(Mutex::new(VfioDevice::new(
                    device_id.clone(),
                    &vfio_dev_config,
                )))
            }
            DeviceConfig::VhostUserBlkCfg(config) => {
                // try to find the device, found and just return id.
                if let Some(dev_id_matched) = self.find_device(config.socket_path.clone()).await {
                    info!(
                        sl!(),
                        "vhost blk device with path:{:?} found. just return device id: {:?}",
                        config.socket_path.clone(),
                        dev_id_matched
                    );

                    return Ok(dev_id_matched);
                }

                self.create_vhost_blk_device(config, device_id.clone())
                    .await
                    .context("failed to create vhost blk device")?
            }
            _ => {
                return Err(anyhow!("invliad device type"));
            }
        };

        // register device to devices
        self.devices.insert(device_id.clone(), dev.clone());

        Ok(device_id)
    }

    async fn create_vhost_blk_device(
        &mut self,
        config: &VhostUserConfig,
        device_id: String,
    ) -> Result<ArcMutexDevice> {
        let mut vhu_blk_config = config.clone();
        vhu_blk_config.driver_option = self.shared_info.block_driver.clone();

        // generate block device index and virt path
        // safe here, Block device always has virt_path.
        if let Some(virt_path) = self.get_dev_virt_path(DEVICE_TYPE_BLOCK)? {
            vhu_blk_config.index = virt_path.0;
            vhu_blk_config.virt_path = virt_path.1;
        }

        Ok(Arc::new(Mutex::new(VhostUserBlkDevice::new(
            device_id,
            vhu_blk_config,
        ))))
    }

    async fn create_block_device(
        &mut self,
        config: &BlockConfig,
        device_id: String,
    ) -> Result<ArcMutexDevice> {
        let mut block_config = config.clone();
        block_config.driver_option = self.shared_info.block_driver.clone();

        // generate virt path
        if let Some(virt_path) = self.get_dev_virt_path(DEVICE_TYPE_BLOCK)? {
            block_config.index = virt_path.0;
            block_config.virt_path = virt_path.1;
        }

        // if the path on host is empty, we need to get device host path from the device major and minor number
        // Otherwise, it might be rawfile based block device, the host path is already passed from the runtime,
        // so we don't need to do anything here.
        if block_config.path_on_host.is_empty() {
            block_config.path_on_host =
                get_host_path(DEVICE_TYPE_BLOCK, config.major, config.minor)
                    .context("failed to get host path")?;
        }

        Ok(Arc::new(Mutex::new(BlockDevice::new(
            device_id,
            block_config,
        ))))
    }

    // device ID must be generated by device manager instead of device itself
    // in case of ID collision
    fn new_device_id(&self) -> Result<String> {
        for _ in 0..5 {
            let rand_bytes = RandomBytes::new(8);
            let id = format!("{:x}", rand_bytes);

            // check collision in devices
            if self.devices.get(&id).is_none() {
                return Ok(id);
            }
        }

        Err(anyhow!("ID are exhausted"))
    }
}

// Many scenarios have similar steps when adding devices. so to reduce duplicated code,
// we should create a common method abstracted and use it in various scenarios.
// do_handle_device:
// (1) new_device with DeviceConfig and return device_id;
// (2) try_add_device with device_id and do really add device;
// (3) return device info of device's info;
pub async fn do_handle_device(
    d: &RwLock<DeviceManager>,
    dev_info: &DeviceConfig,
) -> Result<DeviceType> {
    let device_id = d
        .write()
        .await
        .new_device(dev_info)
        .await
        .context("failed to create deviec")?;

    d.write()
        .await
        .try_add_device(&device_id)
        .await
        .context("failed to add deivce")?;

    let device_info = d
        .read()
        .await
        .get_device_info(&device_id)
        .await
        .context("failed to get device info")?;

    Ok(device_info)
}
