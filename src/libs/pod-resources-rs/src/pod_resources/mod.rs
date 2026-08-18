// Copyright (c) 2026 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

pub mod overlap;
pub mod v1;

use v1::pod_resources_lister_client::PodResourcesListerClient;
use v1::{ContainerResources, PodResources};

use std::collections::HashMap;
use std::convert::TryFrom;

use anyhow::{anyhow, Context, Result};

use crate::{resolvable_cdi_devices, DeviceSource, DEFAULT_CDI_SPEC_DIRS};
use hyper_util::rt::TokioIo;
use tokio::net::UnixStream;
use tokio::time::{timeout, Duration};
use tonic::transport::{Channel, Endpoint, Uri};
use tower::service_fn;

use crate::pod_resources::v1::GetPodResourcesRequest;

// containerd CRI annotations
const SANDBOX_NAME_ANNOTATION: &str = "io.kubernetes.cri.sandbox-name";
const SANDBOX_NAMESPACE_ANNOTATION: &str = "io.kubernetes.cri.sandbox-namespace";

// CRI-O annotations (fallback)
const CRIO_NAME_ANNOTATION: &str = "io.kubernetes.cri-o.KubeName";
const CRIO_NAMESPACE_ANNOTATION: &str = "io.kubernetes.cri-o.Namespace";
pub const DEFAULT_POD_RESOURCES_PATH: &str = "/var/lib/kubelet/pod-resources";
pub const DEFAULT_POD_RESOURCES_TIMEOUT: Duration = Duration::from_secs(10);
pub const CDI_K8S_PREFIX: &str = "cdi.k8s.io/";
const MAX_RECV_MSG_SIZE: usize = 16 * 1024 * 1024; // 16MB

// Create a gRPC channel to the specified Unix socket
async fn create_grpc_channel(socket_path: &str) -> Result<Channel> {
    let socket_path = socket_path.trim_start_matches("unix://");
    let socket_path_owned = socket_path.to_string();

    // Create a gRPC endpoint with a timeout
    let endpoint = Endpoint::try_from("http://[::]:50051")
        .context("failed to create endpoint")?
        .timeout(DEFAULT_POD_RESOURCES_TIMEOUT);

    // Connect to the Unix socket using a custom connector
    let channel = endpoint
        .connect_with_connector(service_fn(move |_: Uri| {
            let socket_path = socket_path_owned.clone();
            async move {
                let stream = UnixStream::connect(&socket_path).await.map_err(|e| {
                    std::io::Error::new(
                        e.kind(),
                        format!("failed to connect to {}: {}", socket_path, e),
                    )
                })?;
                Ok::<_, std::io::Error>(TokioIo::new(stream))
            }
        }))
        .await
        .context("failed to connect to unix socket")?;

    Ok(channel)
}

/// Collect the CDI device names in a container's DynamicResources (KEP-3695).
///
/// A PodResources response carries device data in two places that never
/// shadow each other, which is why each source gets its own collector:
///
/// ```text
/// containers:
/// - name: workload
///   devices:                              # device-plugin allocations
///   - resource_name: "nvidia.com/pgpu"
///     device_ids: ["GPU-8c25ea9f"]
///   dynamic_resources:                    # DRA allocations (read here)
///   - claim_resources:
///     - cdi_devices:
///       - name: "gpu.example.com/gpu=gpu0"
/// ```
///
/// DRA allocations appear only under `dynamic_resources`; the legacy
/// `devices` field is read by `select_cold_plug_devices` when
/// "device-plugin" is a trusted source.
fn collect_pod_resource_cdi_devices(container: &ContainerResources) -> Vec<String> {
    let mut devices = Vec::new();
    for dr in &container.dynamic_resources {
        for cr in &dr.claim_resources {
            for cdi_dev in &cr.cdi_devices {
                if cdi_dev.name.is_empty() {
                    continue;
                }
                devices.push(cdi_dev.name.clone());
            }
        }
    }
    devices
}

