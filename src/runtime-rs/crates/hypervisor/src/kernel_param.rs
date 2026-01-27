// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use anyhow::{anyhow, Result};

use crate::{
    VM_ROOTFS_DRIVER_BLK, VM_ROOTFS_DRIVER_BLK_CCW, VM_ROOTFS_DRIVER_MMIO, VM_ROOTFS_DRIVER_PMEM,
    VM_ROOTFS_ROOT_BLK, VM_ROOTFS_ROOT_PMEM,
};
use kata_types::config::LOG_VPORT_OPTION;
use kata_types::fs::{
    VM_ROOTFS_FILESYSTEM_EROFS, VM_ROOTFS_FILESYSTEM_EXT4, VM_ROOTFS_FILESYSTEM_XFS,
};
use std::collections::HashMap;

// Port where the agent will send the logs. Logs are sent through the vsock in cases
// where the hypervisor has no console.sock, i.e dragonball
const VSOCK_LOGS_PORT: &str = "1025";

const KERNEL_KV_DELIMITER: &str = "=";
const KERNEL_PARAM_DELIMITER: char = ' ';
const VERITY_BLOCK_SIZE_BYTES: u64 = 512;

// Split kernel params on spaces, but keep quoted substrings intact.
// Example: dm-mod.create="dm-verity,,,ro,0 736328 verity 1 /dev/vda1 /dev/vda2 ...".
fn split_kernel_params(params_string: &str) -> Vec<String> {
    let mut params = Vec::new();
    let mut current = String::new();
    let mut in_quote = false;

    for c in params_string.chars() {
        if c == '"' {
            in_quote = !in_quote;
            current.push(c);
            continue;
        }

        if c == KERNEL_PARAM_DELIMITER && !in_quote {
            let trimmed = current.trim();
            if !trimmed.is_empty() {
                params.push(trimmed.to_string());
            }
            current.clear();
            continue;
        }

        current.push(c);
    }

    let trimmed = current.trim();
    if !trimmed.is_empty() {
        params.push(trimmed.to_string());
    }

    params
}

struct KernelVerityConfig {
    mode: String,
    root_hash: String,
    salt: String,
    data_blocks: u64,
    data_block_size: u64,
    hash_block_size: u64,
}

fn parse_kernel_verity_params(params_string: &str) -> Result<Option<KernelVerityConfig>> {
    if params_string.trim().is_empty() {
        return Ok(None);
    }

    let mut values = HashMap::new();
    for field in params_string.split(',') {
        let field = field.trim();
        if field.is_empty() {
            continue;
        }
        let mut parts = field.splitn(2, '=');
        let key = parts.next().unwrap_or("");
        let value = parts.next().ok_or_else(|| anyhow!("Invalid kernel_verity_params entry: {field}"))?;
        values.insert(key.to_string(), value.to_string());
    }

    let mode = values
        .get("mode")
        .ok_or_else(|| anyhow!("Missing kernel_verity_params mode"))?
        .to_string();

    if mode != VERITY_MODE_KERNELINIT && mode != VERITY_MODE_INITRAMFS {
        return Err(anyhow!("Invalid kernel_verity_params mode: {mode}"));
    }

    let root_hash = values
        .get("root_hash")
        .ok_or_else(|| anyhow!("Missing kernel_verity_params root_hash"))?
        .to_string();

    let salt = values.get("salt").cloned().unwrap_or_default();
    let (data_blocks, data_block_size, hash_block_size) = if mode == VERITY_MODE_KERNELINIT {
        let parse_uint_field = |name: &str| -> Result<u64> {
            match values.get(name) {
                Some(value) if !value.is_empty() => value.parse::<u64>().map_err(|e| {
                    anyhow!("Invalid kernel_verity_params {} '{}': {}", name, value, e)
                }),
                _ => Err(anyhow!("Missing kernel_verity_params {}", name)),
            }
        };

        (
            parse_uint_field("data_blocks")?,
            parse_uint_field("data_block_size")?,
            parse_uint_field("hash_block_size")?,
        )
    } else {
        (0, 0, 0)
    };

    Ok(Some(KernelVerityConfig {
        mode,
        root_hash,
        salt,
        data_blocks,
        data_block_size,
        hash_block_size,
    }))
}

