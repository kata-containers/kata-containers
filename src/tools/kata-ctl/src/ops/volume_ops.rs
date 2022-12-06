// Copyright (c) 2022 Boston University
//
// SPDX-License-Identifier: Apache-2.0
//

use crate::args::{DirectVolSubcommand, DirectVolumeCommand};

use anyhow::{anyhow, Ok, Result};
use futures::executor;
use kata_types::mount::{
    DirectVolumeMountInfo, KATA_DIRECT_VOLUME_ROOT_PATH, KATA_MOUNT_INFO_FILE_NAME,
};
use nix;
use reqwest::StatusCode;
use safe_path;
use std::{fs, path::PathBuf, time::Duration};
use url;

use agent::ResizeVolumeRequest;
use shim_interface::shim_mgmt::client::MgmtClient;
use shim_interface::shim_mgmt::{
    DIRECT_VOLUME_PATH_KEY, DIRECT_VOLUME_RESIZE_URL, DIRECT_VOLUME_STATS_URL,
};

const TIMEOUT: Duration = Duration::from_millis(2000);
const CONTENT_TYPE_JSON: &str = "application/json";

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
        println!("{:?}", cmd_result);
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

// join_path joins user provided volumepath with kata direct-volume root path
// the volume_path is base64-encoded and then safely joined to the end of path prefix
fn join_path(prefix: &str, volume_path: &str) -> Result<PathBuf> {
    if volume_path.is_empty() {
        return Err(anyhow!("volume path must not be empty"));
    }
    let b64_encoded_path = base64::encode(volume_path.as_bytes());

    Ok(safe_path::scoped_join(prefix, b64_encoded_path)?)
}

// add writes the mount info (json string) of a direct volume into a filesystem path known to Kata Containers.
pub fn add(volume_path: &str, mount_info: &str) -> Result<Option<String>> {
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

pub fn get_volume_mount_info(volume_path: &str) -> Result<DirectVolumeMountInfo> {
    let mount_info_file_path =
        join_path(KATA_DIRECT_VOLUME_ROOT_PATH, volume_path)?.join(KATA_MOUNT_INFO_FILE_NAME);
    let mount_info_file = fs::read_to_string(mount_info_file_path)?;
    let mount_info: DirectVolumeMountInfo = serde_json::from_str(&mount_info_file)?;

    Ok(mount_info)
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

    return Err(anyhow!("no sandbox found for {}", volume_path));
}
