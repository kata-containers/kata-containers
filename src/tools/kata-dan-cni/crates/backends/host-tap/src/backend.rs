// Copyright (c) 2026 Kata Containers community
// SPDX-License-Identifier: Apache-2.0

use dan_core::{
    AttachmentIdentity, Backend, BackendAddOutcome, BackendCapabilities, BackendError,
    BackendFuture, BackendState, DanDevice, DanEndpoint, HostPortConfig, NetworkContext,
};
use ovs::{CreateOutcome, OvsBinding, OvsError, OvsPortSpec};
use serde_json::Value;
use std::collections::BTreeMap;

use crate::naming::ResourceIdentity;
use crate::state::HostTapState;
use crate::tap::{self, TapSpec};
use crate::{BACKEND_NAME, OVS_BRIDGE_NAME};

#[derive(Clone, Copy, Debug, Default)]
pub struct HostTapBackend;

async fn ovs_operation<T: Send + 'static>(
    operation: impl FnOnce() -> Result<T, OvsError> + Send + 'static,
) -> Result<T, BackendError> {
    tokio::task::spawn_blocking(operation)
        .await
        .map_err(|error| BackendError::OperationFailed(format!("OVS worker failed: {error}")))?
        .map_err(|error| match error {
            OvsError::InvalidInput(message) => BackendError::InvalidInput(message),
            OvsError::OperationFailed(message) | OvsError::TransactionConflict(message) => {
                BackendError::OperationFailed(message)
            }
        })
}

impl HostTapBackend {
    pub fn from_config(config: &Value) -> Result<Self, BackendError> {
        let config = config.as_object().ok_or_else(|| {
            BackendError::InvalidInput("backend configuration must be a JSON object".to_string())
        })?;
        if config.get("type").and_then(Value::as_str) != Some(BACKEND_NAME) {
            return Err(BackendError::InvalidInput(
                "backend configuration field `type` must be host-tap".to_string(),
            ));
        }
        if let Some(field) = config.keys().find(|field| field.as_str() != "type") {
            return Err(BackendError::InvalidInput(format!(
                "unknown host-tap backend configuration field `{field}`"
            )));
        }
        Ok(Self::new())
    }

    pub fn new() -> Self {
        Self
    }

    pub fn prepare(&self, context: &NetworkContext) -> Result<DanDevice, BackendError> {
        Ok(PreparedAttachment::from_context(context)?.dan_device)
    }
}

struct PreparedAttachment {
    tap_spec: TapSpec,
    ovs_spec: OvsPortSpec,
    host_state: HostTapState,
    dan_device: DanDevice,
}

impl PreparedAttachment {
    fn from_context(context: &NetworkContext) -> Result<Self, BackendError> {
        let identity = ResourceIdentity::try_from(context)?;
        if context.allocation.mtu == 0 || context.logical_port.name.is_empty() {
            return Err(BackendError::InvalidInput(
                "MTU and logical port are required".to_string(),
            ));
        }
        let tap_spec = TapSpec {
            tap_name: identity.tap_name.clone(),
            owner_id: identity.owner_id.clone(),
            mtu: context.allocation.mtu,
        };

        let ovs_spec = OvsPortSpec {
            bridge_name: OVS_BRIDGE_NAME.to_string(),
            port_name: identity.tap_name.clone(),
            interface_type: String::new(),
            options: BTreeMap::new(),
            mtu_request: None,
            iface_id: context.logical_port.name.clone(),
            encapsulation_ip: context.logical_port.host_binding.encapsulation_ip,
            owner_id: identity.owner_id.clone(),
        };

        let host_state = HostTapState {
            tap_name: identity.tap_name.clone(),
            owner_id: identity.owner_id,
            created_ovs: false,
        };
        let dan_device = DanDevice {
            guest_if_name: context.selection.if_name.clone(),
            endpoint: DanEndpoint::HostTap {
                tap_name: identity.tap_name,
            },
            allocation: context.allocation.clone(),
        };

        Ok(Self {
            tap_spec,
            ovs_spec,
            host_state,
            dan_device,
        })
    }
}

