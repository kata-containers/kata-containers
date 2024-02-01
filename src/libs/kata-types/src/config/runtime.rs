// Copyright (c) 2021 Alibaba Cloud
//
// SPDX-License-Identifier: Apache-2.0
//

use std::io::Result;
use std::path::Path;

use super::default;
use crate::config::{ConfigOps, TomlConfig};
use crate::mount::split_bind_mounts;
use crate::{eother, validate_path};

#[path = "shared_mount.rs"]
pub mod shared_mount;
pub use shared_mount::SharedMount;

/// Type of runtime VirtContainer.
pub const RUNTIME_NAME_VIRTCONTAINER: &str = "virt_container";

/// Kata runtime configuration information.
#[derive(Debug, Default, Deserialize, Serialize)]
pub struct Runtime {
    /// Runtime name: Plan to support virt-container, linux-container, wasm-container
    #[serde(default)]
    pub name: String,

    /// Hypervisor name: Plan to support dragonball, qemu
    #[serde(default)]
    pub hypervisor_name: String,

    /// Agent name
    #[serde(default)]
    pub agent_name: String,

    /// If enabled, the runtime will log additional debug messages to the system log.
    #[serde(default, rename = "enable_debug")]
    pub debug: bool,

    /// The log level will be applied to runtime.
    /// Possible values are:
    /// - trace
    /// - debug
    /// - info
    /// - warn
    /// - error
    /// - critical
    #[serde(default = "default_runtime_log_level")]
    pub log_level: String,

    /// Enabled experimental feature list, format: ["a", "b"].
    ///
    /// Experimental features are features not stable enough for production, they may break
    /// compatibility, and are prepared for a big version bump.
    #[serde(default)]
    pub experimental: Vec<String>,

    /// Determines how the VM should be connected to the container network interface.
    ///
    /// Options:
    /// - macvtap: used when the Container network interface can be bridged using macvtap.
    /// - none: used when customize network. Only creates a tap device. No veth pair.
    /// - tcfilter: uses tc filter rules to redirect traffic from the network interface provided
    ///   by plugin to a tap interface connected to the VM.
    #[serde(default)]
    pub internetworking_model: String,

    /// If enabled, the runtime won't create a network namespace for shim and hypervisor processes.
    ///
    /// This option may have some potential impacts to your host. It should only be used when you
    /// know what you're doing.
    ///
    /// `disable_new_netns` conflicts with `internetworking_model=tcfilter` and
    /// `internetworking_model=macvtap`. It works only with `internetworking_model=none`.
    /// The tap device will be in the host network namespace and can connect to a bridge (like OVS)
    /// directly.
    ///
    /// If you are using docker, `disable_new_netns` only works with `docker run --net=none`
    #[serde(default)]
    pub disable_new_netns: bool,

    /// If specified, sandbox_bind_mounts identifies host paths to be mounted into the sandboxes
    /// shared path.
    ///
    /// This is only valid if filesystem sharing is utilized. The provided path(s) will be bind
    /// mounted into the shared fs directory. If defaults are utilized, these mounts should be
    /// available in the guest at `/run/kata-containers/shared/containers/passthrough/sandbox-mounts`.
    /// These will not be exposed to the container workloads, and are only provided for potential
    /// guest services.
    #[serde(default)]
    pub sandbox_bind_mounts: Vec<String>,

    /// If enabled, the runtime will add all the kata processes inside one dedicated cgroup.
    ///
    /// The container cgroups in the host are not created, just one single cgroup per sandbox.
    /// The runtime caller is free to restrict or collect cgroup stats of the overall Kata sandbox.
    /// The sandbox cgroup path is the parent cgroup of a container with the PodSandbox annotation.
    /// The sandbox cgroup is constrained if there is no container type annotation.
    /// See: https://pkg.go.dev/github.com/kata-containers/kata-containers/src/runtime/virtcontainers#ContainerType
    #[serde(default)]
    pub sandbox_cgroup_only: bool,

