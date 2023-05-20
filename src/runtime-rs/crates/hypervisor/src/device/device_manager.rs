// Copyright (c) 2019-2023 Alibaba Cloud
// Copyright (c) 2019-2023 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::{collections::HashMap, sync::Arc};

use anyhow::{anyhow, Context, Result};
use kata_sys_util::rand::RandomBytes;
use tokio::sync::Mutex;

use crate::{
    BlockConfig, BlockDevice, Hypervisor, KATA_BLK_DEV_TYPE, KATA_MMIO_BLK_DEV_TYPE,
    VIRTIO_BLOCK_MMIO, VIRTIO_BLOCK_PCI,
};

use super::{
    util::{get_host_path, get_virt_drive_name},
    Device, DeviceConfig,
};
pub type ArcMutexDevice = Arc<Mutex<dyn Device>>;

/// block_index and released_block_index are used to search an available block index
/// in Sandbox.
///
/// @block_index generally default is 1 for <vdb>;
/// @released_block_index for blk devices removed and indexes will released at the same time.
#[derive(Clone, Debug, Default)]
struct SharedInfo {
    block_index: u64,
    released_block_index: Vec<u64>,
}

impl SharedInfo {
    fn new() -> Self {
        SharedInfo {
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
    pub fn new(hypervisor: Arc<dyn Hypervisor>) -> Result<Self> {
        let devices = HashMap::<String, ArcMutexDevice>::new();
        Ok(DeviceManager {
            devices,
            hypervisor,
            shared_info: SharedInfo::new(),
        })
    }

    pub async fn new_device(&mut self, device_config: &DeviceConfig) -> Result<String> {
        let device_id = if let Some(dev) = self.find_device(device_config).await {
            dev
        } else {
            self.create_device(device_config)
                .await
                .context("failed to create device")?
        };
        Ok(device_id)
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
            if let DeviceConfig::BlockCfg(config) = device_guard.get_device_info().await {
                self.shared_info.release_device_index(config.index);
            };
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
            if result.is_ok() {
                drop(device_guard);
                // if detach success, remove it from device manager
                self.devices.remove(device_id);
            }
            return result;
        }
        Err(anyhow!(
            "device with specified ID hasn't been created. {}",
            device_id
        ))
    }

    pub async fn get_device_info(&self, device_id: &str) -> Result<DeviceConfig> {
        if let Some(dev) = self.devices.get(device_id) {
            return Ok(dev.lock().await.get_device_info().await);
        }
        Err(anyhow!(
            "device with specified ID hasn't been created. {}",
            device_id
        ))
    }

    async fn find_device(&self, device_config: &DeviceConfig) -> Option<String> {
        for (device_id, dev) in &self.devices {
            match dev.lock().await.get_device_info().await {
                DeviceConfig::BlockCfg(config) => match device_config {
                    DeviceConfig::BlockCfg(ref config_new) => {
                        if config_new.path_on_host == config.path_on_host {
                            return Some(device_id.to_string());
                        }
                    }
                    _ => {
                        continue;
                    }
                },
                _ => {
                    // TODO: support find other device type
                    continue;
                }
            }
        }
        None
    }

    async fn create_device(&mut self, device_config: &DeviceConfig) -> Result<String> {
        // device ID must be generated by manager instead of device itself
        // in case of ID collision
        let device_id = self.new_device_id()?;
        let dev: ArcMutexDevice = match device_config {
            DeviceConfig::BlockCfg(config) => self
                .create_block_device(config, device_id.clone())
                .await
                .context("failed to create device")?,
            _ => {
                return Err(anyhow!("invliad device type"));
            }
        };
        // register device to devices
        self.devices.insert(device_id.clone(), dev.clone());
        Ok(device_id)
    }

    async fn create_block_device(
        &mut self,
        config: &BlockConfig,
        device_id: String,
    ) -> Result<ArcMutexDevice> {
        let mut block_config = config.clone();
        // get hypervisor block driver
        let block_driver = match self
            .hypervisor
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
        block_config.driver_option = block_driver;
        // generate virt path
        let current_index = self.shared_info.declare_device_index()?;
        block_config.index = current_index;
        let drive_name = get_virt_drive_name(current_index as i32)?;
        block_config.virt_path = format!("/dev/{}", drive_name);
        // if the path on host is empty, we need to get device host path from the device major and minor number
        // Otherwise, it might be rawfile based block device, the host path is already passed from the runtime, so we don't need to do anything here
        if block_config.path_on_host.is_empty() {
            block_config.path_on_host = get_host_path("b".to_owned(), config.major, config.minor)
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
