use crate::error::OciSpecError;
use derive_builder::Builder;
use getset::{Getters, Setters};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(
    Builder, Clone, Debug, Default, Deserialize, Getters, Setters, Eq, PartialEq, Serialize,
)]
#[builder(
    default,
    pattern = "owned",
    setter(into, strip_option),
    build_fn(error = "OciSpecError")
)]
#[getset(get = "pub", set = "pub")]
/// VM contains information for virtual-machine-based containers.
pub struct VM {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Hypervisor specifies hypervisor-related configuration for
    /// virtual-machine-based containers.
    hypervisor: Option<VMHypervisor>,

    /// Kernel specifies kernel-related configuration for
    /// virtual-machine-based containers.
    kernel: VMKernel,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Image specifies guest image related configuration for
    /// virtual-machine-based containers.
    image: Option<VMImage>,
}

#[derive(
    Builder, Clone, Debug, Default, Deserialize, Getters, Setters, Eq, PartialEq, Serialize,
)]
#[builder(
    default,
    pattern = "owned",
    setter(into, strip_option),
    build_fn(error = "OciSpecError")
)]
#[getset(get = "pub", set = "pub")]
/// VMHypervisor contains information about the hypervisor to use for a
/// virtual machine.
pub struct VMHypervisor {
    /// Path is the host path to the hypervisor used to manage the virtual
    /// machine.
    path: PathBuf,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Parameters specifies parameters to pass to the hypervisor.
    parameters: Option<Vec<String>>,
}

#[derive(
    Builder, Clone, Debug, Default, Deserialize, Getters, Setters, Eq, PartialEq, Serialize,
)]
#[builder(
    default,
    pattern = "owned",
    setter(into, strip_option),
    build_fn(error = "OciSpecError")
)]
#[getset(get = "pub", set = "pub")]
/// VMKernel contains information about the kernel to use for a virtual
/// machine.
pub struct VMKernel {
    /// Path is the host path to the kernel used to boot the virtual
    /// machine.
    path: PathBuf,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Parameters specifies parameters to pass to the kernel.
    parameters: Option<Vec<String>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// InitRD is the host path to an initial ramdisk to be used by the
    /// kernel.
    initrd: Option<String>,
}

#[derive(
    Builder, Clone, Debug, Default, Deserialize, Getters, Setters, Eq, PartialEq, Serialize,
)]
#[builder(
    default,
    pattern = "owned",
    setter(into, strip_option),
    build_fn(error = "OciSpecError")
)]
#[getset(get = "pub", set = "pub")]
/// VMImage contains information about the virtual machine root image.
pub struct VMImage {
    /// Path is the host path to the root image that the VM kernel would
    /// boot into.
    path: PathBuf,

    /// Format is the root image format type (e.g. "qcow2", "raw", "vhd",
    /// etc).
    format: String,
}