    /// If enabled, the runtime will create opentracing.io traces and spans.
    /// See https://www.jaegertracing.io/docs/getting-started.
    #[serde(default)]
    pub enable_tracing: bool,
    /// The full url to the Jaeger HTTP Thrift collector.
    #[serde(default)]
    pub jaeger_endpoint: String,
    /// The username to be used if basic auth is required for Jaeger.
    #[serde(default)]
    pub jaeger_user: String,
    /// The password to be used if basic auth is required for Jaeger.
    #[serde(default)]
    pub jaeger_password: String,

    /// If enabled, user can run pprof tools with shim v2 process through kata-monitor.
    #[serde(default)]
    pub enable_pprof: bool,

    /// If enabled, static resource management will calculate the vcpu and memory for the sandbox/container
    /// And pod configured this will not be able to further update its CPU/Memory resource
    #[serde(default)]
    pub static_sandbox_resource_mgmt: bool,

    /// Determines whether container seccomp profiles are passed to the virtual machine and
    /// applied by the kata agent. If set to true, seccomp is not applied within the guest.
    #[serde(default)]
    pub disable_guest_seccomp: bool,

    /// Determines how VFIO devices should be be presented to the container.
    ///
    /// Options:
    /// - vfio: Matches behaviour of OCI runtimes (e.g. runc) as much as possible.  VFIO devices
    ///   will appear in the container as VFIO character devices under /dev/vfio. The exact names
    ///   may differ from the host (they need to match the VM's IOMMU group numbers rather than
    ///   the host's)
    /// - guest-kernel: This is a Kata-specific behaviour that's useful in certain cases.
    ///   The VFIO device is managed by whatever driver in the VM kernel claims it. This means
    ///   it will appear as one or more device nodes or network interfaces depending on the nature
    ///   of the device. Using this mode requires specially built workloads that know how to locate
    ///   the relevant device interfaces within the VM.
    #[serde(default)]
    pub vfio_mode: String,

    /// Vendor customized runtime configuration.
    #[serde(default, flatten)]
    pub vendor: RuntimeVendor,

    /// If keep_abnormal is enabled, it means that 1) if the runtime exits abnormally, the cleanup process
    /// will be skipped, and 2) the runtime will not exit even if the health check fails.
    /// This option is typically used to retain abnormal information for debugging.
    #[serde(default)]
    pub keep_abnormal: bool,

    /// Base directory of directly attachable network config, the default value
    /// is "/run/kata-containers/dans".
    ///
    /// Network devices for VM-based containers are allowed to be placed in the
    /// host netns to eliminate as many hops as possible, which is what we
    /// called a "directly attachable network". The config, set by special CNI
    /// plugins, is used to tell the Kata Containers what devices are attached
    /// to the hypervisor.
    #[serde(default)]
    pub dan_conf: String,

    /// shared_mount declarations
    #[serde(default)]
    pub shared_mounts: Vec<SharedMount>,

    /// If enabled, the runtime will attempt to use fd passthrough feature for process io.
    #[serde(default)]
    pub use_passfd_io: bool,

    /// If fd passthrough io is enabled, the runtime will attempt to use the specified port instead of the default port.
    #[serde(default = "default_passfd_listener_port")]
    pub passfd_listener_port: u32,
}

fn default_passfd_listener_port() -> u32 {
    default::DEFAULT_PASSFD_LISTENER_PORT
}

impl ConfigOps for Runtime {
    fn adjust_config(conf: &mut TomlConfig) -> Result<()> {
        RuntimeVendor::adjust_config(conf)?;
        if conf.runtime.internetworking_model.is_empty() {
            conf.runtime.internetworking_model = default::DEFAULT_INTERNETWORKING_MODEL.to_owned();
        }

        for bind in conf.runtime.sandbox_bind_mounts.iter_mut() {
            // Split the bind mount, canonicalize the path and then append rw mode to it.
            let (real_path, mode) = split_bind_mounts(bind);
            match Path::new(real_path).canonicalize() {
                Err(e) => return Err(eother!("sandbox bind mount `{}` is invalid: {}", bind, e)),
                Ok(path) => {
                    *bind = format!("{}{}", path.display(), mode);
                }
            }
        }

        Ok(())
    }

