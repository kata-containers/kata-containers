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

use crate::{
    resolvable_cdi_devices, DEFAULT_CDI_SPEC_DIRS, POD_RESOURCE_DEVICE_SOURCE_DEVICE_PLUGIN,
    POD_RESOURCE_DEVICE_SOURCE_DRA,
};
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

/// Collect the CDI device names in a container's DynamicResources (KEP-3695):
/// DRA allocations are reported only there, never in the device-plugin
/// `devices` field.
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

/// Select the cold-plug device list from a PodResources response, reading the
/// fields picked by `sources`: "device-plugin" (`container.devices`) and/or
/// "dra" (`dynamic_resources` CDI devices). List both only for disjoint device
/// sets: kubelet double-counts a device advertised via both at scheduling; a
/// same-device collision here is caught by the overlap check. Fail closed: an
/// unlisted source carrying CDI-resolvable data is an error, so misconfiguration
/// cannot silently boot the guest without its devices; data that never resolves
/// in the CDI cache is not cold-pluggable and is exempt.
fn select_cold_plug_devices(
    pod_resources: &PodResources,
    sources: &[String],
    spec_dirs: &[&str],
) -> Result<Vec<String>> {
    let want_device_plugin = sources
        .iter()
        .any(|s| s == POD_RESOURCE_DEVICE_SOURCE_DEVICE_PLUGIN);
    let want_dra = sources.iter().any(|s| s == POD_RESOURCE_DEVICE_SOURCE_DRA);

    let format_cdi_device_ids = |resource_name: &str, device_ids: &[String]| -> Vec<String> {
        device_ids
            .iter()
            .map(|id| format!("{}={}", resource_name, id))
            .collect()
    };

    let mut devices: Vec<String> = Vec::new();
    let mut all_device_plugin_devs: Vec<String> = Vec::new();
    let mut all_dra_devs: Vec<String> = Vec::new();

    for container in &pod_resources.containers {
        let mut device_plugin_devs: Vec<String> = Vec::new();
        for device in &container.devices {
            device_plugin_devs.extend(format_cdi_device_ids(&device.resource_name, &device.device_ids));
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
                    POD_RESOURCE_DEVICE_SOURCE_DEVICE_PLUGIN,
                    sources,
                    POD_RESOURCE_DEVICE_SOURCE_DEVICE_PLUGIN,
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
                    POD_RESOURCE_DEVICE_SOURCE_DRA,
                    sources,
                    POD_RESOURCE_DEVICE_SOURCE_DRA,
                ));
            }
        }

        if want_device_plugin {
            let deduped = dedup_strings(&device_plugin_devs);
            devices.extend(deduped.iter().cloned());
            all_device_plugin_devs.extend(deduped);
        }
        if want_dra {
            let deduped = dedup_strings(&dra_devs);
            devices.extend(deduped.iter().cloned());
            all_dra_devs.extend(deduped);
        }
    }

    if want_device_plugin && want_dra {
        overlap::check_cross_source_physical_overlap(
            &dedup_strings(&all_device_plugin_devs),
            &dedup_strings(&all_dra_devs),
            spec_dirs,
        )?;
    }

    Ok(dedup_strings(&devices))
}

pub async fn get_pod_cdi_devices(
    socket: &str,
    annotations: &HashMap<String, String>,
    sources: &[String],
) -> Result<Vec<String>> {
    if sources.is_empty() {
        // Config loading defaults this to ["device-plugin"]; if it is empty
        // anyway, fail closed rather than guess a source.
        return Err(anyhow!(
            "cold plug: pod_resource_device_sources is empty, refusing to guess a device source"
        ));
    }

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
        let devs = select_cold_plug_devices(
            &pod,
            &[POD_RESOURCE_DEVICE_SOURCE_DEVICE_PLUGIN.to_string()],
            &spec_dirs,
        )
        .unwrap();
        assert_eq!(devs, vec!["vendor.com/gpu=gpu0".to_string()]);
    }

    #[test]
    fn test_select_dra_only() {
        let spec = tempfile::tempdir().unwrap();
        let spec_dirs = [spec.path().to_str().unwrap()];
        let pod = pod_with_both_sources();
        let devs = select_cold_plug_devices(
            &pod,
            &[POD_RESOURCE_DEVICE_SOURCE_DRA.to_string()],
            &spec_dirs,
        )
        .unwrap();
        assert_eq!(devs, vec!["vendor.com/dra=gpu1".to_string()]);
    }

    #[test]
    fn test_select_both_sources() {
        let spec = tempfile::tempdir().unwrap();
        let spec_dirs = [spec.path().to_str().unwrap()];
        let pod = pod_with_both_sources();
        let devs = select_cold_plug_devices(
            &pod,
            &[
                POD_RESOURCE_DEVICE_SOURCE_DEVICE_PLUGIN.to_string(),
                POD_RESOURCE_DEVICE_SOURCE_DRA.to_string(),
            ],
            &spec_dirs,
        )
        .unwrap();
        assert_eq!(
            devs,
            vec![
                "vendor.com/gpu=gpu0".to_string(),
                "vendor.com/dra=gpu1".to_string()
            ]
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
        let err = select_cold_plug_devices(
            &pod,
            &[POD_RESOURCE_DEVICE_SOURCE_DRA.to_string()],
            &spec_dirs,
        )
        .unwrap_err();

        let msg = err.to_string();
        assert!(msg.contains("not in pod_resource_device_sources"), "{}", msg);
        assert!(msg.contains(POD_RESOURCE_DEVICE_SOURCE_DEVICE_PLUGIN), "{}", msg);
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
        let devices = select_cold_plug_devices(
            &pod,
            &[POD_RESOURCE_DEVICE_SOURCE_DRA.to_string()],
            &spec_dirs,
        )
        .expect("node-less unlisted device must not fail closed");
        assert!(!devices.iter().any(|d| d.contains("gpu0")), "{:?}", devices);
    }
}
