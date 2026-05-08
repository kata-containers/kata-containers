// Copyright (c) 2026 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

pub mod v1;

use v1::pod_resources_lister_client::PodResourcesListerClient;

use std::collections::HashMap;
use std::convert::TryFrom;

use anyhow::{Context, Result, anyhow};
use hyper_util::rt::TokioIo;
use tokio::net::UnixStream;
use tokio::time::{Duration, timeout};
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

pub async fn get_pod_cdi_devices(
    socket: &str,
    annotations: &HashMap<String, String>,
) -> Result<Vec<String>> {
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

    // Format device specifications
    let format_cdi_device_ids = |resource_name: &str, device_ids: &[String]| -> Vec<String> {
        device_ids
            .iter()
            .map(|id| format!("{}={}", resource_name, id))
            .collect()
    };

    // Collect all device specifications from all containers
    let mut devices = Vec::new();
    for container in &pod_resources.containers {
        for device in &container.devices {
            let cdi_devices = format_cdi_device_ids(&device.resource_name, &device.device_ids);
            devices.extend(cdi_devices);
        }
    }

    Ok(devices)
}
