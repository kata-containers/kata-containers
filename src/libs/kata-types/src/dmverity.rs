// Copyright (c) 2023 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

use anyhow::{bail, Context, Error, Result};
use base64;
use devicemapper::{DevId, DmFlags, DmName, DmOptions, DM};
use serde_json;
use std::convert::TryFrom;
use std::path::Path;

/// Configuration information for DmVerity device.
#[derive(Debug, Clone, Eq, PartialEq, Default, Serialize, Deserialize)]
pub struct DmVerityInfo {
    /// Hash algorithm for dm-verity.
    pub hashtype: String,
    /// Root hash for device verification or activation.
    pub hash: String,
    /// Size of data device used in verification.
    pub blocknum: u64,
    /// Used block size for the data device.
    pub blocksize: u64,
    /// Used block size for the hash device.
    pub hashsize: u64,
    /// Offset of hash area/superblock on hash_device.
    pub offset: u64,
}

/// Configuration information for DmVerity device.

impl DmVerityInfo {
    /// Validate configuration information for DmVerity device.
    pub fn validate(&self) -> Result<()> {
        match self.hashtype.to_lowercase().as_str() {
            "sha1" => {
                if self.hash.len() != 40 || hex::decode(&self.hash).is_err() {
                    bail!(
                        "Invalid hash value sha1:{} for DmVerity device with sha1",
                        self.hash,
                    );
                }
            }
            "sha224" => {
                if self.hash.len() != 56 || hex::decode(&self.hash).is_err() {
                    bail!(
                        "Invalid hash value sha224:{} for DmVerity device with sha1",
                        self.hash,
                    );
                }
            }
            "sha256" => {
                if self.hash.len() != 64 || hex::decode(&self.hash).is_err() {
                    bail!(
                        "Invalid hash value sha256:{} for DmVerity device with sha256",
                        self.hash,
                    );
                }
            }
            "sha384" => {
                if self.hash.len() != 96 || hex::decode(&self.hash).is_err() {
                    bail!(
                        "Invalid hash value sha384:{} for DmVerity device with sha1",
                        self.hash,
                    );
                }
            }
            "sha512" => {
                if self.hash.len() != 128 || hex::decode(&self.hash).is_err() {
                    bail!(
                        "Invalid hash value sha512:{} for DmVerity device with sha1",
                        self.hash,
                    );
                }
            }
            _ => {
                bail!(
                    "Unsupported hash algorithm {} for DmVerity device {}",
                    self.hashtype,
                    self.hash,
                );
            }
        }

        if self.blocknum == 0 || self.blocknum > u32::MAX as u64 {
            bail!("Zero block count for DmVerity device {}", self.hash);
        }
        if !Self::is_valid_block_size(self.blocksize) || !Self::is_valid_block_size(self.hashsize) {
            bail!(
                "Unsupported verity block size: data_block_size = {},hash_block_size = {}",
                self.blocksize,
                self.hashsize
            );
        }
        if self.offset % self.hashsize != 0 || self.offset < self.blocksize * self.blocknum {
            bail!(
                "Invalid hashvalue offset {} for DmVerity device {}",
                self.offset,
                self.hash
            );
        }

        Ok(())
    }

    // Checks if the block size is a power of two between 2^9 and 2^19.
    // The minimal block size: For disk sector size is always 512 bytes (https://docs.rs/devicemapper/latest/devicemapper/constant.SECTOR_SIZE.html),
    // and the size of DmDevice is counted in sectors, (https://docs.rs/devicemapper/latest/devicemapper/trait.DmDevice.html)
    // so the block size should be equal or more than 512 bytes and must be divisible by 512.
    //
    // The max block size: The max size of the buffer in the devicemapper crate used to dm-verity is u32::MAX.
    // (https://docs.rs/crate/devicemapper/0.33.5/source/src/core/dm.rs#:~:text=//%20If%20DM_BUFFER_FULL,exceed%20u32%3A%3AMAX.)
    // And the buffer includes the struct of `dm_ioctl`, which the size is 312 (0x138), equal to 39 bytes.
    // So the max size of the data should be less than u32:MAX - 312
    // And the data length would be check whether it's a power of two.
    // (https://docs.rs/crate/devicemapper/0.33.5/source/src/core/dm.rs#:~:text=buffer.resize((len%20as%20u32).saturating_mul(2)%20as%20usize%2C%200)%3B)
    // So the block size should be less than 2^31 bytes.
    fn is_valid_block_size(block_size: u64) -> bool {
        for order in 9..32 {
            if block_size == 1 << order {
                return true;
            }
        }
        false
    }
}

