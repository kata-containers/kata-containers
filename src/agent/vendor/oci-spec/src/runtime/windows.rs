use crate::error::OciSpecError;
use derive_builder::Builder;
use getset::{CopyGetters, Getters, Setters};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(
    Builder,
    Clone,
    Debug,
    Default,
    Deserialize,
    Eq,
    CopyGetters,
    Getters,
    Setters,
    PartialEq,
    Serialize,
)]
#[serde(rename_all = "camelCase")]
#[builder(
    default,
    pattern = "owned",
    setter(into, strip_option),
    build_fn(error = "OciSpecError")
)]
/// Windows defines the runtime configuration for Windows based containers,
/// including Hyper-V containers.
pub struct Windows {
    #[getset(get = "pub", set = "pub")]
    /// LayerFolders contains a list of absolute paths to directories
    /// containing image layers.
    layer_folders: Vec<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[getset(get = "pub", set = "pub")]
    /// Devices are the list of devices to be mapped into the container.
    devices: Option<Vec<WindowsDevice>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[getset(get_copy = "pub", set = "pub")]
    /// Resources contains information for handling resource constraints for
    /// the container.
    resources: Option<WindowsResources>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[getset(get = "pub", set = "pub")]
    /// CredentialSpec contains a JSON object describing a group Managed
    /// Service Account (gMSA) specification.
    credential_spec: Option<HashMap<String, Option<serde_json::Value>>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[getset(get_copy = "pub", set = "pub")]
    /// Servicing indicates if the container is being started in a mode to
    /// apply a Windows Update servicing operation.
    servicing: Option<bool>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[getset(get_copy = "pub", set = "pub")]
    /// IgnoreFlushesDuringBoot indicates if the container is being started
    /// in a mode where disk writes are not flushed during its boot
    /// process.
    ignore_flushes_during_boot: Option<bool>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[getset(get = "pub", set = "pub")]
    /// HyperV contains information for running a container with Hyper-V
    /// isolation.
    hyperv: Option<WindowsHyperV>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[getset(get = "pub", set = "pub")]
    /// Network restriction configuration.
    network: Option<WindowsNetwork>,
}

#[derive(
    Builder, Clone, Debug, Default, Deserialize, Eq, Getters, Setters, PartialEq, Serialize,
)]
#[serde(rename_all = "camelCase")]
#[builder(
    default,
    pattern = "owned",
    setter(into, strip_option),
    build_fn(error = "OciSpecError")
)]
#[getset(get = "pub", set = "pub")]
/// WindowsDevice represents information about a host device to be mapped
/// into the container.
pub struct WindowsDevice {
    /// Device identifier: interface class GUID, etc..
    id: String,

    /// Device identifier type: "class", etc..
    id_type: String,
}

#[derive(
    Builder, Clone, Copy, Debug, Default, Deserialize, Eq, Getters, Setters, PartialEq, Serialize,
)]
#[builder(
    default,
    pattern = "owned",
    setter(into, strip_option),
    build_fn(error = "OciSpecError")
)]
#[getset(get_copy = "pub", set = "pub")]
/// Available windows resources.
pub struct WindowsResources {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Memory restriction configuration.
    memory: Option<WindowsMemoryResources>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// CPU resource restriction configuration.
    cpu: Option<WindowsCPUResources>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Storage restriction configuration.
    storage: Option<WindowsStorageResources>,
}

#[derive(
    Builder, Clone, Copy, Debug, Default, Deserialize, Eq, Getters, Setters, PartialEq, Serialize,
)]
#[builder(
    default,
    pattern = "owned",
    setter(into, strip_option),
    build_fn(error = "OciSpecError")
)]
#[getset(get_copy = "pub", set = "pub")]
/// WindowsMemoryResources contains memory resource management settings.
pub struct WindowsMemoryResources {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Memory limit in bytes.
    limit: Option<u64>,
}

#[derive(
    Builder, Clone, Copy, Debug, Default, Deserialize, Eq, Getters, Setters, PartialEq, Serialize,
)]
#[builder(
    default,
    pattern = "owned",
    setter(into, strip_option),
    build_fn(error = "OciSpecError")
)]
#[getset(get_copy = "pub", set = "pub")]
/// WindowsCPUResources contains CPU resource management settings.
pub struct WindowsCPUResources {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Number of CPUs available to the container.
    count: Option<u64>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// CPU shares (relative weight to other containers with cpu shares).
    shares: Option<u16>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Specifies the portion of processor cycles that this container can
    /// use as a percentage times 100.
    maximum: Option<u16>,
}

#[derive(
    Builder, Clone, Copy, Debug, Default, Deserialize, Eq, Getters, Setters, PartialEq, Serialize,
)]
#[serde(rename_all = "camelCase")]
#[builder(
    default,
    pattern = "owned",
    setter(into, strip_option),
    build_fn(error = "OciSpecError")
)]
#[getset(get_copy = "pub", set = "pub")]
/// WindowsStorageResources contains storage resource management settings.
pub struct WindowsStorageResources {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Specifies maximum Iops for the system drive.
    iops: Option<u64>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Specifies maximum bytes per second for the system drive.
    bps: Option<u64>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Sandbox size specifies the minimum size of the system drive in
    /// bytes.
    sandbox_size: Option<u64>,
}

#[derive(
    Builder, Clone, Debug, Default, Deserialize, Eq, Getters, Setters, PartialEq, Serialize,
)]
#[serde(rename_all = "camelCase")]
#[builder(
    default,
    pattern = "owned",
    setter(into, strip_option),
    build_fn(error = "OciSpecError")
)]
#[getset(get = "pub", set = "pub")]
/// WindowsHyperV contains information for configuring a container to run
/// with Hyper-V isolation.
pub struct WindowsHyperV {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// UtilityVMPath is an optional path to the image used for the Utility
    /// VM.
    utility_vm_path: Option<String>,
}

#[derive(
    Builder, Clone, Debug, Default, Deserialize, Eq, Getters, Setters, PartialEq, Serialize,
)]
#[serde(rename_all = "camelCase")]
#[builder(
    default,
    pattern = "owned",
    setter(into, strip_option),
    build_fn(error = "OciSpecError")
)]
/// WindowsNetwork contains network settings for Windows containers.
pub struct WindowsNetwork {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[getset(get = "pub", set = "pub")]
    /// List of HNS endpoints that the container should connect to.
    endpoint_list: Option<Vec<String>>,

    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "allowUnqualifiedDNSQuery"
    )]
    #[getset(get_copy = "pub", set = "pub")]
    /// Specifies if unqualified DNS name resolution is allowed.
    allow_unqualified_dns_query: Option<bool>,

    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "DNSSearchList"
    )]
    #[getset(get = "pub", set = "pub")]
    /// Comma separated list of DNS suffixes to use for name resolution.
    dns_search_list: Option<Vec<String>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[getset(get = "pub", set = "pub")]
    /// Name (ID) of the container that we will share with the network
    /// stack.
    network_shared_container_name: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[getset(get = "pub", set = "pub")]
    /// name (ID) of the network namespace that will be used for the
    /// container.
    network_namespace: Option<String>,
}