impl Backend for HostTapBackend {
    fn name(&self) -> &'static str {
        BACKEND_NAME
    }

    fn capabilities(&self) -> BackendCapabilities {
        BackendCapabilities {
            ipv4: true,
            ipv6: true,
        }
    }

    fn add<'a>(&'a self, context: &'a NetworkContext) -> BackendFuture<'a, BackendAddOutcome> {
        Box::pin(async move {
            self.add_with_port_config(context, &HostPortConfig::default())
                .await
        })
    }

    fn add_with_port_config<'a>(
        &'a self,
        context: &'a NetworkContext,
        config: &'a HostPortConfig,
    ) -> BackendFuture<'a, BackendAddOutcome> {
        Box::pin(async move {
            config
                .validate()
                .map_err(|error| BackendError::InvalidInput(error.to_string()))?;
            let prepared = PreparedAttachment::from_context(context)?;
            if config.has_qos() {
                ovs_operation(|| {
                    use ovs::{OvsControl, OvsDbControl};
                    match OvsDbControl::default().bridge_datapath_type(OVS_BRIDGE_NAME)? {
                        Some(datapath) if datapath.is_empty() || datapath == "system" => Ok(()),
                        Some(_) => Err(OvsError::InvalidInput(
                            "host-tap rate and Netem settings require the OVS system datapath"
                                .into(),
                        )),
                        None => Err(OvsError::OperationFailed(
                            "OVS br-int does not exist".into(),
                        )),
                    }
                })
                .await?;
            }
            tap::create(&prepared.tap_spec).await?;

            let port_config = config.clone();
            let ovs_outcome = match ovs_operation(move || {
                OvsBinding::default()
                    .create_or_validate_with_config(&prepared.ovs_spec, &port_config)
            })
            .await
            {
                Ok(outcome) => outcome,
                Err(ovs_error) => {
                    if let Err(cleanup_error) =
                        tap::delete(&prepared.host_state.tap_name, &prepared.host_state.owner_id)
                            .await
                    {
                        return Err(BackendError::OperationFailed(format!(
                            "failed to create OVS port ({ovs_error}) and failed to roll back TAP `{}` ({cleanup_error})",
                            prepared.host_state.tap_name
                        )));
                    }
                    return Err(ovs_error);
                }
            };

            let mut receipt = prepared.host_state;
            receipt.created_ovs = ovs_outcome == CreateOutcome::Created;
            Ok(BackendAddOutcome {
                state: BackendState::from(receipt),
                device: prepared.dan_device,
            })
        })
    }

    fn check<'a>(
        &'a self,
        identity: &'a AttachmentIdentity,
        device: &'a DanDevice,
    ) -> BackendFuture<'a, ()> {
        Box::pin(async move {
            let resource = ResourceIdentity::try_from(identity)?;
            let DanEndpoint::HostTap { tap_name } = &device.endpoint;
            if *tap_name != resource.tap_name || device.guest_if_name != identity.if_name {
                return Err(BackendError::InvalidInput(
                    "DAN device does not belong to this attachment".to_string(),
                ));
            }
            tap::check(&TapSpec {
                tap_name: resource.tap_name.clone(),
                owner_id: resource.owner_id.clone(),
                mtu: device.allocation.mtu,
            })
            .await?;
            ovs_operation(move || {
                OvsBinding::default().check_owned(
                    OVS_BRIDGE_NAME,
                    &resource.tap_name,
                    &resource.owner_id,
                    "",
                    &BTreeMap::new(),
                    None,
                )
            })
            .await
        })
    }

    fn del<'a>(&'a self, identity: &'a AttachmentIdentity) -> BackendFuture<'a, ()> {
        Box::pin(async move {
            let resource = ResourceIdentity::try_from(identity)?;
            let ovs_resource = resource.clone();
            ovs_operation(move || {
                OvsBinding::default().delete_owned(&ovs_resource.tap_name, &ovs_resource.owner_id)
            })
            .await?;
            tap::delete(&resource.tap_name, &resource.owner_id).await
        })
    }

    fn rollback_add<'a>(&'a self, state: &'a BackendState) -> BackendFuture<'a, ()> {
        Box::pin(async move {
            let receipt = HostTapState::try_from(state)?;
            if receipt.created_ovs {
                let ovs_receipt = receipt.clone();
                ovs_operation(move || {
                    OvsBinding::default().delete_owned(&ovs_receipt.tap_name, &ovs_receipt.owner_id)
                })
                .await?;
            }
            tap::delete(&receipt.tap_name, &receipt.owner_id).await
        })
    }
}