// Parse `DmVerityInfo` object from plaintext or base64 encoded json string.
impl TryFrom<&str> for DmVerityInfo {
    type Error = Error;

    fn try_from(value: &str) -> std::result::Result<Self, Self::Error> {
        let option = if let Ok(v) = serde_json::from_str::<DmVerityInfo>(value) {
            v
        } else {
            let decoded = base64::decode(value)?;
            serde_json::from_slice::<DmVerityInfo>(&decoded)?
        };

        option.validate()?;
        Ok(option)
    }
}

impl TryFrom<&String> for DmVerityInfo {
    type Error = Error;

    fn try_from(value: &String) -> std::result::Result<Self, Self::Error> {
        Self::try_from(value.as_str())
    }
}

/// Creates a mapping with <name> backed by data_device <source_device_path>
/// and using hash_device for in-kernel verification.
/// It will return the verity block device Path "/dev/mapper/<name>"
/// Notes: the data device and the hash device are the same one.
pub fn create_verity_device(
    verity_option: &DmVerityInfo,
    source_device_path: &Path,
) -> Result<String> {
    //Make sure the fields in DmVerityInfo are validated.
    verity_option.validate()?;

    let dm = DM::new()?;
    let verity_name = DmName::new(&verity_option.hash)?;
    let id = DevId::Name(verity_name);
    let opts = DmOptions::default().set_flags(DmFlags::DM_READONLY);
    let hash_start_block: u64 =
        (verity_option.offset + verity_option.hashsize - 1) / verity_option.hashsize;

    // verity parameters: <version> <data_device> <hash_device> <data_blk_size> <hash_blk_size>
    // <blocks> <hash_start> <algorithm> <root_hash> <salt>
    // version: on-disk hash version
    //     0 is the original format used in the Chromium OS.
    //     1 is the current format that should be used for new devices.
    // data_device: device containing the data the integrity of which needs to be checked.
    //     It may be specified as a path, like /dev/vdX, or a device number, major:minor.
    // hash_device: device that that supplies the hash tree data.
    //     It is specified similarly to the data device path and is the same device in the function of create_verity_device.
    //     The hash_start should be outside of the dm-verity configured device size.
    // data_blk_size: The block size in bytes on a data device.
    // hash_blk_size: The size of a hash block in bytes.
    // blocks: The number of data blocks on the data device.
    // hash_start: offset, in hash_blk_size blocks, from the start of hash_device to the root block of the hash tree.
    // algorithm: The cryptographic hash algorithm used for this device. This should be the name of the algorithm, like "sha256".
    // root_hash: The hexadecimal encoding of the cryptographic hash of the root hash block and the salt.
    // salt: The hexadecimal encoding of the salt value.
    let verity_params = format!(
        "1 {} {} {} {} {} {} {} {} {}",
        source_device_path.display(),
        source_device_path.display(),
        verity_option.blocksize,
        verity_option.hashsize,
        verity_option.blocknum,
        hash_start_block,
        verity_option.hashtype,
        verity_option.hash,
        "-",
    );
    // Mapping table in device mapper: <start_sector> <size> <target_name> <target_params>:
    // <start_sector> is 0
    // <size> is size of device in sectors, and one sector is equal to 512 bytes.
    // <target_name> is name of mapping target, here "verity" for dm-verity
    // <target_params> are parameters for verity target
    let verity_table = vec![(
        0,
        verity_option.blocknum * verity_option.blocksize / 512,
        "verity".into(),
        verity_params,
    )];

    dm.device_create(verity_name, None, opts)?;
    dm.table_load(&id, verity_table.as_slice(), opts)?;
    dm.device_suspend(&id, opts)?;

    Ok(format!("/dev/mapper/{}", &verity_option.hash))
}

/// Destroy a DmVerity device with specified name.
pub fn destroy_verity_device(verity_device_name: &str) -> Result<()> {
    let dm = devicemapper::DM::new()?;
    let name = devicemapper::DmName::new(verity_device_name)?;

    dm.device_remove(
        &devicemapper::DevId::Name(name),
        devicemapper::DmOptions::default(),
    )
    .context(format!("remove DmverityDevice {}", verity_device_name))?;

    Ok(())
}

/// Get the DmVerity device name from option string.
pub fn get_verity_device_name(verity_options: &str) -> Result<String> {
    let option = DmVerityInfo::try_from(verity_options)?;
    Ok(option.hash)
}
