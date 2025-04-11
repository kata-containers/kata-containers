// Copyright (c) 2022 Boston University
//
// SPDX-License-Identifier: Apache-2.0
//

use crate::args::{DirectVolSubcommand, DirectVolumeCommand};

use anyhow::{anyhow, Ok, Result};
use futures::executor;
use kata_types::mount::{
    get_volume_mount_info, join_path, DirectVolumeMountInfo, KATA_DIRECT_VOLUME_ROOT_PATH,
    KATA_MOUNT_INFO_FILE_NAME,
};
use nix;
use reqwest::StatusCode;
use slog::{info, o};
use std::fs;
use url;

use agent::ResizeVolumeRequest;
use shim_interface::shim_mgmt::client::MgmtClient;
use shim_interface::shim_mgmt::{
    DIRECT_VOLUME_PATH_KEY, DIRECT_VOLUME_RESIZE_URL, DIRECT_VOLUME_STATS_URL,
};

use crate::utils::TIMEOUT;

const CONTENT_TYPE_JSON: &str = "application/json";

macro_rules! sl {
    () => {
        slog_scope::logger().new(o!("subsystem" => "volume_ops"))
    };
}

pub fn handle_direct_volume(vol_cmd: DirectVolumeCommand) -> Result<()> {
    if !nix::unistd::Uid::effective().is_root() {
        return Err(anyhow!(
            "super-user privileges are required for the direct-volume subcommand"
        ));
    }
    let command = vol_cmd.directvol_cmd;
    let cmd_result: Option<String> = match command {
        DirectVolSubcommand::Add(args) => add(&args.volume_path, &args.mount_info)?,
        DirectVolSubcommand::Remove(args) => remove(&args.volume_path)?,
        DirectVolSubcommand::Stats(args) => executor::block_on(stats(&args.volume_path))?,
        DirectVolSubcommand::Resize(args) => {
            executor::block_on(resize(&args.volume_path, args.resize_size))?
        }
    };
    if let Some(cmd_result) = cmd_result {
        info!(sl!(), "{:?}", cmd_result);
    }

    Ok(())
}

async fn resize(volume_path: &str, size: u64) -> Result<Option<String>> {
    let sandbox_id = get_sandbox_id_for_volume(volume_path)?;
    let mount_info = get_volume_mount_info(volume_path)?;
    let resize_req = ResizeVolumeRequest {
        size,
        volume_guest_path: mount_info.device,
    };
    let encoded = serde_json::to_string(&resize_req)?;
    let shim_client = MgmtClient::new(&sandbox_id, Some(TIMEOUT))?;

    let url = DIRECT_VOLUME_RESIZE_URL;
    let response = shim_client
        .post(url, &String::from(CONTENT_TYPE_JSON), &encoded)
        .await?;
    let status = response.status();
    if status != StatusCode::OK {
        let body = format!("{:?}", response.into_body());
        return Err(anyhow!(
            "failed to resize volume ({:?}): {:?}",
            status,
            body
        ));
    }

    Ok(None)
}

async fn stats(volume_path: &str) -> Result<Option<String>> {
    let sandbox_id = get_sandbox_id_for_volume(volume_path)?;
    let mount_info = get_volume_mount_info(volume_path)?;

    let req_url = url::form_urlencoded::Serializer::new(String::from(DIRECT_VOLUME_STATS_URL))
        .append_pair(DIRECT_VOLUME_PATH_KEY, &mount_info.device)
        .finish();

    let shim_client = MgmtClient::new(&sandbox_id, Some(TIMEOUT))?;
    let response = shim_client.get(&req_url).await?;
    // turn body into string
    let body = format!("{:?}", response.into_body());

    Ok(Some(body))
}

// add writes the mount info (json string) of a direct volume into a filesystem path known to Kata Containers.
pub fn add(volume_path: &str, mount_info: &str) -> Result<Option<String>> {
    fs::create_dir_all(KATA_DIRECT_VOLUME_ROOT_PATH)?;
    let mount_info_dir_path = join_path(KATA_DIRECT_VOLUME_ROOT_PATH, volume_path)?;

    // create directory if missing
    fs::create_dir_all(&mount_info_dir_path)?;

    // This behavior of deserializing and serializing comes from
    // https://github.com/kata-containers/kata-containers/blob/cd27ad144e1a111cb606015c5c9671431535e644/src/runtime/pkg/direct-volume/utils.go#L57-L79
    // Assuming that this is for the purpose of validating the json schema.
    let unserialized_mount_info: DirectVolumeMountInfo = serde_json::from_str(mount_info)?;

    let mount_info_file_path = mount_info_dir_path.join(KATA_MOUNT_INFO_FILE_NAME);
    let serialized_mount_info = serde_json::to_string(&unserialized_mount_info)?;
    fs::write(mount_info_file_path, serialized_mount_info)?;

    Ok(None)
}

// remove deletes the direct volume path including all the files inside it.
pub fn remove(volume_path: &str) -> Result<Option<String>> {
    let path = join_path(KATA_DIRECT_VOLUME_ROOT_PATH, volume_path)?;
    // removes path and any children it contains.
    fs::remove_dir_all(path)?;

    Ok(None)
}

// get_sandbox_id_for_volume finds the id of the first sandbox found in the dir.
// We expect a direct-assigned volume is associated with only a sandbox at a time.
pub fn get_sandbox_id_for_volume(volume_path: &str) -> Result<String> {
    let dir_path = join_path(KATA_DIRECT_VOLUME_ROOT_PATH, volume_path)?;
    let paths = fs::read_dir(dir_path)?;
    for path in paths {
        let path = path?;
        // compare with MOUNT_INFO_FILE_NAME
        if path.file_name() == KATA_MOUNT_INFO_FILE_NAME {
            continue;
        }

        let file_name = path.file_name();
        // turn file_name into String and return it
        let file_name = file_name.to_str().ok_or_else(|| {
            anyhow!(
                "failed to convert file_name {:?} to string",
                file_name.to_string_lossy()
            )
        })?;

        return Ok(String::from(file_name));
    }

    Err(anyhow!("no sandbox found for {}", volume_path))
}

