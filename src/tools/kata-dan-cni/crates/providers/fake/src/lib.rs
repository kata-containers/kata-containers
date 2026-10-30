// Copyright (c) 2026 Kata Containers community
// SPDX-License-Identifier: Apache-2.0

use dan_core::{
    LogicalPort, NetworkAllocation, NetworkSelection, PodIdentity, Provider, ProviderFuture,
};

/// Returns a predetermined allocation without reading any external system.
#[derive(Clone, Debug)]
pub struct FakeProvider {
    allocation: NetworkAllocation,
    logical_port: LogicalPort,
}

impl FakeProvider {
    pub fn new(allocation: NetworkAllocation, logical_port: LogicalPort) -> Self {
        Self {
            allocation,
            logical_port,
        }
    }
}

impl Provider for FakeProvider {
    fn resolve<'a>(
        &'a self,
        _identity: &'a PodIdentity,
        _selection: &'a NetworkSelection,
    ) -> ProviderFuture<'a> {
        Box::pin(async move { Ok((self.allocation.clone(), self.logical_port.clone())) })
    }
}

#[cfg(test)]
mod tests {
    use crate::FakeProvider;
    use dan_core::{
        HostPortBinding, HostPortConfig, IpAssignment, LogicalPort, MacAddress, NetworkAllocation,
        NetworkSelection, PodIdentity, Provider,
    };

    #[tokio::test]
    async fn resolve_static_allocation() {
        let expected = NetworkAllocation {
            mac_address: MacAddress([2, 0, 0, 0, 0, 1]),
            addresses: vec![IpAssignment {
                address: "192.0.2.10".parse().expect("fixture IP"),
                prefix_length: 24,
            }],
            routes: vec![],
            mtu: 1500,
        };
        let port = LogicalPort {
            name: "external-port-id".into(),
            host_binding: HostPortBinding::default(),
        };
        let provider: Box<dyn Provider> =
            Box::new(FakeProvider::new(expected.clone(), port.clone()));
        let identity = PodIdentity {
            namespace: "any".into(),
            name: "pod".into(),
            uid: "uid".into(),
        };
        let selection = NetworkSelection {
            network_name: "independent-network".into(),
            if_name: "net1".into(),
        };

        let (allocation, resolved_port, policy) = provider
            .resolve_with_port_config(&identity, &selection)
            .await
            .expect("provider-neutral resolution");
        assert_eq!(allocation, expected);
        assert_eq!(resolved_port, port);
        assert_eq!(policy, HostPortConfig::default());
        provider
            .verify_port_config(&identity, &selection, &resolved_port, &policy)
            .await
            .expect("providers without host policy need no external verification");
    }
}
