// Copyright (c) 2026 Kata Containers community
// SPDX-License-Identifier: Apache-2.0

use cni_protocol::{CniCommand, CniEnvironment, CniError, CniErrorCode, DelegateExecutor, NetConf};
use dan_core::{
    AttachmentIdentity, Backend, BackendError, NetworkContext, NetworkSelection, PodIdentity,
    Provider, ProviderError,
};
use serde_json::Value;
use std::net::IpAddr;

use crate::backend_selection::create_backend;
use crate::dan_file::{encode_device, DanFile};
use crate::provider_selection::build_configured_provider;
use crate::request_selection::{select_request_path, RequestPath};
use crate::workload::{
    client_from_kubeconfig, identity_from_cni, lookup_pod, supports_dan_runtime,
};

#[derive(Clone, Debug, PartialEq)]
pub enum CommandOutput {
    Json(Value),
    Empty,
}

pub fn dispatch(environment: CniEnvironment, input: &[u8]) -> Result<CommandOutput, CniError> {
    let config = NetConf::parse(input)?;
    if environment.command == CniCommand::Check
        && matches!(
            config.cni.cni_version.as_str(),
            "0.1.0" | "0.2.0" | "0.3.0" | "0.3.1"
        )
    {
        return Err(CniError::new(
            CniErrorCode::IncompatibleVersion,
            "CNI CHECK requires version 0.4.0 or newer",
            "",
        )
        .with_cni_version(config.cni.cni_version));
    }
    let result = match select_request_path(&config.runtime_config) {
        Ok(RequestPath::Delegate) => delegate(&environment, &config),
        Ok(RequestPath::Dan) => {
            runtime().and_then(|runtime| runtime.block_on(dan(&environment, &config)))
        }
        Err(error) => Err(error),
    };
    result.map_err(|error| error.with_cni_version(config.cni.cni_version))
}

fn delegate(environment: &CniEnvironment, config: &NetConf) -> Result<CommandOutput, CniError> {
    let executor = DelegateExecutor::new();
    match environment.command {
        CniCommand::Add => {
            let result = executor.add(
                &config.delegate,
                &config.cni.cni_version,
                &config.runtime_config,
                environment,
                config.cni.prev_result.as_ref(),
            )?;
            Ok(CommandOutput::Json(result))
        }
        CniCommand::Del => {
            executor.delete(
                &config.delegate,
                &config.cni.cni_version,
                &config.runtime_config,
                environment,
                config.cni.prev_result.as_ref(),
            )?;
            Ok(CommandOutput::Empty)
        }
        CniCommand::Check => {
            executor.check(
                &config.delegate,
                &config.cni.cni_version,
                &config.runtime_config,
                environment,
                config.cni.prev_result.as_ref(),
            )?;
            Ok(CommandOutput::Empty)
        }
        _ => unreachable!("attachment command"),
    }
}

async fn dan(environment: &CniEnvironment, config: &NetConf) -> Result<CommandOutput, CniError> {
    let attachment = attachment_identity(environment)?;
    let backend = configured_backend(config)?;
    let store = dan_file(config)?;
    match environment.command {
        CniCommand::Del => {
            // Clean this endpoint without querying the Pod, even if the file is damaged.
            // File cleanup preserves damaged data rather than discarding other interfaces.
            backend.del(&attachment).await.map_err(backend_error)?;
            store.remove(&attachment)?;
            Ok(CommandOutput::Empty)
        }
        CniCommand::Check => {
            let device = store.read(&attachment)?;
            let (_, expected) = encode_device(&device, &config.cni.cni_version);
            check_previous_result(config.cni.prev_result.as_ref(), &expected)?;
            backend
                .check(&attachment, &device)
                .await
                .map_err(backend_error)?;
            Ok(CommandOutput::Empty)
        }
        CniCommand::Add => {
            let class = config
                .dan_runtime_class
                .as_deref()
                .ok_or_else(|| missing_config("danRuntimeClass"))?;
            let kubeconfig = config
                .kubeconfig
                .as_deref()
                .ok_or_else(|| missing_config("kubeconfig"))?;
            let identity = identity_from_cni(environment)?;
            let client = client_from_kubeconfig(kubeconfig).await?;
            let pod = lookup_pod(&client, &identity).await?;
            if !supports_dan_runtime(&pod, class) {
                return Err(invalid_config("Pod RuntimeClass is not eligible for DAN"));
            }
            let provider = build_configured_provider(config, client, pod)?;
            add_dan(
                &attachment,
                config,
                provider.as_ref(),
                identity,
                backend.as_ref(),
                &store,
            )
            .await
            .map(CommandOutput::Json)
        }
        _ => unreachable!("attachment command"),
    }
}