fn kernel_verity_root_flags(rootfs_type: &str) -> Result<String> {
    let normalized = if rootfs_type.is_empty() {
        VM_ROOTFS_FILESYSTEM_EXT4
    } else {
        rootfs_type
    };

    match normalized {
        VM_ROOTFS_FILESYSTEM_EXT4 => Ok("data=ordered,errors=remount-ro ro".to_string()),
        VM_ROOTFS_FILESYSTEM_XFS | VM_ROOTFS_FILESYSTEM_EROFS => Ok("ro".to_string()),
        _ => Err(anyhow!("Unsupported rootfs type {}", rootfs_type)),
    }
}

const VERITY_MODE_KERNELINIT: &str = "kernelinit";
const VERITY_MODE_INITRAMFS: &str = "initramfs";

#[derive(Debug, Clone, PartialEq)]
pub struct Param {
    pub key: String,
    pub value: String,
}

impl Param {
    pub fn new(key: &str, value: &str) -> Self {
        Param {
            key: key.to_owned(),
            value: value.to_owned(),
        }
    }

    pub fn to_string(&self) -> Result<String> {
        if self.key.is_empty() && self.value.is_empty() {
            Err(anyhow!("Empty key and value"))
        } else if self.key.is_empty() {
            Err(anyhow!("Empty key"))
        } else if self.value.is_empty() {
            Ok(self.key.to_string())
        } else {
            Ok(format!("{}{}{}", self.key, KERNEL_KV_DELIMITER, self.value))
        }
    }
}

#[derive(Debug, PartialEq)]
pub(crate) struct KernelParams {
    params: Vec<Param>,
}

impl KernelParams {
    pub(crate) fn new(debug: bool) -> Self {
        // default kernel params
        let mut params = vec![
            Param::new("reboot", "k"),
            Param::new("panic", "1"),
            Param::new("systemd.unit", "kata-containers.target"),
            Param::new("systemd.mask", "systemd-networkd.service"),
        ];

        if debug {
            params.push(Param::new(LOG_VPORT_OPTION, VSOCK_LOGS_PORT));
        }

        Self { params }
    }

