// Copyright (c) 2021 Alibaba Cloud
// Copyright (c) 2021 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

const CRI_CONTAINER_TYPE_KEY_LIST: &[&str] = &[
    // cri containerd
    "io.kubernetes.cri.container-type",
    // cri-o
    "io.kubernetes.cri-o.ContainerType",
    // docker shim
    "io.kubernetes.docker.type",
];

const CRI_SANDBOX_NAME_KEY_LIST: &[&str] = &[
    // cri containerd
    "io.kubernetes.cri.sandbox-id",
    // cri-o
    "io.kubernetes.cri-o.SandboxID",
    // docker shim
    "io.kubernetes.sandbox.id",
];

#[derive(Debug, thiserror::Error)]
pub enum OciSpecInfoError {
    #[error("cannot infer container type")]
    UnknownContainerType,
    #[error("cannot find sandbox id")]
    MissingSandboxId,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ContainerType {
    PodSandbox,
    PodContainer,
}

#[derive(Debug, Clone)]
pub enum ContainerSandboxInfo {
    Sandbox,
    Container(String),
}

pub fn contaier_type(
    spec: &oci_spec::runtime::Spec,
) -> std::result::Result<ContainerType, OciSpecInfoError> {
    let annotations = match spec.annotations() {
        Some(ann) => ann,
        None => return Ok(ContainerType::PodSandbox),
    };
    for k in CRI_CONTAINER_TYPE_KEY_LIST.iter() {
        if let Some(type_value) = annotations.get(*k) {
            match type_value.as_str() {
                "sandbox" => return Ok(ContainerType::PodSandbox),
                "podsandbox" => return Ok(ContainerType::PodSandbox),
                "container" => return Ok(ContainerType::PodContainer),
                _ => return Err(OciSpecInfoError::UnknownContainerType),
            }
        }
    }

    Ok(ContainerType::PodSandbox)
}

pub fn container_sandbox_info(
    spec: &oci_spec::runtime::Spec,
) -> std::result::Result<ContainerSandboxInfo, OciSpecInfoError> {
    match contaier_type(spec)? {
        ContainerType::PodSandbox => Ok(ContainerSandboxInfo::Sandbox),
        ContainerType::PodContainer => {
            if let Some(annotations) = spec.annotations() {
                for k in CRI_SANDBOX_NAME_KEY_LIST {
                    if let Some(sandbox_id) = annotations.get(*k) {
                        return Ok(ContainerSandboxInfo::Container(sandbox_id.into()));
                    }
                }
            }
            Err(OciSpecInfoError::MissingSandboxId)
        }
    }
}