async fn add_dan(
    attachment: &AttachmentIdentity,
    config: &NetConf,
    provider: &dyn Provider,
    identity: PodIdentity,
    backend: &dyn Backend,
    store: &DanFile,
) -> Result<Value, CniError> {
    let selection = NetworkSelection {
        network_name: config.cni.name.clone(),
        if_name: attachment.if_name.clone(),
    };
    let (allocation, logical_port, port_config) = provider
        .resolve_with_port_config(&identity, &selection)
        .await
        .map_err(provider_error)?;
    allocation.validate().map_err(provider_error)?;
    backend
        .capabilities()
        .validate(&allocation)
        .map_err(backend_error)?;
    let context = NetworkContext {
        container_id: attachment.container_id.clone(),
        selection,
        allocation,
        logical_port,
    };
    let outcome = backend
        .add_with_port_config(&context, &port_config)
        .await
        .map_err(backend_error)?;
    // A Pod update may have been consumed before the port became visible to the daemon.
    // Once the port exists, later updates can find it; reject an outdated initial snapshot.
    if let Err(error) = provider
        .verify_port_config(
            &identity,
            &context.selection,
            &context.logical_port,
            &port_config,
        )
        .await
    {
        let error = provider_error(error);
        if let Err(cleanup) = backend.rollback_add(&outcome.state).await {
            return Err(CniError::new(
                CniErrorCode::IoFailure,
                "DAN initialization failed and endpoint rollback failed",
                format!("{error}; rollback: {cleanup}"),
            ));
        }
        return Err(error);
    }
    let (json, result) = encode_device(&outcome.device, &config.cni.cni_version);
    if let Err(error) = store.publish(&attachment.container_id, &json) {
        backend
            .rollback_add(&outcome.state)
            .await
            .map_err(backend_error)?;
        return Err(error);
    }
    Ok(result)
}

fn check_previous_result(previous: Option<&Value>, expected: &Value) -> Result<(), CniError> {
    let previous = previous.ok_or_else(|| invalid_config("CHECK requires prevResult"))?;
    let previous_ips = result_addresses(previous);
    if result_addresses(expected)
        .iter()
        .any(|address| !previous_ips.contains(address))
    {
        return Err(invalid_config(
            "prevResult is missing the expected DAN IP addresses",
        ));
    }
    for interface in expected["interfaces"].as_array().into_iter().flatten() {
        let name = &interface["name"];
        if !previous["interfaces"].as_array().is_some_and(|interfaces| {
            interfaces
                .iter()
                .any(|interface| interface.get("name") == Some(name))
        }) {
            return Err(invalid_config(
                "prevResult is missing the expected host interface",
            ));
        }
    }
    Ok(())
}

fn result_addresses(result: &Value) -> Vec<(IpAddr, u8)> {
    result["ips"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|ip| {
            let (address, prefix) = ip["address"].as_str()?.split_once('/')?;
            Some((address.parse().ok()?, prefix.parse().ok()?))
        })
        .collect()
}

fn runtime() -> Result<tokio::runtime::Runtime, CniError> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| {
            CniError::new(
                CniErrorCode::IoFailure,
                "failed to start CNI runtime",
                error.to_string(),
            )
        })
}

fn configured_backend(config: &NetConf) -> Result<Box<dyn Backend>, CniError> {
    create_backend(
        config
            .backend
            .as_ref()
            .ok_or_else(|| missing_config("backend"))?,
    )
    .map_err(backend_error)
}

fn dan_file(config: &NetConf) -> Result<DanFile, CniError> {
    let directory = config
        .dan_conf_dir
        .as_ref()
        .ok_or_else(|| missing_config("danConfDir"))?;
    if !directory.is_absolute() {
        return Err(invalid_config("danConfDir must be an absolute path"));
    }
    Ok(DanFile::new(directory.clone()))
}

fn attachment_identity(environment: &CniEnvironment) -> Result<AttachmentIdentity, CniError> {
    Ok(AttachmentIdentity {
        container_id: environment
            .container_id
            .clone()
            .ok_or_else(|| invalid_config("CNI_CONTAINERID is required"))?,
        if_name: environment
            .if_name
            .clone()
            .ok_or_else(|| invalid_config("CNI_IFNAME is required"))?,
    })
}

fn invalid_config(details: impl Into<String>) -> CniError {
    CniError::new(
        CniErrorCode::InvalidNetworkConfig,
        "invalid CNI configuration",
        details,
    )
}

fn missing_config(field: &str) -> CniError {
    invalid_config(format!("{field} is required for DAN"))
}

fn backend_error(error: BackendError) -> CniError {
    let code = if matches!(error, BackendError::InvalidInput(_)) {
        CniErrorCode::InvalidNetworkConfig
    } else {
        CniErrorCode::IoFailure
    };
    CniError::new(code, "DAN backend failed", error.to_string())
}

fn provider_error(error: ProviderError) -> CniError {
    let code = match error {
        ProviderError::NotReady(_) => CniErrorCode::RetryLater,
        ProviderError::InvalidData(_) => CniErrorCode::InvalidNetworkConfig,
        ProviderError::RequestFailed(_) => CniErrorCode::IoFailure,
    };
    CniError::new(code, "DAN allocation failed", error.to_string())
}

#[cfg(test)]
mod integration_tests;

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn check_result_matches() {
        let (_, expected) = encode_device(&crate::test_support::device("eth0"), "1.0.0");
        let mut aggregated = expected.clone();
        aggregated["interfaces"]
            .as_array_mut()
            .expect("interfaces")
            .push(json!({"name": "other"}));
        aggregated["ips"]
            .as_array_mut()
            .expect("IPs")
            .push(json!({"address": "192.0.2.4/24"}));
        check_previous_result(Some(&aggregated), &expected).expect("semantic IP comparison");
        assert!(check_previous_result(None, &expected).is_err());
        let mut missing_interface = aggregated.clone();
        missing_interface["interfaces"] = json!([{"name": "other"}]);
        assert!(check_previous_result(Some(&missing_interface), &expected).is_err());
        aggregated["ips"][0]["address"] = json!("10.42.0.9/32");
        assert!(check_previous_result(Some(&aggregated), &expected).is_err());
    }
}