/// Deduplicate preserving order: a ResourceClaim shared by several containers
/// reports the same CDI device once per container, and plugging it twice
/// would duplicate its OCI edits.
fn dedup_strings(input: &[String]) -> Vec<String> {
    let mut seen = std::collections::HashSet::with_capacity(input.len());
    let mut out = Vec::with_capacity(input.len());
    for s in input {
        if seen.insert(s.clone()) {
            out.push(s.clone());
        }
    }
    out
}

/// Cold-plug CDI device names selected from a PodResources response, kept per
/// source so the caller can run cross-source enforcement (the physical overlap
/// check) before flattening for attachment.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct SelectedColdPlugDevices {
    /// Devices from `container.devices` (device-plugin allocations).
    pub device_plugin: Vec<String>,
    /// Devices from `dynamic_resources` (DRA allocations).
    pub dra: Vec<String>,
}

impl SelectedColdPlugDevices {
    /// The final plug list: device-plugin devices first, then DRA devices,
    /// deduplicated preserving order.
    ///
    /// Precondition when both lists are non-empty: run
    /// `overlap::check_cross_source_physical_overlap` on the two lists
    /// first. Flattening erases the source, and a same-physical-device
    /// collision that was not rejected would be cold-plugged twice.
    pub fn flattened(&self) -> Vec<String> {
        let mut all = self.device_plugin.clone();
        all.extend(self.dra.iter().cloned());
        dedup_strings(&all)
    }
}

/// Select the cold-plug devices from a PodResources response, reading the
/// fields picked by `sources`: `DevicePlugin` (`container.devices`) and/or
/// `Dra` (`dynamic_resources` CDI devices). An empty `sources` trusts neither
/// field. Fail closed: an unlisted source carrying CDI-resolvable data is an
/// error, so misconfiguration cannot silently boot the guest without its
/// devices; data that never resolves in the CDI cache is not cold-pluggable
/// and is exempt. No cross-source policy is applied here: the response is
/// passed through per source, and the overlap check runs in the caller's
/// device path.
fn select_cold_plug_devices(
    pod_resources: &PodResources,
    sources: &[DeviceSource],
    spec_dirs: &[&str],
) -> Result<SelectedColdPlugDevices> {
    let want_device_plugin = sources.contains(&DeviceSource::DevicePlugin);
    let want_dra = sources.contains(&DeviceSource::Dra);
    let sources_display =
        |sources: &[DeviceSource]| sources.iter().map(|s| s.to_string()).collect::<Vec<_>>();

    let format_cdi_device_ids = |resource_name: &str, device_ids: &[String]| -> Vec<String> {
        device_ids
            .iter()
            .map(|id| format!("{}={}", resource_name, id))
            .collect()
    };

    let mut selected = SelectedColdPlugDevices::default();

    for container in &pod_resources.containers {
        let mut device_plugin_devs: Vec<String> = Vec::new();
        for device in &container.devices {
            device_plugin_devs.extend(format_cdi_device_ids(
                &device.resource_name,
                &device.device_ids,
            ));
        }

        let dra_devs = collect_pod_resource_cdi_devices(container);

        if !want_device_plugin && !device_plugin_devs.is_empty() {
            let resolvable = resolvable_cdi_devices(spec_dirs, &device_plugin_devs);
            if !resolvable.is_empty() {
                return Err(anyhow!(
                    "cold plug: container {:?} has cold-pluggable (CDI-resolvable) device-plugin \
                     PodResources data ({:?}) but {:?} is not in pod_resource_device_sources={:?}; \
                     add {:?} to the config option or this data will be silently dropped",
                    container.name,
                    resolvable,
                    DeviceSource::DevicePlugin.to_string(),
                    sources_display(sources),
                    DeviceSource::DevicePlugin.to_string(),
                ));
            }
        }
        if !want_dra && !dra_devs.is_empty() {
            let resolvable = resolvable_cdi_devices(spec_dirs, &dra_devs);
            if !resolvable.is_empty() {
                return Err(anyhow!(
                    "cold plug: container {:?} has cold-pluggable (CDI-resolvable) DRA \
                     PodResources data ({:?}) but {:?} is not in pod_resource_device_sources={:?}; \
                     add {:?} to the config option or this data will be silently dropped",
                    container.name,
                    resolvable,
                    DeviceSource::Dra.to_string(),
                    sources_display(sources),
                    DeviceSource::Dra.to_string(),
                ));
            }
        }

        if want_device_plugin {
            selected
                .device_plugin
                .extend(dedup_strings(&device_plugin_devs));
        }
        if want_dra {
            selected.dra.extend(dedup_strings(&dra_devs));
        }
    }

    selected.device_plugin = dedup_strings(&selected.device_plugin);
    selected.dra = dedup_strings(&selected.dra);
    Ok(selected)
}