    pub(crate) fn new_rootfs_kernel_params(
        kernel_verity_params: &str,
        rootfs_driver: &str,
        rootfs_type: &str,
    ) -> Result<Self> {
        let mut params = vec![];

        match rootfs_driver {
            VM_ROOTFS_DRIVER_PMEM => {
                params.push(Param::new("root", VM_ROOTFS_ROOT_PMEM));
                match rootfs_type {
                    VM_ROOTFS_FILESYSTEM_EXT4 => {
                        params.push(Param::new(
                            "rootflags",
                            "dax,data=ordered,errors=remount-ro ro",
                        ));
                    }
                    VM_ROOTFS_FILESYSTEM_XFS => {
                        params.push(Param::new("rootflags", "dax ro"));
                    }
                    VM_ROOTFS_FILESYSTEM_EROFS => {
                        params.push(Param::new("rootflags", "dax ro"));
                    }
                    _ => {
                        return Err(anyhow!("Unsupported rootfs type {}", rootfs_type));
                    }
                }
            }
            VM_ROOTFS_DRIVER_BLK | VM_ROOTFS_DRIVER_BLK_CCW | VM_ROOTFS_DRIVER_MMIO => {
                params.push(Param::new("root", VM_ROOTFS_ROOT_BLK));
                match rootfs_type {
                    VM_ROOTFS_FILESYSTEM_EXT4 => {
                        params.push(Param::new("rootflags", "data=ordered,errors=remount-ro ro"));
                    }
                    VM_ROOTFS_FILESYSTEM_XFS => {
                        params.push(Param::new("rootflags", "ro"));
                    }
                    VM_ROOTFS_FILESYSTEM_EROFS => {
                        params.push(Param::new("rootflags", "ro"));
                    }
                    _ => {
                        return Err(anyhow!("Unsupported rootfs type {}", rootfs_type));
                    }
                }
            }
            _ => {
                return Err(anyhow!("Unsupported rootfs driver {}", rootfs_driver));
            }
        }

        params.push(Param::new("rootfstype", rootfs_type));

        let mut params = Self { params };
        let cfg = match parse_kernel_verity_params(kernel_verity_params)? {
            Some(cfg) => cfg,
            None => return Ok(params),
        };

        if cfg.mode == VERITY_MODE_INITRAMFS {
            params.append(&mut KernelParams::from_string(&format!(
                "rootfs_verity.scheme=dm-verity rootfs_verity.hash={}",
                cfg.root_hash
            )));
            return Ok(params);
        }

        if cfg.salt.is_empty() {
            return Err(anyhow!("Missing kernel_verity_params salt"));
        }
        if cfg.data_blocks == 0 || cfg.data_block_size == 0 || cfg.hash_block_size == 0 {
            return Err(anyhow!(
                "Invalid kernel_verity_params data_blocks/data_block_size/hash_block_size: must be non-zero"
            ));
        }
        if cfg.data_block_size % VERITY_BLOCK_SIZE_BYTES != 0 {
            return Err(anyhow!(
                "Invalid kernel_verity_params data_block_size: must be multiple of {}",
                VERITY_BLOCK_SIZE_BYTES
            ));
        }
        if cfg.hash_block_size % VERITY_BLOCK_SIZE_BYTES != 0 {
            return Err(anyhow!(
                "Invalid kernel_verity_params hash_block_size: must be multiple of {}",
                VERITY_BLOCK_SIZE_BYTES
            ));
        }

        let (root_device, hash_device) = match rootfs_driver {
            VM_ROOTFS_DRIVER_PMEM => ("/dev/pmem0p1", "/dev/pmem0p2"),
            VM_ROOTFS_DRIVER_BLK | VM_ROOTFS_DRIVER_BLK_CCW | VM_ROOTFS_DRIVER_MMIO => {
                ("/dev/vda1", "/dev/vda2")
            }
            _ => return Err(anyhow!("Unsupported rootfs driver {}", rootfs_driver)),
        };

        let data_sectors = (cfg.data_block_size / VERITY_BLOCK_SIZE_BYTES) * cfg.data_blocks;
        let root_flags = kernel_verity_root_flags(rootfs_type)?;

        let dm_cmd = format!(
            "dm-verity,,,ro,0 {} verity 1 {} {} {} {} {} 0 sha256 {} {}",
            data_sectors,
            root_device,
            hash_device,
            cfg.data_block_size,
            cfg.hash_block_size,
            cfg.data_blocks,
            cfg.root_hash,
            cfg.salt
        );

        params.remove_all_by_key("root".to_string());
        params.remove_all_by_key("rootflags".to_string());
        params.remove_all_by_key("rootfstype".to_string());

        params.push(Param {
            key: "dm-mod.create".to_string(),
            value: format!("\"{}\"", dm_cmd),
        });
        params.push(Param::new("root", "/dev/dm-0"));
        params.push(Param::new("rootflags", &root_flags));
        if rootfs_type.is_empty() {
            params.push(Param::new("rootfstype", VM_ROOTFS_FILESYSTEM_EXT4));
        } else {
            params.push(Param::new("rootfstype", rootfs_type));
        }

        Ok(params)
    }

    pub(crate) fn append(&mut self, params: &mut KernelParams) {
        self.params.append(&mut params.params);
    }

    pub(crate) fn push(&mut self, new_param: Param) {
        self.params.push(new_param);
    }

    pub(crate) fn remove_all_by_key(&mut self, key: String) {
        // Remove all params with the given key from the vector
        self.params.retain(|param| param.key != key);
    }

    pub(crate) fn from_string(params_string: &str) -> Self {
        let mut params = vec![];

        let parameters_vec = split_kernel_params(params_string);

        for param in parameters_vec.iter() {
            if param.is_empty() {
                continue;
            }

            let ps: Vec<&str> = param.splitn::<_>(2, KERNEL_KV_DELIMITER).collect();

            if ps.len() == 2 {
                params.push(Param {
                    key: String::from(ps[0]),
                    value: String::from(ps[1]),
                });
            } else {
                params.push(Param {
                    key: String::from(ps[0]),
                    value: String::from(""),
                });
            }
        }

        Self { params }
    }

