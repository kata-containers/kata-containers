// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use kata_types::container::ContainerType;
use oci_spec::{runtime as oci, OciSpecError};
use std::path::PathBuf;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    /// unknown container type
    #[error("unknown container type {0}")]
    UnknownContainerType(String),
    /// missing sandboxID
    #[error("missing sandboxID")]
    MissingSandboxID,
    /// oci error
    #[error("oci error")]
    Oci(#[from] OciSpecError),
}

const CRI_CONTAINER_TYPE_KEY_LIST: &[&str] = &[
    // cri containerd
    "io.kubernetes.cri.container-type",
    // cri-o
    "io.kubernetes.cri-o.ContainerType",
    // docker shim
    "io.kubernetes.docker.type",
];

const CRI_SANDBOX_ID_KEY_LIST: &[&str] = &[
    // cri containerd
    "io.kubernetes.cri.sandbox-id",
    // cri-o
    "io.kubernetes.cri-o.SandboxID",
    // docker shim
    "io.kubernetes.sandbox.id",
];

/// container sandbox info
#[derive(Debug, Clone)]
pub enum ShimIdInfo {
    /// Sandbox
    Sandbox,
    /// Container
    Container(String),
}

/// get container type
pub fn get_container_type(spec: &oci::Spec) -> Result<ContainerType, Error> {
    if let Some(annotations) = spec.annotations() {
        for k in CRI_CONTAINER_TYPE_KEY_LIST.iter() {
            if let Some(type_value) = annotations.get(*k) {
                match type_value.as_str() {
                    "sandbox" => return Ok(ContainerType::PodSandbox),
                    "podsandbox" => return Ok(ContainerType::PodSandbox),
                    "container" => return Ok(ContainerType::PodContainer),
                    _ => return Err(Error::UnknownContainerType(type_value.clone())),
                }
            }
        }
    }

    Ok(ContainerType::PodSandbox)
}

/// get shim id info
pub fn get_shim_id_info() -> Result<ShimIdInfo, Error> {
    let spec = load_oci_spec()?;
    match get_container_type(&spec)? {
        ContainerType::PodSandbox | ContainerType::SingleContainer => Ok(ShimIdInfo::Sandbox),
        ContainerType::PodContainer => {
            if let Some(annotations) = spec.annotations() {
                for k in CRI_SANDBOX_ID_KEY_LIST {
                    if let Some(sandbox_id) = annotations.get(*k) {
                        return Ok(ShimIdInfo::Container(sandbox_id.into()));
                    }
                }
            }

            Err(Error::MissingSandboxID)
        }
    }
}

/// get bundle path
pub fn get_bundle_path() -> std::io::Result<PathBuf> {
    std::env::current_dir()
}

/// load oci spec
pub fn load_oci_spec() -> Result<oci::Spec, OciSpecError> {
    let bundle_path = get_bundle_path()?;
    let spec_file = bundle_path.join("config.json");

    oci::Spec::load(spec_file.to_str().unwrap_or_default())
}
