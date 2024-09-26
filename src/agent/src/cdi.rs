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

#[cfg(test)]
mod tests {
    use super::*;

    use protocols::agent::Device;
    use std::fs;
    use std::fs::File;
    use std::io::Write;
    use std::os::unix::fs::PermissionsExt;
    use tempfile::{self, tempdir};

    const VDCTL_CONFIG: &str = r#"
    {
        "vendor_ctl": {
            "/path/to/vendor_ctl": ["arg1", "arg2"]
        },
        "cdi_path": "/etc/cdi",
        "policy": "default",
        "options": {
            "key": "value"
        }
    }
    "#;

    fn create_fake_vendor_tool(vdctl_dir: PathBuf) -> PathBuf {
        let script_content = "#!/bin/bash\necho \"Hello, $1!\"";
        // config_path/vendor-tool
        let vdctl_path = vdctl_dir.join("vendor-tool");

        let mut file = File::create(&vdctl_path).unwrap();
        file.write_all(script_content.as_bytes()).unwrap();

        println!("Shell script 'vendor-tool' has been created.");

        #[cfg(unix)]
        {
            let mut perms = file.metadata().unwrap().permissions();
            perms.set_mode(0o755); // This sets the file permissions to rwxr-xr-x
            std::fs::set_permissions(&vdctl_path, perms).unwrap();
            println!("execution permissions have been set.");
        }

        vdctl_path
    }

    fn setup_config_path() -> PathBuf {
        let tempdir = tempdir().unwrap();
        fs::create_dir_all(tempdir.path()).unwrap();
        let path = tempdir.path().join("configure.json");
        let mut file = fs::File::create(&path).unwrap();
        file.write_all(VDCTL_CONFIG.as_bytes()).unwrap();

        path
    }

    #[test]
    fn test_load_vendor_tool_config_success() {
        let tempdir = tempdir().unwrap();
        fs::create_dir_all(tempdir.path()).unwrap();
        let config_path = tempdir.path().join("configure.json");
        let mut file = fs::File::create(&config_path).unwrap();
        file.write_all(VDCTL_CONFIG.as_bytes()).unwrap();
        println!("load_vendor_tool_config: {:?}", &config_path);

        let config_result = VendorToolConfig::load(config_path);
        assert!(config_result.is_ok());
        let config = config_result.unwrap();

        assert_eq!(config.vendor_ctl.len(), 1);
        assert_eq!(config.cdi_path, PathBuf::from("/etc/cdi"));
        assert_eq!(config.policy, "default");
    }

    #[test]
    fn test_load_vendor_tool_config_missing_file() {
        let result = VendorToolConfig::load(PathBuf::from("/path/to/non-extisting-path"));
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_vendor_tool_config_no_vendor_ctl() {
        let config = VendorToolConfig {
            vendor_ctl: HashMap::new(),
            cdi_path: PathBuf::from(DEFAULT_CDI_PATH_STATIC),
            policy: String::new(),
            options: HashMap::new(),
        };

        let result = config.validate();
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "vendor tool doesn't exist."
        );
    }

    #[test]
    fn test_extract_pci_paths_success() {
        let options = vec![
            "Host_BDF1=PCI_Guest_Path1".to_string(),
            "Host_BDF2=PCI_Guest_Path2".to_string(),
        ];

        let pci_paths_result = extract_pci_paths(&options);
        assert!(pci_paths_result.is_ok());
        let pci_paths = pci_paths_result.unwrap();
        assert_eq!(
            pci_paths,
            vec!["PCI_Guest_Path1".to_string(), "PCI_Guest_Path2".to_string()]
        );
    }

    #[test]
    fn test_extract_pci_paths_malformed() {
        let options = vec!["malformed_option".to_string()];

        let result = extract_pci_paths(&options);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "malformed vfio PCI path \"malformed_option\""
        );
    }

    #[test]
    fn test_extract_pci_paths_empty_options() {
        let options = vec![];

        let result = extract_pci_paths(&options);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 0);
    }

    #[test]
    fn test_get_vendor_devices() {
        let devices = vec![Device {
            options: vec!["Host_BDF1=PCI_Guest_Path1".to_string()],
            ..Default::default()
        }];

        let devices_bdfs = get_vendor_devices(&devices).unwrap();

        assert_eq!(devices_bdfs, vec!["PCI_Guest_Path1".to_string()]);

        let devices2 = vec![Device {
            options: vec![],
            ..Default::default()
        }];
        let devices_bdfs2 = get_vendor_devices(&devices2).unwrap();

        assert!(devices_bdfs2.is_empty());
    }

    #[test]
    fn test_get_vendor_devices_without_cdi_devices() {
        let devices: Vec<Device> = vec![
            Device {
                id: "dev001".to_string(),
                options: vec![],
                ..Default::default()
            },
            Device {
                id: "dev002".to_string(),
                options: vec![],
                ..Default::default()
            },
            Device {
                id: "dev003".to_string(),
                options: vec![],
                ..Default::default()
            },
        ];

        let devices_bdfs = get_vendor_devices(&devices).unwrap();
        assert!(devices_bdfs.is_empty());
    }

    #[test]
    fn test_setup_vendor_devices_success() {
        let tempdir = tempdir().unwrap();
        fs::create_dir_all(tempdir.path()).unwrap();

        // create config path with config file
        let config_path = tempdir.path().join("configure.json");
        let mut file = fs::File::create(&config_path).unwrap();
        file.write_all(VDCTL_CONFIG.as_bytes()).unwrap();
        println!("setup_vendor_devices: {:?}", &config_path);

        // create vendor tool path with vendor tool
        let root_path = tempdir.path().join("vdctlpath");
        fs::create_dir_all(&root_path).unwrap();
        println!("vendor tool path: {:?}", &root_path);
        let vdctl_path = create_fake_vendor_tool(root_path);

        // create devices with cdi devices options with elements
        let devices = vec![Device {
            options: vec!["Host_BDF1=PCI_Guest_Path1".to_string()],
            ..Default::default()
        }];

        // get vendor cdi devices
        let devices_bdfs = get_vendor_devices(&devices).unwrap();

        // new VendorDevice
        let vd_res = VendorDevice::new(config_path.display().to_string().as_str());
        assert!(vd_res.is_ok());

        // fill the real vendor tool path with args("")
        let mut vendor_device = vd_res.unwrap();
        vendor_device.config.vendor_ctl = HashMap::from([(vdctl_path, vec!["".to_string()])]);

        // setup device just print hello
        let result2 = vendor_device.setup_vendor_devices(devices_bdfs.as_slice());
        assert!(result2.is_ok());
    }
}