    fn validate(conf: &TomlConfig) -> Result<()> {
        RuntimeVendor::validate(conf)?;

        let net_model = &conf.runtime.internetworking_model;
        if !net_model.is_empty()
            && net_model != "macvtap"
            && net_model != "none"
            && net_model != "tcfilter"
        {
            return Err(eother!(
                "Invalid internetworking_model `{}` in configuration file",
                net_model
            ));
        }

        let vfio_mode = &conf.runtime.vfio_mode;
        if !vfio_mode.is_empty() && vfio_mode != "vfio" && vfio_mode != "guest-kernel" {
            return Err(eother!(
                "Invalid vfio_mode `{}` in configuration file",
                vfio_mode
            ));
        }

        for shared_mount in &conf.runtime.shared_mounts {
            shared_mount.validate()?;
        }

        for bind in conf.runtime.sandbox_bind_mounts.iter() {
            // Just validate the real_path.
            let (real_path, _mode) = split_bind_mounts(bind);
            validate_path!(
                real_path.to_owned(),
                "sandbox bind mount `{}` is invalid: {}"
            )?;
        }

        Ok(())
    }
}

impl Runtime {
    /// Check whether experiment `feature` is enabled or not.
    pub fn is_experiment_enabled(&self, feature: &str) -> bool {
        self.experimental.contains(&feature.to_string())
    }
}

fn default_runtime_log_level() -> String {
    String::from("info")
}

#[cfg(not(feature = "enable-vendor"))]
mod vendor {
    use super::*;

    /// Vendor customization runtime configuration.
    #[derive(Debug, Default, Deserialize, Serialize)]
    pub struct RuntimeVendor {}

    impl ConfigOps for RuntimeVendor {}
}

#[cfg(feature = "enable-vendor")]
#[path = "runtime_vendor.rs"]
mod vendor;

pub use vendor::RuntimeVendor;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_invalid_config() {
        let content = r#"
[runtime]
enable_debug = 10
"#;
        TomlConfig::load(content).unwrap_err();

        let content = r#"
[runtime]
enable_debug = true
internetworking_model = "test"
"#;
        let config: TomlConfig = TomlConfig::load(content).unwrap();
        config.validate().unwrap_err();

        let content = r#"
[runtime]
enable_debug = true
internetworking_model = "macvtap,none"
"#;
        let config: TomlConfig = TomlConfig::load(content).unwrap();
        config.validate().unwrap_err();

        let content = r#"
[runtime]
enable_debug = true
vfio_mode = "none"
"#;
        let config: TomlConfig = TomlConfig::load(content).unwrap();
        config.validate().unwrap_err();

        let content = r#"
[runtime]
enable_debug = true
vfio_mode = "vfio,guest-kernel"
"#;
        let config: TomlConfig = TomlConfig::load(content).unwrap();
        config.validate().unwrap_err();

        let content = r#"
[runtime]
enable_debug = true
vfio_mode = "guest_kernel"
"#;
        let config: TomlConfig = TomlConfig::load(content).unwrap();
        config.validate().unwrap_err();
    }

    #[test]
    fn test_config() {
        let content = r#"
[runtime]
name = "virt-container"
enable_debug = true
experimental = ["a", "b"]
internetworking_model = "macvtap"
disable_new_netns = true
sandbox_bind_mounts = []
sandbox_cgroup_only = true
enable_tracing = true
jaeger_endpoint = "localhost:1234"
jaeger_user = "user"
jaeger_password = "pw"
enable_pprof = true
disable_guest_seccomp = true
vfio_mode = "vfio"
field_should_be_ignored = true
"#;
        let config: TomlConfig = TomlConfig::load(content).unwrap();
        config.validate().unwrap();
        assert_eq!(&config.runtime.name, "virt-container");
        assert!(config.runtime.debug);
        assert_eq!(config.runtime.experimental.len(), 2);
        assert_eq!(&config.runtime.experimental[0], "a");
        assert_eq!(&config.runtime.experimental[1], "b");
        assert_eq!(&config.runtime.internetworking_model, "macvtap");
        assert!(config.runtime.disable_new_netns);
        assert_eq!(config.runtime.sandbox_bind_mounts.len(), 0);
        assert!(config.runtime.sandbox_cgroup_only);
        assert!(config.runtime.enable_tracing);
        assert!(config.runtime.is_experiment_enabled("a"));
        assert!(config.runtime.is_experiment_enabled("b"));
        assert!(!config.runtime.is_experiment_enabled("c"));
    }
}
