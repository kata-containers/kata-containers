//
// Copyright (c) 2024 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

/// # Overview
/// This module provides ablilities for managing vendor-specific device configuration and service within a system.
/// It handles tasks such as loading device tool configurations from a JSON file, validating the configurations,
/// and setting up device services based on specified parameters.
///
/// # Usage
///
/// * 1. Configure File: During the rootfs creation process, a JSON configuration file needs to be created and
/// placed at `/opt/kata/vendor-devices/configure.toml`. This file should contain configurations for device tools
/// (executables, scripts), specified parameters, CDI storage paths, execution policies, and other options.
///
/// * 2. Load Configuration: Utilize the VendorDevice::new() method to invoke the load function, thereby loading
/// the configuration from the designated file and performing validation.
///
/// * 3. Setup Vendor Devices: Call the `handle_cdi_devices()` method, which in turn calls the `setup_vendor_devices()`
/// method, passing in devices list to execute the device tool loading service.
///
use std::collections::HashMap;
use std::fs;
use std::io::Read;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};

use protocols::agent::Device;

// DEFAULT_CDI_PATH_STATIC is the default path for static CDI Specs.
const DEFAULT_CDI_PATH_STATIC: &str = "/etc/cdi";
// DEFAULT_CDI_PATH_DYNAMIC is the default path for generated CDI Specs
const DEFAULT_CDI_PATH_DYNAMIC: &str = "/var/run/cdi";

// DEFAULT_KATA_VENDOR_DEVICES_CONFIG is default path for storing vendor-ctl config
const DEFAULT_KATA_VENDOR_DEVICES_CONFIG: &str = "/opt/kata/vendor-devices/configure.json";

// Commands is for command and its arguments
type Commands = HashMap<PathBuf, Vec<String>>;

pub struct VendorDevice {
    /// config is for vendor tool configuration
    config: VendorToolConfig,
}

#[derive(Debug, Default, Deserialize, Serialize, Clone)]
struct VendorToolConfig {
    /// vendor_ctl defines a set of commands will be invoked by kata-agent
    /// if vendor_ctl is empty, we regard it as there's no need to execute
    /// vendor tools.
    #[serde(default)]
    vendor_ctl: Commands,

    /// cdi path is directoy path for cdi specs, /etc/cdi, /var/run/cdi.
    #[serde(default)]
    cdi_path: PathBuf,

    /// policy is for how vendor tool to execute
    #[serde(default)]
    policy: String,

    /// options is for vendor tool to execute
    #[serde(default)]
    options: HashMap<String, String>,
}

impl VendorToolConfig {
    /// validate the configuration
    fn validate(&self) -> Result<()> {
        if self.vendor_ctl.is_empty() {
            return Err(anyhow!("vendor tool doesn't exist."));
        }

        let cdi_paths = [
            PathBuf::from(DEFAULT_CDI_PATH_STATIC),
            PathBuf::from(DEFAULT_CDI_PATH_DYNAMIC),
        ];
        if !cdi_paths.contains(&self.cdi_path) {
            fs::create_dir_all(PathBuf::from(DEFAULT_CDI_PATH_DYNAMIC))?;
        } else if !self.cdi_path.exists() {
            fs::create_dir_all(&self.cdi_path)?;
        }

        Ok(())
    }

    /// Loads the configuration from a JSON file
    fn load(path: PathBuf) -> Result<VendorToolConfig> {
        // open config file
        let mut file = std::fs::File::open(path).context("open failed")?;

        // Read the file content from path
        let mut json_string = String::new();
        file.read_to_string(&mut json_string)?;

        // Deserialize the file content into VendorToolConfig
        let config: VendorToolConfig =
            serde_json::from_str(&json_string).context("serde_json from str failed")?;

        // do validate
        config.validate().context("validate failed")?;

        Ok(config)
    }
}

impl VendorDevice {
    fn new(config_path: &str) -> Result<Self> {
        let vendor_path = PathBuf::from(config_path);

        Ok(VendorDevice {
            config: VendorToolConfig::load(vendor_path)?,
        })
    }

    fn setup_vendor_devices(&self, bdfs: &[String]) -> Result<()> {
        // root@kata-containers:~/ vendor_ctl vendor_args bdfs
        for (vendor_ctl, vendor_args) in self.config.vendor_ctl.iter() {
            let _output = Command::new(vendor_ctl.as_os_str())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .args(vendor_args)
                .args(bdfs)
                .output()
                .context(format!(
                    "failed to setup vendor devices {}",
                    vendor_ctl.display()
                ))?;
        }

        Ok(())
    }
}

// Function to extract guest_pcipath from device_options
// options ["Host_BDF1=PCI_Guest_Path1", "Host_BDF2=PCI_Guest_Path2", "Host_BDF3=PCI_Guest_Path3"]
// pci_paths: ["PCI_Guest_Path1", "PCI_Guest_Path2", "PCI_Guest_Path3"]
fn extract_pci_paths(options: &[String]) -> Result<Vec<String>> {
    let mut pci_paths: Vec<String> = Vec::with_capacity(options.len());
    for option in options.iter() {
        let pos = option
            .find('=')
            .ok_or_else(|| anyhow!("malformed vfio PCI path {:?}", &option))?;
        pci_paths.push(option[pos + 1..].to_owned());
    }

    Ok(pci_paths)
}

fn get_vendor_devices(devices: &[Device]) -> Result<Vec<String>> {
    let mut device_bdfs: Vec<String> = Vec::new();
    for dev in devices.iter() {
        let bdfs = extract_pci_paths(&dev.options).context("extract pci paths")?;
        device_bdfs.extend(bdfs);
    }

    Ok(device_bdfs)
}

pub fn setup_vendor_devices(bdfs: &[String]) -> Result<()> {
    let vendor_device =
        VendorDevice::new(DEFAULT_KATA_VENDOR_DEVICES_CONFIG).context("new vendor device")?;
    vendor_device
        .setup_vendor_devices(bdfs)
        .context("setup vendor devices failed")?;

    Ok(())
}

// TODO: handle_cdi_devices invokes setup_vendor_devices
// #[instrument]
// pub async fn handle_cdi_devices(
//     devices: &[Device],
//     spec: &mut Spec,
//     sandbox: &Arc<Mutex<Sandbox>>,
// ) -> Result<()> {
//     let bdfs = get_vendor_devices(devices)?;
//     setup_vendor_devices(&bdfs).context("handle vendor devices")?;

//     Ok(())
// }