#[cfg(test)]
mod tests {
    use super::HostTapBackend;
    use dan_core::{
        AttachmentIdentity, Backend, BackendError, DanEndpoint, HostPortBinding, IpAssignment,
        LogicalPort, MacAddress, NetworkAllocation, NetworkContext, NetworkSelection,
    };

    fn context() -> NetworkContext {
        NetworkContext {
            container_id: "sandbox".into(),
            selection: NetworkSelection {
                network_name: "ovn".into(),
                if_name: "eth0".into(),
            },
            allocation: NetworkAllocation {
                mac_address: MacAddress([0x02, 0, 0, 0, 0, 1]),
                addresses: vec![
                    IpAssignment {
                        address: "10.0.0.2".parse().expect("IP"),
                        prefix_length: 24,
                    },
                    IpAssignment {
                        address: "fd00::2".parse().expect("IP"),
                        prefix_length: 64,
                    },
                ],
                routes: vec![],
                mtu: 1500,
            },
            logical_port: LogicalPort {
                name: "pod.namespace.ovn".into(),
                host_binding: HostPortBinding::default(),
            },
        }
    }

    #[test]
    fn prepare_dual_stack() {
        let backend = HostTapBackend::new();
        let original = context();
        let device = backend.prepare(&original).expect("prepare");
        assert_eq!(device.allocation, original.allocation);
        assert_eq!(device.guest_if_name, "eth0");
        backend
            .capabilities()
            .validate(&device.allocation)
            .expect("dual stack capability");
        let DanEndpoint::HostTap { tap_name } = &device.endpoint;
        assert!(tap_name.len() <= 15);
        assert!(tap_name.bytes().all(|byte| byte.is_ascii_alphanumeric()));
        let mut reallocated = original.clone();
        reallocated.allocation.mtu = 1450;
        reallocated.logical_port.name = "replacement-logical-port".into();
        assert_eq!(
            backend.prepare(&reallocated).expect("reprepare").endpoint,
            device.endpoint
        );
        for change_container in [false, true] {
            let mut another = original.clone();
            if change_container {
                another.container_id.push('2');
            } else {
                another.selection.if_name = "eth1".into();
            }
            assert_ne!(
                backend
                    .prepare(&another)
                    .expect("another attachment")
                    .endpoint,
                device.endpoint
            );
        }
    }

    #[tokio::test]
    async fn reject_foreign_device() {
        let backend = HostTapBackend::new();
        let identity = AttachmentIdentity {
            container_id: "sandbox".into(),
            if_name: "eth0".into(),
        };
        let device = backend.prepare(&context()).expect("device");
        for other_endpoint in [false, true] {
            let mut invalid = device.clone();
            if other_endpoint {
                invalid.endpoint = DanEndpoint::HostTap {
                    tap_name: "foreign-tap".into(),
                };
            } else {
                invalid.guest_if_name = "eth1".into();
            }
            assert!(matches!(
                backend.check(&identity, &invalid).await,
                Err(BackendError::InvalidInput(_))
            ));
        }
    }
}