    pub(crate) fn to_string(&self) -> Result<String> {
        let mut parameters: Vec<String> = Vec::new();

        for param in &self.params {
            parameters.push(param.to_string()?);
        }

        Ok(parameters.join(&KERNEL_PARAM_DELIMITER.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use anyhow::Result;

    use super::*;

    use crate::{
        VM_ROOTFS_DRIVER_BLK, VM_ROOTFS_DRIVER_PMEM, VM_ROOTFS_ROOT_BLK, VM_ROOTFS_ROOT_PMEM,
    };
    use kata_types::fs::{
        VM_ROOTFS_FILESYSTEM_EROFS, VM_ROOTFS_FILESYSTEM_EXT4, VM_ROOTFS_FILESYSTEM_XFS,
    };

    #[test]
    fn test_params() {
        let param1 = Param::new("", "");
        let param2 = Param::new("", "foo");
        let param3 = Param::new("foo", "");

        assert!(param1.to_string().is_err());
        assert!(param2.to_string().is_err());
        assert_eq!(param3.to_string().unwrap(), String::from("foo"));

        let param4 = Param::new("foo", "bar");
        assert_eq!(param4.to_string().unwrap(), String::from("foo=bar"));
    }

    #[test]
    fn test_kernel_params() -> Result<()> {
        let expect_params_string = "k1=v1 k2=v2 k3=v3".to_string();
        let expect_params = KernelParams {
            params: vec![
                Param::new("k1", "v1"),
                Param::new("k2", "v2"),
                Param::new("k3", "v3"),
            ],
        };

        // check kernel params from string
        let kernel_params = KernelParams::from_string(&expect_params_string);
        assert_eq!(kernel_params, expect_params);

        // check kernel params to string
        let kernel_params_string = expect_params.to_string()?;
        assert_eq!(kernel_params_string, expect_params_string);

        Ok(())
    }

    #[derive(Debug)]
    struct TestData<'a> {
        rootfs_driver: &'a str,
        rootfs_type: &'a str,
        expect_params: KernelParams,
        result: Result<()>,
    }

    #[test]
    fn test_rootfs_kernel_params() {
        let tests = &[
            // EXT4
            TestData {
                rootfs_driver: VM_ROOTFS_DRIVER_PMEM,
                rootfs_type: VM_ROOTFS_FILESYSTEM_EXT4,
                expect_params: KernelParams {
                    params: [
                        Param::new("root", VM_ROOTFS_ROOT_PMEM),
                        Param::new("rootflags", "dax,data=ordered,errors=remount-ro ro"),
                        Param::new("rootfstype", VM_ROOTFS_FILESYSTEM_EXT4),
                    ]
                    .to_vec(),
                },
                result: Ok(()),
            },
            TestData {
                rootfs_driver: VM_ROOTFS_DRIVER_BLK,
                rootfs_type: VM_ROOTFS_FILESYSTEM_EXT4,
                expect_params: KernelParams {
                    params: [
                        Param::new("root", VM_ROOTFS_ROOT_BLK),
                        Param::new("rootflags", "data=ordered,errors=remount-ro ro"),
                        Param::new("rootfstype", VM_ROOTFS_FILESYSTEM_EXT4),
                    ]
                    .to_vec(),
                },
                result: Ok(()),
            },
            // XFS
            TestData {
                rootfs_driver: VM_ROOTFS_DRIVER_PMEM,
                rootfs_type: VM_ROOTFS_FILESYSTEM_XFS,
                expect_params: KernelParams {
                    params: [
                        Param::new("root", VM_ROOTFS_ROOT_PMEM),
                        Param::new("rootflags", "dax ro"),
                        Param::new("rootfstype", VM_ROOTFS_FILESYSTEM_XFS),
                    ]
                    .to_vec(),
                },
                result: Ok(()),
            },
            TestData {
                rootfs_driver: VM_ROOTFS_DRIVER_BLK,
                rootfs_type: VM_ROOTFS_FILESYSTEM_XFS,
                expect_params: KernelParams {
                    params: [
                        Param::new("root", VM_ROOTFS_ROOT_BLK),
                        Param::new("rootflags", "ro"),
                        Param::new("rootfstype", VM_ROOTFS_FILESYSTEM_XFS),
                    ]
                    .to_vec(),
                },
                result: Ok(()),
            },
            // EROFS
            TestData {
                rootfs_driver: VM_ROOTFS_DRIVER_PMEM,
                rootfs_type: VM_ROOTFS_FILESYSTEM_EROFS,
                expect_params: KernelParams {
                    params: [
                        Param::new("root", VM_ROOTFS_ROOT_PMEM),
                        Param::new("rootflags", "dax ro"),
                        Param::new("rootfstype", VM_ROOTFS_FILESYSTEM_EROFS),
                    ]
                    .to_vec(),
                },
                result: Ok(()),
            },
            TestData {
                rootfs_driver: VM_ROOTFS_DRIVER_BLK,
                rootfs_type: VM_ROOTFS_FILESYSTEM_EROFS,
                expect_params: KernelParams {
                    params: [
                        Param::new("root", VM_ROOTFS_ROOT_BLK),
                        Param::new("rootflags", "ro"),
                        Param::new("rootfstype", VM_ROOTFS_FILESYSTEM_EROFS),
                    ]
                    .to_vec(),
                },
                result: Ok(()),
            },
            // Unsupported rootfs driver
            TestData {
                rootfs_driver: "foo",
                rootfs_type: VM_ROOTFS_FILESYSTEM_EXT4,
                expect_params: KernelParams {
                    params: [
                        Param::new("root", VM_ROOTFS_ROOT_BLK),
                        Param::new("rootflags", "data=ordered,errors=remount-ro ro"),
                        Param::new("rootfstype", VM_ROOTFS_FILESYSTEM_EXT4),
                    ]
                    .to_vec(),
                },
                result: Err(anyhow!("Unsupported rootfs driver foo")),
            },
            // Unsupported rootfs type
            TestData {
                rootfs_driver: VM_ROOTFS_DRIVER_BLK,
                rootfs_type: "foo",
                expect_params: KernelParams {
                    params: [
                        Param::new("root", VM_ROOTFS_ROOT_BLK),
                        Param::new("rootflags", "data=ordered,errors=remount-ro ro"),
                        Param::new("rootfstype", VM_ROOTFS_FILESYSTEM_EXT4),
                    ]
                    .to_vec(),
                },
                result: Err(anyhow!("Unsupported rootfs type foo")),
            },
        ];

        for (i, t) in tests.iter().enumerate() {
            let msg = format!("test[{i}]: {t:?}");
            let result =
                KernelParams::new_rootfs_kernel_params("", t.rootfs_driver, t.rootfs_type);
            let msg = format!("{msg}, result: {result:?}");
            if t.result.is_ok() {
                assert!(result.is_ok(), "{}", msg);
                assert_eq!(t.expect_params, result.unwrap());
            } else {
                let expected_error = format!("{}", t.result.as_ref().unwrap_err());
                let actual_error = format!("{}", result.unwrap_err());
                assert!(actual_error == expected_error, "{}", msg);
            }
        }
    }

    #[test]
    fn test_kernel_verity_params() -> Result<()> {
        let params = KernelParams::new_rootfs_kernel_params(
            "mode=initramfs,root_hash=abc",
            VM_ROOTFS_DRIVER_PMEM,
            VM_ROOTFS_FILESYSTEM_EXT4,
        )?;
        assert!(params
            .to_string()?
            .contains("rootfs_verity.scheme=dm-verity"));
        assert!(params.to_string()?.contains("rootfs_verity.hash=abc"));
        assert!(params.to_string()?.contains("root="));

        let params = KernelParams::new_rootfs_kernel_params(
            "mode=kernelinit,root_hash=abc,salt=def,data_blocks=1,data_block_size=4096,hash_block_size=4096",
            VM_ROOTFS_DRIVER_BLK,
            VM_ROOTFS_FILESYSTEM_EXT4,
        )?;
        let params_string = params.to_string()?;
        assert!(params_string.contains("dm-mod.create="));
        assert!(params_string.contains("root=/dev/dm-0"));
        assert!(params_string.contains("rootfstype=ext4"));

        let err = KernelParams::new_rootfs_kernel_params(
            "mode=kernelinit,root_hash=abc,data_blocks=1,data_block_size=4096,hash_block_size=4096",
            VM_ROOTFS_DRIVER_BLK,
            VM_ROOTFS_FILESYSTEM_EXT4,
        )
        .err()
        .expect("expected missing salt error");
        assert!(format!("{err}").contains("Missing kernel_verity_params salt"));

        Ok(())
    }
}