/// Resolve the pod's cold-plug CDI devices from kubelet's PodResources API,
/// trusting only the sources named in `sources`. An empty `sources` trusts
/// neither: a pod without cold-pluggable data proceeds normally, one that
/// carries it fails closed through the unlisted-source check.
///
/// `DevicePlugin` and `Dra` may be listed together: an operator can serve
/// different device classes through each API on the same node, and a node
/// migrating between the two runs both for a while. The sets have to stay
/// disjoint, because kubelet counts a device advertised via both APIs twice
/// at scheduling. This function passes each source's devices through as-is;
/// the caller runs `overlap::check_cross_source_physical_overlap` on the
/// returned lists before attaching, where a same-device collision is
/// rejected instead of being plugged twice.
pub async fn get_pod_cdi_devices(
    socket: &str,
    annotations: &HashMap<String, String>,
    sources: &[DeviceSource],
) -> Result<SelectedColdPlugDevices> {
    let pod_name = annotations
        .get(SANDBOX_NAME_ANNOTATION)
        .or_else(|| annotations.get(CRIO_NAME_ANNOTATION))
        .ok_or_else(|| {
            anyhow::anyhow!(
                "cold plug: missing annotation {} or {}",
                SANDBOX_NAME_ANNOTATION,
                CRIO_NAME_ANNOTATION
            )
        })?;

    let pod_namespace = annotations
        .get(SANDBOX_NAMESPACE_ANNOTATION)
        .or_else(|| annotations.get(CRIO_NAMESPACE_ANNOTATION))
        .ok_or_else(|| {
            anyhow::anyhow!(
                "cold plug: missing annotation {} or {}",
                SANDBOX_NAMESPACE_ANNOTATION,
                CRIO_NAMESPACE_ANNOTATION
            )
        })?;

    // Create gRPC channel to kubelet pod-resources socket
    let channel = create_grpc_channel(socket)
        .await
        .context("cold plug: failed to connect to kubelet")?;

    // Create PodResourcesLister client
    let mut client = PodResourcesListerClient::new(channel)
        .max_decoding_message_size(MAX_RECV_MSG_SIZE)
        .max_encoding_message_size(MAX_RECV_MSG_SIZE);

    // Prepare and send GetPodResources request
    let request = tonic::Request::new(GetPodResourcesRequest {
        pod_name: pod_name.to_string(),
        pod_namespace: pod_namespace.to_string(),
    });

    // Await response with timeout
    let response = timeout(DEFAULT_POD_RESOURCES_TIMEOUT, client.get(request))
        .await
        .context("cold plug: GetPodResources timeout")?
        .context("cold plug: GetPodResources RPC failed")?;

    // Extract PodResources from response
    let pod_resources = response
        .into_inner()
        .pod_resources
        .ok_or_else(|| anyhow!("cold plug: PodResources is nil"))?;

    select_cold_plug_devices(&pod_resources, sources, &DEFAULT_CDI_SPEC_DIRS)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pod_resources::v1::{
        CdiDevice, ClaimResource, ContainerDevices, ContainerResources, DynamicResource,
        PodResources,
    };

    fn pod_with_both_sources() -> PodResources {
        PodResources {
            name: "pod-a".to_string(),
            namespace: "ns-a".to_string(),
            containers: vec![ContainerResources {
                name: "ctr-a".to_string(),
                devices: vec![ContainerDevices {
                    resource_name: "vendor.com/gpu".to_string(),
                    device_ids: vec!["gpu0".to_string()],
                    ..Default::default()
                }],
                dynamic_resources: vec![DynamicResource {
                    claim_name: "claim-a".to_string(),
                    claim_resources: vec![ClaimResource {
                        cdi_devices: vec![CdiDevice {
                            name: "vendor.com/dra=gpu1".to_string(),
                        }],
                        ..Default::default()
                    }],
                    ..Default::default()
                }],
                ..Default::default()
            }],
        }
    }

    #[test]
    fn test_collect_pod_resource_cdi_devices() {
        let pod = pod_with_both_sources();
        let dra = collect_pod_resource_cdi_devices(&pod.containers[0]);
        assert_eq!(dra, vec!["vendor.com/dra=gpu1".to_string()]);
    }

    #[test]
    fn test_dedup_strings() {
        let input = vec![
            "a".to_string(),
            "b".to_string(),
            "a".to_string(),
            "c".to_string(),
            "b".to_string(),
        ];
        assert_eq!(
            dedup_strings(&input),
            vec!["a".to_string(), "b".to_string(), "c".to_string()]
        );
    }

    #[test]
    fn test_select_device_plugin_only() {
        // Empty spec dir: nothing resolves, so the unlisted DRA data is exempt.
        let spec = tempfile::tempdir().unwrap();
        let spec_dirs = [spec.path().to_str().unwrap()];
        let pod = pod_with_both_sources();
        let devs =
            select_cold_plug_devices(&pod, &[DeviceSource::DevicePlugin], &spec_dirs).unwrap();
        assert_eq!(devs.device_plugin, vec!["vendor.com/gpu=gpu0".to_string()]);
        assert!(devs.dra.is_empty());
        assert_eq!(devs.flattened(), vec!["vendor.com/gpu=gpu0".to_string()]);
    }

    #[test]
    fn test_select_dra_only() {
        let spec = tempfile::tempdir().unwrap();
        let spec_dirs = [spec.path().to_str().unwrap()];
        let pod = pod_with_both_sources();
        let devs = select_cold_plug_devices(&pod, &[DeviceSource::Dra], &spec_dirs).unwrap();
        assert!(devs.device_plugin.is_empty());
        assert_eq!(devs.dra, vec!["vendor.com/dra=gpu1".to_string()]);
    }

    #[test]
    fn test_select_both_sources() {
        let spec = tempfile::tempdir().unwrap();
        let spec_dirs = [spec.path().to_str().unwrap()];
        let pod = pod_with_both_sources();
        let devs = select_cold_plug_devices(
            &pod,
            &[DeviceSource::DevicePlugin, DeviceSource::Dra],
            &spec_dirs,
        )
        .unwrap();
        assert_eq!(devs.device_plugin, vec!["vendor.com/gpu=gpu0".to_string()]);
        assert_eq!(devs.dra, vec!["vendor.com/dra=gpu1".to_string()]);
        assert_eq!(
            devs.flattened(),
            vec![
                "vendor.com/gpu=gpu0".to_string(),
                "vendor.com/dra=gpu1".to_string()
            ]
        );
    }

    #[test]
    fn test_select_both_sources_two_containers_groups_by_source() {
        // The plug list groups by source (all device-plugin devices, then
        // all DRA devices), not by container. This pins the order the
        // flattened list feeds into VFIO attachment.
        let spec = tempfile::tempdir().unwrap();
        let spec_dirs = [spec.path().to_str().unwrap()];
        let mut pod = pod_with_both_sources();
        let mut second = pod.containers[0].clone();
        second.name = "ctr-b".to_string();
        second.devices[0].device_ids = vec!["gpu2".to_string()];
        second.dynamic_resources[0].claim_resources[0].cdi_devices[0].name =
            "vendor.com/dra=gpu3".to_string();
        pod.containers.push(second);

        let devs = select_cold_plug_devices(
            &pod,
            &[DeviceSource::DevicePlugin, DeviceSource::Dra],
            &spec_dirs,
        )
        .unwrap();
        assert_eq!(
            devs.flattened(),
            vec![
                "vendor.com/gpu=gpu0".to_string(),
                "vendor.com/gpu=gpu2".to_string(),
                "vendor.com/dra=gpu1".to_string(),
                "vendor.com/dra=gpu3".to_string(),
            ]
        );
    }

    #[test]
    fn test_select_no_sources_without_resolvable_data_is_ok() {
        // Explicit "trust neither": nothing resolves in the empty spec dir,
        // so the pod proceeds with no cold-plug devices.
        let spec = tempfile::tempdir().unwrap();
        let spec_dirs = [spec.path().to_str().unwrap()];
        let pod = pod_with_both_sources();
        let devs = select_cold_plug_devices(&pod, &[], &spec_dirs).unwrap();
        assert!(devs.device_plugin.is_empty());
        assert!(devs.dra.is_empty());
        assert!(devs.flattened().is_empty());
    }

    #[test]
    fn test_select_no_sources_with_resolvable_data_errors() {
        // Explicit "trust neither" still fails closed when the pod carries
        // cold-pluggable data that would be silently dropped.
        let spec = tempfile::tempdir().unwrap();
        write_cdi_spec(
            spec.path(),
            "dp",
            "vendor.com/gpu",
            &[("gpu0", "/dev/null")],
        );
        let spec_dirs = [spec.path().to_str().unwrap()];
        let pod = pod_with_both_sources();
        let err = select_cold_plug_devices(&pod, &[], &spec_dirs).unwrap_err();
        assert!(
            err.to_string()
                .contains("not in pod_resource_device_sources"),
            "{}",
            err
        );
    }

    fn write_cdi_spec(dir: &std::path::Path, name: &str, kind: &str, devices: &[(&str, &str)]) {
        let mut content = format!("cdiVersion: \"0.5.0\"\nkind: \"{kind}\"\ndevices:\n");
        for (dev_name, path) in devices {
            content.push_str(&format!(
                "  - name: \"{dev_name}\"\n    containerEdits:\n      deviceNodes:\n      - path: \"{path}\"\n"
            ));
        }
        std::fs::write(dir.join(format!("{name}.yaml")), content).unwrap();
    }

    #[test]
    fn test_select_unlisted_source_with_resolvable_device_errors() {
        // gpu0 resolves but "device-plugin" is not listed: fail closed.
        let spec = tempfile::tempdir().unwrap();
        write_cdi_spec(
            spec.path(),
            "dp",
            "vendor.com/gpu",
            &[("gpu0", "/dev/null")],
        );
        let spec_dirs = [spec.path().to_str().unwrap()];

        let pod = pod_with_both_sources();
        let err = select_cold_plug_devices(&pod, &[DeviceSource::Dra], &spec_dirs).unwrap_err();

        let msg = err.to_string();
        assert!(
            msg.contains("not in pod_resource_device_sources"),
            "{}",
            msg
        );
        assert!(msg.contains("device-plugin"), "{}", msg);
        assert!(msg.contains("vendor.com/gpu=gpu0"), "{}", msg);
    }

    #[test]
    fn test_select_unlisted_source_without_device_nodes_is_exempt() {
        // gpu0 resolves in the cache but declares no device nodes (env-only
        // CDI): nothing is cold-plugged for it, so an unlisted source must not
        // fail closed on it.
        let spec = tempfile::tempdir().unwrap();
        std::fs::write(
            spec.path().join("dp.yaml"),
            "cdiVersion: \"0.5.0\"\nkind: \"vendor.com/gpu\"\ndevices:\n  - name: \"gpu0\"\n    containerEdits:\n      env:\n      - \"FOO=bar\"\n",
        )
        .unwrap();
        let spec_dirs = [spec.path().to_str().unwrap()];

        let pod = pod_with_both_sources();
        let devices = select_cold_plug_devices(&pod, &[DeviceSource::Dra], &spec_dirs)
            .expect("node-less unlisted device must not fail closed");
        assert!(
            !devices.flattened().iter().any(|d| d.contains("gpu0")),
            "{:?}",
            devices
        );
    }
}
