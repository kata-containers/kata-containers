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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::process::Command;

    #[test]
    fn test_decode_verity_options() {
        let verity_option = DmVerityInfo {
            hashtype: "sha256".to_string(),
            blocksize: 512,
            hashsize: 512,
            blocknum: 16384,
            offset: 8388608,
            hash: "9de18652fe74edfb9b805aaed72ae2aa48f94333f1ba5c452ac33b1c39325174".to_string(),
        };
        let json_option = serde_json::to_string(&verity_option).unwrap();
        let encoded = base64::encode(&json_option);

        let decoded = DmVerityInfo::try_from(&encoded).unwrap_or_else(|err| panic!("{}", err));
        assert_eq!(decoded.hashtype, verity_option.hashtype);
        assert_eq!(decoded.blocksize, verity_option.blocksize);
        assert_eq!(decoded.hashsize, verity_option.hashsize);
        assert_eq!(decoded.blocknum, verity_option.blocknum);
        assert_eq!(decoded.offset, verity_option.offset);
        assert_eq!(decoded.hash, verity_option.hash);

        let decoded = DmVerityInfo::try_from(&json_option).unwrap();
        assert_eq!(decoded.hashtype, verity_option.hashtype);
        assert_eq!(decoded.blocksize, verity_option.blocksize);
        assert_eq!(decoded.hashsize, verity_option.hashsize);
        assert_eq!(decoded.blocknum, verity_option.blocknum);
        assert_eq!(decoded.offset, verity_option.offset);
        assert_eq!(decoded.hash, verity_option.hash);
    }

    #[test]
    fn test_check_verity_options() {
        let tests = &[
            DmVerityInfo {
                hashtype: "md5".to_string(), // "md5" is not a supported hash algorithm
                blocksize: 512,
                hashsize: 512,
                blocknum: 16384,
                offset: 8388608,
                hash: "9de18652fe74edfb9b805aaed72ae2aa48f94333f1ba5c452ac33b1c39325174"
                    .to_string(),
            },
            DmVerityInfo {
                hashtype: "sha256".to_string(),
                blocksize: 3000, // Invalid block size, not a power of 2.
                hashsize: 512,
                blocknum: 16384,
                offset: 8388608,
                hash: "9de18652fe74edfb9b805aaed72ae2aa48f94333f1ba5c452ac33b1c39325174"
                    .to_string(),
            },
            DmVerityInfo {
                hashtype: "sha256".to_string(),
                blocksize: 0, // Invalid block size, less than 512.
                hashsize: 512,
                blocknum: 16384,
                offset: 8388608,
                hash: "9de18652fe74edfb9b805aaed72ae2aa48f94333f1ba5c452ac33b1c39325174"
                    .to_string(),
            },
            DmVerityInfo {
                hashtype: "sha256".to_string(),
                blocksize: 524800, // Invalid block size, greater than 524288.
                hashsize: 512,
                blocknum: 16384,
                offset: 8388608,
                hash: "9de18652fe74edfb9b805aaed72ae2aa48f94333f1ba5c452ac33b1c39325174"
                    .to_string(),
            },
            DmVerityInfo {
                hashtype: "sha256".to_string(),
                blocksize: 512,
                hashsize: 3000, // Invalid hash block size, not a power of 2.
                blocknum: 16384,
                offset: 8388608,
                hash: "9de18652fe74edfb9b805aaed72ae2aa48f94333f1ba5c452ac33b1c39325174"
                    .to_string(),
            },
            DmVerityInfo {
                hashtype: "sha256".to_string(),
                blocksize: 512,
                hashsize: 0, // Invalid hash block size, less than 512.
                blocknum: 16384,
                offset: 8388608,
                hash: "9de18652fe74edfb9b805aaed72ae2aa48f94333f1ba5c452ac33b1c39325174"
                    .to_string(),
            },
            DmVerityInfo {
                hashtype: "sha256".to_string(),
                blocksize: 512,
                hashsize: 524800, // Invalid hash block size, greater than 524288.
                blocknum: 16384,
                offset: 8388608,
                hash: "9de18652fe74edfb9b805aaed72ae2aa48f94333f1ba5c452ac33b1c39325174"
                    .to_string(),
            },
            DmVerityInfo {
                hashtype: "sha256".to_string(),
                blocksize: 512,
                hashsize: 512,
                blocknum: 0, // Invalid blocknum, it must be greater than 0.
                offset: 8388608,
                hash: "9de18652fe74edfb9b805aaed72ae2aa48f94333f1ba5c452ac33b1c39325174"
                    .to_string(),
            },
            DmVerityInfo {
                hashtype: "sha256".to_string(),
                blocksize: 512,
                hashsize: 512,
                blocknum: 16384,
                offset: 0, // Invalid offset, it must be greater than 0.
                hash: "9de18652fe74edfb9b805aaed72ae2aa48f94333f1ba5c452ac33b1c39325174"
                    .to_string(),
            },
            DmVerityInfo {
                hashtype: "sha256".to_string(),
                blocksize: 512,
                hashsize: 512,
                blocknum: 16384,
                offset: 8193, // Invalid offset, it must be aligned to 512.
                hash: "9de18652fe74edfb9b805aaed72ae2aa48f94333f1ba5c452ac33b1c39325174"
                    .to_string(),
            },
            DmVerityInfo {
                hashtype: "sha256".to_string(),
                blocksize: 512,
                hashsize: 512,
                blocknum: 16384,
                offset: 8388608 - 4096, // Invalid offset, it must be equal to blocksize * blocknum.
                hash: "9de18652fe74edfb9b805aaed72ae2aa48f94333f1ba5c452ac33b1c39325174"
                    .to_string(),
            },
        ];
        for d in tests.iter() {
            d.validate().unwrap_err();
        }
        let test_data = DmVerityInfo {
            hashtype: "sha256".to_string(),
            blocksize: 512,
            hashsize: 512,
            blocknum: 16384,
            offset: 8388608,
            hash: "9de18652fe74edfb9b805aaed72ae2aa48f94333f1ba5c452ac33b1c39325174".to_string(),
        };
        test_data.validate().unwrap();
    }

    #[test]
    fn test_create_verity_device() {
        let work_dir = tempfile::tempdir().unwrap();
        let file_name: std::path::PathBuf = work_dir.path().join("test.file");
        let data = vec![0u8; 1048576];
        fs::write(&file_name, data)
            .unwrap_or_else(|err| panic!("Failed to write to file: {}", err));

        let loop_control = loopdev::LoopControl::open().unwrap_or_else(|err| panic!("{}", err));
        let loop_device = loop_control
            .next_free()
            .unwrap_or_else(|err| panic!("{}", err));
        loop_device
            .with()
            .autoclear(true)
            .attach(file_name.to_str().unwrap())
            .unwrap_or_else(|err| panic!("{}", err));
        let loop_device_path = loop_device
            .path()
            .unwrap_or_else(|| panic!("failed to get loop device path"));

        let tests = &[
            DmVerityInfo {
                hashtype: "sha256".to_string(),
                blocksize: 512,
                hashsize: 4096,
                blocknum: 1024,
                offset: 524288,
                hash: "fc65e84aa2eb12941aeaa29b000bcf1d9d4a91190bd9b10b5f51de54892952c6"
                    .to_string(),
            },
            DmVerityInfo {
                hashtype: "sha1".to_string(),
                blocksize: 512,
                hashsize: 1024,
                blocknum: 1024,
                offset: 524288,
                hash: "e889164102360c7b0f56cbef6880c4ae75f552cf".to_string(),
            },
        ];
        for d in tests.iter() {
            let verity_device_path =
                create_verity_device(d, &loop_device_path).unwrap_or_else(|err| panic!("{}", err));
            assert_eq!(verity_device_path, format!("/dev/mapper/{}", d.hash));
            destroy_verity_device(&d.hash).unwrap();
        }
    }

    #[test]
    fn test_mount_and_umount_image_block_with_integrity() {
        const VERITYSETUP_PATH: &[&str] = &["/sbin/veritysetup", "/usr/sbin/veritysetup"];
        //create a disk image file
        let work_dir = tempfile::tempdir().unwrap();
        let mount_dir = tempfile::tempdir().unwrap();
        let file_name: std::path::PathBuf = work_dir.path().join("test.file");
        let default_hash_type = "sha256";
        let default_data_block_size: u64 = 512;
        let default_data_block_num: u64 = 1024;
        let data_device_size = default_data_block_size * default_data_block_num;
        let default_hash_size: u64 = 4096;
        let default_resize_size: u64 = data_device_size * 4;
        let data = vec![0u8; data_device_size as usize];
        fs::write(&file_name, data)
            .unwrap_or_else(|err| panic!("Failed to write to file: {}", err));
        Command::new("mkfs")
            .args(["-t", "ext4", file_name.to_str().unwrap()])
            .output()
            .map_err(|err| format!("Failed to format disk image: {}", err))
            .unwrap_or_else(|err| panic!("{}", err));

        Command::new("truncate")
            .args([
                "-s",
                default_resize_size.to_string().as_str(),
                file_name.to_str().unwrap(),
            ])
            .output()
            .map_err(|err| format!("Failed to resize disk image: {}", err))
            .unwrap_or_else(|err| panic!("{}", err));

        //find an unused loop device and attach the file to the device
        let loop_control = loopdev::LoopControl::open().unwrap_or_else(|err| panic!("{}", err));
        let loop_device = loop_control
            .next_free()
            .unwrap_or_else(|err| panic!("{}", err));
        loop_device
            .with()
            .autoclear(true)
            .attach(file_name.to_str().unwrap())
            .unwrap_or_else(|err| panic!("{}", err));
        let loop_device_path = loop_device
            .path()
            .unwrap_or_else(|| panic!("failed to get loop device path"));
        let loop_device_path_str = loop_device_path
            .to_str()
            .unwrap_or_else(|| panic!("failed to get path string"));

        let mut verity_option = DmVerityInfo {
            hashtype: default_hash_type.to_string(),
            blocksize: default_data_block_size,
            hashsize: default_hash_size,
            blocknum: default_data_block_num,
            offset: data_device_size,
            hash: "".to_string(),
        };

        // Calculates and permanently stores hash verification data for data_device.
        let veritysetup_bin = VERITYSETUP_PATH
            .iter()
            .find(|&path| Path::new(path).exists())
            .copied()
            .unwrap_or_else(|| panic!("Veritysetup path not found"));
        let output = Command::new(veritysetup_bin)
            .args([
                "format",
                "--no-superblock",
                "--format=1",
                "-s",
                "",
                &format!("--hash={}", verity_option.hashtype),
                &format!("--data-block-size={}", verity_option.blocksize),
                &format!("--hash-block-size={}", verity_option.hashsize),
                "--data-blocks",
                &format!("{}", verity_option.blocknum),
                "--hash-offset",
                &format!("{}", verity_option.offset),
                loop_device_path_str,
                loop_device_path_str,
            ])
            .output()
            .unwrap_or_else(|err| panic!("{}", err));
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let lines: Vec<&str> = stdout.lines().collect();
            let hash_strings: Vec<&str> = lines[lines.len() - 1].split_whitespace().collect();
            verity_option.hash = hash_strings[2].to_string()
        } else {
            let error_message = String::from_utf8_lossy(&output.stderr);
            panic!("Failed to create hash device: {}", error_message);
        }

        let verity_device_path = create_verity_device(&verity_option, &loop_device_path)
            .unwrap_or_else(|_|panic!("failed to create verity device"));
        let mount_dir_path= mount_dir.path();
        assert!(nix::mount::mount(
            Some(verity_device_path.as_str()),
            mount_dir_path,
            Some("ext4"),
            nix::mount::MsFlags::MS_RDONLY,
            None::<&str>,
        ).is_ok());
        let verity_device_name = verity_option.hash;
        assert!(nix::mount::umount(mount_dir_path).is_ok());
        assert!(destroy_verity_device(&verity_device_name).is_ok());
    }
}