#[cfg(test)]
mod tests {
    use super::*;
    use kata_types::mount::DirectVolumeMountInfo;
    use serial_test::serial;
    use std::{collections::HashMap, fs, path::PathBuf};
    use tempfile::tempdir;
    use test_utils::skip_if_not_root;

    #[test]
    #[serial]
    fn test_get_sandbox_id_for_volume() {
        // this test has to run as root, so has to manually cleanup afterwards
        skip_if_not_root!();

        // create KATA_DIRECT_VOLUME_ROOT_PATH first as safe_path::scoped_join
        // requires prefix dir to exist
        fs::create_dir_all(KATA_DIRECT_VOLUME_ROOT_PATH)
            .expect("create kata direct volume root path failed");

        let test_sandbox_id = "sandboxid_test_file";
        let test_volume_path = String::from("a/b/c");
        let joined_volume_path =
            join_path(KATA_DIRECT_VOLUME_ROOT_PATH, &test_volume_path).unwrap();

        let test_file_dir = joined_volume_path.join(test_sandbox_id);
        fs::create_dir_all(&joined_volume_path).expect("failed to mkdir -p");
        fs::write(&test_file_dir, "teststring").expect("failed to write");

        // test that get_sandbox_id gets the correct sandboxid it sees
        let got = get_sandbox_id_for_volume(&test_volume_path).unwrap();
        assert!(got.eq(test_sandbox_id));

        // test that get_sandbox_id returns error if no sandboxid found
        fs::remove_file(&test_file_dir).expect("failed to remove");
        get_sandbox_id_for_volume(&test_volume_path).expect_err("error expected");

        // cleanup test directory
        fs::remove_dir_all(&joined_volume_path).expect("failed to cleanup test")
    }

    #[test]
    fn test_path_join() {
        #[derive(Debug)]
        struct TestData<'a> {
            rootfs: &'a str,
            volume_path: &'a str,
            result: Result<PathBuf>,
        }
        // the safe_path::scoped_join requires the prefix path to exist on testing machine
        let root_fs = tempdir().expect("failed to create tmpdir").into_path();
        let root_fs_str = root_fs.to_str().unwrap();

        let relative_secret_path = "../../etc/passwd";
        let b64_relative_secret_path =
            base64::encode_config(relative_secret_path, base64::URL_SAFE);

        // byte array of "abcdddd"
        let b64_abs_path = vec![97, 98, 99, 100, 100, 100, 100];
        // b64urlencoded string of "abcdddd"
        let b64urlencodes_relative_path = "YWJjZGRkZA==";

        let tests = &[
            TestData {
                rootfs: root_fs_str,
                volume_path: "",
                result: Err(anyhow!(std::io::ErrorKind::NotFound)),
            },
            TestData {
                rootfs: root_fs_str,
                volume_path: relative_secret_path,
                result: Ok(root_fs.join(b64_relative_secret_path)),
            },
            TestData {
                rootfs: root_fs_str,
                volume_path: unsafe { std::str::from_utf8_unchecked(&b64_abs_path) },
                result: Ok(root_fs.join(b64urlencodes_relative_path)),
            },
        ];

        for (i, d) in tests.iter().enumerate() {
            let msg = format!("test[{}]: {:?}", i, d);
            let result = join_path(d.rootfs, d.volume_path);
            let msg = format!("{}, result: {:?}", msg, result);
            if result.is_ok() {
                assert!(
                    result.as_ref().unwrap() == d.result.as_ref().unwrap(),
                    "{}",
                    msg
                );
                continue;
            }
            let expected_error = format!("{}", d.result.as_ref().unwrap_err());
            let actual_error = format!("{}", result.unwrap_err());
            assert!(actual_error == expected_error, "{}", msg);
        }
    }

    #[test]
    #[serial]
    fn test_add_remove() {
        skip_if_not_root!();
        // example volume dir is a/b/c, note the behavior of join would take "/a" as absolute path.
        // testing with isn't really viable here since the path is then b64 encoded,
        // so this test had to run as root and call `remove()` to manully cleanup afterwards.

        fs::create_dir_all(KATA_DIRECT_VOLUME_ROOT_PATH)
            .expect("create kata direct volume root path failed");

        let base_dir = tempdir().expect("failed to create tmpdir");
        let dir_name = base_dir.path().join("a/b/c");
        let volume_path = String::from(dir_name.to_str().unwrap());
        let actual: DirectVolumeMountInfo = DirectVolumeMountInfo {
            volume_type: String::from("block"),
            device: String::from("/dev/sda"),
            fs_type: String::from("ext4"),
            metadata: HashMap::new(),
            options: vec![String::from("journal_dev"), String::from("noload")],
        };
        // serialize volumemountinfo into json string
        let mount_info = serde_json::to_string(&actual).unwrap();
        add(&volume_path, &mount_info).expect("add failed");
        let expected_file_path = volume_path;
        let expected: DirectVolumeMountInfo = get_volume_mount_info(&expected_file_path).unwrap();
        remove(&expected_file_path).expect("remove failed");
        assert_eq!(actual.device, expected.device);
        assert_eq!(actual.fs_type, expected.fs_type);
        assert_eq!(actual.metadata, expected.metadata);
        assert_eq!(actual.options, expected.options);
        assert_eq!(actual.volume_type, expected.volume_type);
    }
}
