// Copyright (c) 2026 Kata Containers community
// SPDX-License-Identifier: Apache-2.0

use std::path::Path;
use std::time::Duration;

use cni_protocol::{CniEnvironment, CniError, CniErrorCode};
use dan_core::PodIdentity;
use k8s_openapi::api::core::v1::Pod;
use kube::config::{KubeConfigOptions, Kubeconfig};
use kube::{Api, Client, Config};

const POD_GET_TIMEOUT: Duration = Duration::from_secs(5);

pub(crate) fn identity_from_cni(environment: &CniEnvironment) -> Result<PodIdentity, CniError> {
    fn field<'a>(environment: &'a CniEnvironment, key: &'static str) -> Result<&'a str, CniError> {
        environment
            .cni_arg(key)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                CniError::new(
                    CniErrorCode::InvalidEnvironment,
                    "missing Kubernetes Pod identity in CNI_ARGS",
                    key,
                )
            })
    }

    Ok(PodIdentity {
        namespace: field(environment, "K8S_POD_NAMESPACE")?.to_owned(),
        name: field(environment, "K8S_POD_NAME")?.to_owned(),
        uid: field(environment, "K8S_POD_UID")?.to_owned(),
    })
}

pub(crate) async fn client_from_kubeconfig(path: &Path) -> Result<Client, CniError> {
    let kubeconfig = Kubeconfig::read_from(path).map_err(|error| {
        CniError::new(
            CniErrorCode::IoFailure,
            "failed to read Kata DAN kubeconfig",
            format!("{}: {error}", path.display()),
        )
    })?;
    let mut config = Config::from_custom_kubeconfig(kubeconfig, &KubeConfigOptions::default())
        .await
        .map_err(|error| {
            CniError::new(
                CniErrorCode::InvalidNetworkConfig,
                "invalid Kata DAN kubeconfig",
                error.to_string(),
            )
        })?;
    config.connect_timeout = Some(Duration::from_secs(3));
    config.read_timeout = Some(POD_GET_TIMEOUT);
    config.write_timeout = Some(POD_GET_TIMEOUT);
    // kube-client ignores NO_PROXY when inheriting HTTPS_PROXY from containerd.
    // Connect directly to the in-cluster API.
    config.proxy_url = None;
    Client::try_from(config).map_err(|error| {
        CniError::new(
            CniErrorCode::InvalidNetworkConfig,
            "failed to create Kata DAN Kubernetes client",
            error.to_string(),
        )
    })
}

pub(crate) async fn lookup_pod(client: &Client, identity: &PodIdentity) -> Result<Pod, CniError> {
    let api: Api<Pod> = Api::namespaced(client.clone(), &identity.namespace);
    let response = tokio::time::timeout(POD_GET_TIMEOUT, api.get(&identity.name)).await;
    let pod = match response {
        Ok(Ok(pod)) => pod,
        Ok(Err(kube::Error::Api(response))) if response.code == 404 => {
            return Err(CniError::new(
                CniErrorCode::RetryLater,
                "Pod is not available for DAN ADD",
                "",
            ));
        }
        Ok(Err(kube::Error::Api(response))) => {
            return Err(CniError::new(
                CniErrorCode::IoFailure,
                "failed to query Pod for network selection",
                format!("Kubernetes API returned HTTP {}", response.code),
            ));
        }
        Ok(Err(error)) => {
            return Err(CniError::new(
                CniErrorCode::IoFailure,
                "failed to query Pod for network selection",
                error.to_string(),
            ));
        }
        Err(_) => {
            return Err(CniError::new(
                CniErrorCode::RetryLater,
                "timed out querying Pod for network selection",
                "Kubernetes API did not respond within five seconds",
            ));
        }
    };
    if pod.metadata.uid.as_deref() != Some(identity.uid.as_str()) {
        return Err(CniError::new(
            CniErrorCode::InvalidNetworkConfig,
            "invalid CNI configuration",
            "Pod UID changed during DAN ADD",
        ));
    }
    Ok(pod)
}

pub(crate) fn supports_dan_runtime(pod: &Pod, dan_runtime_class: &str) -> bool {
    pod.spec
        .as_ref()
        .and_then(|spec| spec.runtime_class_name.as_deref())
        == Some(dan_runtime_class)
}
