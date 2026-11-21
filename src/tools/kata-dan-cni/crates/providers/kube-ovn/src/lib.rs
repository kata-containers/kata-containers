// Copyright (c) 2026 Kata Containers community
// SPDX-License-Identifier: Apache-2.0

use std::collections::BTreeMap;
use std::fs;
use std::net::IpAddr;
use std::time::Duration;

use dan_core::{
    HostPortBinding, HostPortConfig, LogicalPort, NetworkAllocation, NetworkSelection, PodIdentity,
    Provider, ProviderError, ProviderFuture, Route,
};
use k8s_openapi::api::core::v1::{Node, Pod};
use kube::core::DynamicObject;
use kube::{Api, Client};
use tokio::time::sleep;

mod annotations;
mod resources;

const POD_READY_ATTEMPTS: usize = 20;
const POD_READY_INTERVAL: Duration = Duration::from_secs(1);
const OVN0_MTU_PATH: &str = "/sys/class/net/ovn0/mtu";
const NODE_NETWORKS_ANNOTATION: &str = "ovn.kubernetes.io/node_networks";

/// Read-only resolver for a Kube-OVN OVN-backed Pod attachment.
#[derive(Clone)]
pub struct KubeOvnProvider {
    client: Client,
    node_name: String,
    provider: String,
    config_routes: Vec<Route>,
    initial_pod: Option<Pod>,
}

impl KubeOvnProvider {
    pub fn new(client: Client, node_name: String) -> Self {
        Self {
            client,
            node_name,
            provider: "ovn".into(),
            config_routes: Vec::new(),
            initial_pod: None,
        }
    }

    /// Select the OVN network named by the delegated Kube-OVN configuration.
    /// Other providers may only supply IPAM and have no OVN logical port.
    pub fn with_provider(mut self, provider: &str) -> Result<Self, ProviderError> {
        let provider = if provider.is_empty() { "ovn" } else { provider };
        let parts: Vec<_> = provider.split('.').collect();
        if provider != "ovn"
            && !(parts.len() == 3
                && parts[2] == "ovn"
                && valid_provider_label(parts[0])
                && valid_provider_label(parts[1]))
        {
            return Err(ProviderError::InvalidData(format!(
                "DAN requires an OVN-backed Kube-OVN provider: `ovn` or `<network>.<namespace>.ovn`; got `{provider}`"
            )));
        }
        self.provider = provider.to_string();
        Ok(self)
    }

    /// Reuse an already fetched Pod for the first readiness check.
    pub fn with_pod(mut self, pod: Pod) -> Self {
        self.initial_pod = Some(pod);
        self
    }

    /// Include static routes from the Kube-OVN CNI configuration.
    /// Pod annotation routes take precedence for the same destination.
    pub fn with_config_routes(
        mut self,
        routes: Option<serde_json::Value>,
    ) -> Result<Self, ProviderError> {
        self.config_routes = annotations::parse_config_routes(routes)?;
        Ok(self)
    }

    async fn resolve_inner(
        &self,
        identity: &PodIdentity,
        selection: &NetworkSelection,
    ) -> Result<(NetworkAllocation, LogicalPort, HostPortConfig), ProviderError> {
        let (pod_network, mut port_config) =
            self.wait_for_ready_network(identity, selection).await?;
        port_config.pod_name.clone_from(&identity.name);
        port_config.pod_namespace.clone_from(&identity.namespace);
        let subnet = self.get_subnet(&pod_network.logical_switch).await?;

        let effective_gateways = effective_gateways(&pod_network, &subnet)?;
        let (mtu, host_binding) = self.resolve_host_config(&pod_network, &subnet).await?;
        let routes =
            annotations::build_routes(&pod_network, &effective_gateways, &self.config_routes);
        Ok((
            NetworkAllocation {
                mac_address: pod_network.mac_address,
                addresses: pod_network.addresses,
                routes,
                mtu,
            },
            LogicalPort {
                name: self.logical_port_name(identity, selection, pod_network.append_if_name),
                host_binding,
            },
            port_config,
        ))
    }

    async fn wait_for_ready_network(
        &self,
        identity: &PodIdentity,
        selection: &NetworkSelection,
    ) -> Result<(annotations::PodNetworkData, HostPortConfig), ProviderError> {
        let pods: Api<Pod> = Api::namespaced(self.client.clone(), &identity.namespace);
        let mut initial_pod = self.initial_pod.clone();
        let mut attempts_left = POD_READY_ATTEMPTS;
        loop {
            let pod = match initial_pod.take() {
                Some(pod) => pod,
                None => pods.get(&identity.name).await.map_err(|error| {
                    ProviderError::RequestFailed(format!(
                        "failed to read Pod `{}/{}`: {error}",
                        identity.namespace, identity.name
                    ))
                })?,
            };

            let readiness = self.validate_pod(&pod, identity).and_then(|()| {
                annotations::ensure_ready(
                    pod.metadata.annotations.as_ref(),
                    &self.provider,
                    &selection.if_name,
                )
            });
            match readiness {
                Ok(annotations) => {
                    return Ok((
                        annotations::parse(annotations, &self.provider, &selection.if_name)?,
                        annotations::parse_port_config(
                            annotations,
                            &self.provider,
                            &selection.if_name,
                        )?,
                    ));
                }
                Err(ProviderError::NotReady(_)) if attempts_left > 1 => {
                    attempts_left -= 1;
                    sleep(POD_READY_INTERVAL).await;
                }
                Err(error) => return Err(error),
            }
        }
    }

    fn logical_port_name(
        &self,
        identity: &PodIdentity,
        selection: &NetworkSelection,
        append_if_name: bool,
    ) -> String {
        let mut name = format!("{}.{}", identity.name, identity.namespace);
        if self.provider != "ovn" {
            name.push('.');
            name.push_str(&self.provider);
        }
        if append_if_name {
            name.push('.');
            name.push_str(&selection.if_name);
        }
        name
    }

    fn validate_pod(&self, pod: &Pod, identity: &PodIdentity) -> Result<(), ProviderError> {
        if pod.metadata.uid.as_deref() != Some(identity.uid.as_str()) {
            return Err(ProviderError::InvalidData(format!(
                "Pod UID does not match `{}`",
                identity.uid,
            )));
        }

        if pod.metadata.deletion_timestamp.is_some() {
            return Err(ProviderError::InvalidData(format!(
                "Pod `{}/{}` is being deleted",
                identity.namespace, identity.name
            )));
        }

        let pod_node_name = pod
            .spec
            .as_ref()
            .and_then(|spec| spec.node_name.as_deref())
            .ok_or_else(|| {
                ProviderError::NotReady(format!(
                    "Pod `{}/{}` has not been scheduled",
                    identity.namespace, identity.name
                ))
            })?;
        if pod_node_name != self.node_name {
            return Err(ProviderError::InvalidData(format!(
                "Pod `{}/{}` is scheduled on node `{pod_node_name}`, not `{}`",
                identity.namespace, identity.name, self.node_name
            )));
        }

        Ok(())
    }

    async fn get_subnet(&self, name: &str) -> Result<resources::SubnetData, ProviderError> {
        let api_resource = resources::api_resource();
        let subnets: Api<DynamicObject> = Api::all_with(self.client.clone(), &api_resource);
        let subnet = subnets.get(name).await.map_err(|error| {
            ProviderError::RequestFailed(format!(
                "failed to read Kube-OVN Subnet `{name}`: {error}"
            ))
        })?;
        resources::parse(subnet)
    }

    async fn get_node(&self) -> Result<Node, ProviderError> {
        let nodes: Api<Node> = Api::all(self.client.clone());
        nodes.get(&self.node_name).await.map_err(|error| {
            ProviderError::RequestFailed(format!(
                "failed to read Kubernetes Node `{}`: {error}",
                self.node_name
            ))
        })
    }

    async fn resolve_host_config(
        &self,
        pod_network: &annotations::PodNetworkData,
        subnet: &resources::SubnetData,
    ) -> Result<(u32, HostPortBinding), ProviderError> {
        let provider_network = if subnet.spec.mtu == 0
            && !subnet.spec.logical_gateway
            && !subnet.spec.u2o_interconnection
        {
            pod_network.provider_network.as_deref()
        } else {
            None
        };
        let mut mtu = subnet.spec.mtu;
        let mut binding = HostPortBinding::default();
        if provider_network.is_some() || !subnet.spec.node_network.is_empty() {
            let node = self.get_node().await?;
            if let Some(provider_network) = provider_network {
                mtu = provider_network_mtu(&node, provider_network)?;
            }
            if !subnet.spec.node_network.is_empty() {
                binding.encapsulation_ip = Some(node_network_encapsulation_ip(
                    &node,
                    &subnet.spec.node_network,
                )?);
            }
        }
        if mtu == 0 {
            mtu = read_ovn0_mtu()?;
        }
        Ok((mtu, binding))
    }
}

fn valid_provider_label(label: &str) -> bool {
    !label.is_empty()
        && label.len() <= 63
        && label.starts_with(|character: char| {
            character.is_ascii_lowercase() || character.is_ascii_digit()
        })
        && label.ends_with(|character: char| {
            character.is_ascii_lowercase() || character.is_ascii_digit()
        })
        && label
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

fn provider_network_mtu(node: &Node, provider_network: &str) -> Result<u32, ProviderError> {
    let label = format!("{provider_network}.provider-network.kubernetes.io/mtu");
    let value = node
        .metadata
        .labels
        .as_ref()
        .and_then(|labels| labels.get(&label))
        .ok_or_else(|| ProviderError::NotReady(format!("Node is missing MTU label `{label}`")))?;
    parse_positive_mtu(value, &format!("Node label `{label}`"))
}

fn node_network_encapsulation_ip(node: &Node, network_name: &str) -> Result<IpAddr, ProviderError> {
    let node_name = node.metadata.name.as_deref().unwrap_or("<unknown>");
    let annotation = node
        .metadata
        .annotations
        .as_ref()
        .and_then(|annotations| annotations.get(NODE_NETWORKS_ANNOTATION))
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            ProviderError::NotReady(format!(
                "Node `{node_name}` does not have annotation `{NODE_NETWORKS_ANNOTATION}` required by node network `{network_name}`"
            ))
        })?;
    let networks =
        serde_json::from_str::<BTreeMap<String, String>>(annotation).map_err(|error| {
            ProviderError::InvalidData(format!(
                "invalid Node `{node_name}` annotation `{NODE_NETWORKS_ANNOTATION}`: {error}"
            ))
        })?;

    let address = networks.get(network_name).ok_or_else(|| {
        ProviderError::NotReady(format!(
            "Node `{node_name}` annotation `{NODE_NETWORKS_ANNOTATION}` does not contain node network `{network_name}`"
        ))
    })?;
    address.parse().map_err(|error| {
        ProviderError::InvalidData(format!(
            "invalid encapsulation IP for node network `{network_name}`: {error}"
        ))
    })
}

impl Provider for KubeOvnProvider {
    fn resolve<'a>(
        &'a self,
        identity: &'a PodIdentity,
        selection: &'a NetworkSelection,
    ) -> ProviderFuture<'a> {
        Box::pin(async move {
            self.resolve_inner(identity, selection)
                .await
                .map(|(allocation, port, _)| (allocation, port))
        })
    }

    fn resolve_with_port_config<'a>(
        &'a self,
        identity: &'a PodIdentity,
        selection: &'a NetworkSelection,
    ) -> ProviderFuture<'a, (NetworkAllocation, LogicalPort, HostPortConfig)> {
        Box::pin(async move { self.resolve_inner(identity, selection).await })
    }

    fn verify_port_config<'a>(
        &'a self,
        identity: &'a PodIdentity,
        selection: &'a NetworkSelection,
        expected_port: &'a LogicalPort,
        expected: &'a HostPortConfig,
    ) -> ProviderFuture<'a, ()> {
        Box::pin(async move {
            let pods: Api<Pod> = Api::namespaced(self.client.clone(), &identity.namespace);
            let pod = pods.get(&identity.name).await.map_err(|error| {
                ProviderError::RequestFailed(format!(
                    "failed to verify Pod `{}/{}` host port settings: {error}",
                    identity.namespace, identity.name
                ))
            })?;
            self.validate_pod(&pod, identity)?;
            let annotations = annotations::ensure_ready(
                pod.metadata.annotations.as_ref(),
                &self.provider,
                &selection.if_name,
            )?;
            let network = annotations::parse(annotations, &self.provider, &selection.if_name)?;
            if self.logical_port_name(identity, selection, network.append_if_name)
                != expected_port.name
            {
                return Err(ProviderError::NotReady(
                    "Pod network selection changed while the endpoint was being created".into(),
                ));
            }
            let mut current =
                annotations::parse_port_config(annotations, &self.provider, &selection.if_name)?;
            current.pod_name.clone_from(&identity.name);
            current.pod_namespace.clone_from(&identity.namespace);
            if &current != expected {
                return Err(ProviderError::NotReady(
                    "Pod host port settings changed while the endpoint was being created".into(),
                ));
            }
            Ok(())
        })
    }
}

fn read_ovn0_mtu() -> Result<u32, ProviderError> {
    let value = fs::read_to_string(OVN0_MTU_PATH).map_err(|error| {
        ProviderError::RequestFailed(format!("failed to read `{OVN0_MTU_PATH}`: {error}"))
    })?;
    parse_positive_mtu(&value, OVN0_MTU_PATH)
}

fn effective_gateways(
    pod_network: &annotations::PodNetworkData,
    subnet: &resources::SubnetData,
) -> Result<Vec<IpAddr>, ProviderError> {
    if !pod_network.default_route || !subnet.spec.u2o_interconnection {
        return Ok(pod_network.gateways.clone());
    }

    let gateway = subnet
        .status
        .u2o_interconnection_ip
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            ProviderError::NotReady(format!(
                "Subnet `{}` has U2O enabled but its status does not contain an interconnection IP",
                pod_network.logical_switch
            ))
        })?;
    let available = annotations::parse_address_list(gateway, "Subnet.status.u2oInterconnectionIP")?;
    let mut gateways = Vec::with_capacity(pod_network.addresses.len());
    for assignment in &pod_network.addresses {
        let selected = available
            .iter()
            .find(|gateway| gateway.is_ipv4() == assignment.address.is_ipv4())
            .copied()
            .ok_or_else(|| {
                ProviderError::NotReady(format!(
                    "Subnet `{}` U2O interconnection IP does not contain the Pod's {} family",
                    pod_network.logical_switch,
                    if assignment.address.is_ipv4() {
                        "IPv4"
                    } else {
                        "IPv6"
                    }
                ))
            })?;
        gateways.push(selected);
    }

    Ok(gateways)
}

fn parse_positive_mtu(value: &str, source: &str) -> Result<u32, ProviderError> {
    let mtu = value
        .trim()
        .parse::<u32>()
        .map_err(|error| ProviderError::InvalidData(format!("invalid MTU in {source}: {error}")))?;
    if mtu == 0 {
        return Err(ProviderError::InvalidData(format!(
            "MTU in {source} must be greater than zero"
        )));
    }
    Ok(mtu)
}

#[cfg(test)]
mod tests {
    use std::net::IpAddr;
    use std::time::Duration;

    use crate::KubeOvnProvider;
    use dan_core::{
        HostPortBinding, HostPortConfig, IpAssignment, LogicalPort, MacAddress, NetworkSelection,
        PodIdentity, Provider, ProviderError, Route,
    };
    use http::{Method, Request, Response};
    use k8s_openapi::api::core::v1::Pod;
    use kube::client::Body;
    use kube::Client;
    use serde_json::{json, Value};
    use tokio::task::JoinHandle;
    use tokio::time::timeout;

    const POD_PATH: &str = "/api/v1/namespaces/default/pods/demo";
    const SUBNET_PATH: &str = "/apis/kubeovn.io/v1/subnets/pod-subnet";

    fn identity() -> PodIdentity {
        PodIdentity {
            namespace: "default".into(),
            name: "demo".into(),
            uid: "pod-uid".into(),
        }
    }

    fn selection(if_name: &str) -> NetworkSelection {
        NetworkSelection {
            network_name: "pod-network".into(),
            if_name: if_name.into(),
        }
    }

    fn ip(value: &str) -> IpAddr {
        value.parse().expect("valid IP fixture")
    }

    fn route(destination: &str, prefix_length: u8, gateway: Option<&str>) -> Route {
        Route {
            destination: ip(destination),
            prefix_length,
            gateway: gateway.map(ip),
        }
    }

    fn pod() -> Pod {
        serde_json::from_value(json!({
            "apiVersion": "v1", "kind": "Pod",
            "metadata": {
                "name": "demo", "namespace": "default", "uid": "pod-uid",
                "annotations": {
                    "ovn.kubernetes.io/allocated": "true",
                    "ovn.kubernetes.io/routed": "true",
                    "ovn.kubernetes.io/ip_address": "10.42.0.10",
                    "ovn.kubernetes.io/mac_address": "02:00:00:00:00:0a",
                    "ovn.kubernetes.io/cidr": "10.42.0.0/24",
                    "ovn.kubernetes.io/gateway": "10.42.0.1",
                    "ovn.kubernetes.io/logical_switch": "pod-subnet"
                }
            },
            "spec": {"nodeName": "node-a", "containers": []}
        }))
        .expect("valid Pod fixture")
    }

    fn annotate(pod: &mut Pod, key: &str, value: &str) {
        pod.metadata
            .annotations
            .get_or_insert_default()
            .insert(key.into(), value.into());
    }

    fn subnet(spec: Value) -> Value {
        json!({
            "apiVersion": "kubeovn.io/v1", "kind": "Subnet",
            "metadata": {"name": "pod-subnet"}, "spec": spec
        })
    }

    // Every expected request is a GET to an exact resource. A write, list, watch,
    // unexpected Node lookup, or extra readiness read makes the resolution fail.
    fn mock_api(replies: Vec<(&'static str, Value)>) -> (Client, JoinHandle<()>) {
        let (service, mut handle) = tower_test::mock::pair::<Request<Body>, Response<Body>>();
        let server = tokio::spawn(async move {
            for (path, body) in replies {
                let (request, response) = timeout(Duration::from_secs(3), handle.next_request())
                    .await
                    .expect("Provider issued the expected request")
                    .expect("mock API remains connected");
                assert_eq!(
                    request.method(),
                    Method::GET,
                    "Provider must remain read-only"
                );
                assert_eq!(
                    request.uri().path_and_query().map(|value| value.as_str()),
                    Some(path)
                );
                response.send_response(
                    Response::builder()
                        .status(200)
                        .header("content-type", "application/json")
                        .body(Body::from(serde_json::to_vec(&body).expect("JSON fixture")))
                        .expect("valid HTTP response"),
                );
            }
        });
        (Client::new(service, "default"), server)
    }

    #[tokio::test]
    async fn resolve_dual_stack() {
        let mut pod = pod();
        annotate(
            &mut pod,
            "ovn.kubernetes.io/ip_address",
            "fd00:42::a,10.42.0.10",
        );
        annotate(
            &mut pod,
            "ovn.kubernetes.io/cidr",
            "10.42.0.0/24,fd00:42::/64",
        );
        annotate(
            &mut pod,
            "ovn.kubernetes.io/gateway",
            "fd00:42::1,10.42.0.1",
        );
        let (client, server) = mock_api(vec![
            (POD_PATH, serde_json::to_value(pod).expect("Pod JSON")),
            (SUBNET_PATH, subnet(json!({"mtu": 1400}))),
        ]);
        let (allocation, port, settings) = KubeOvnProvider::new(client, "node-a".into())
            .resolve_with_port_config(&identity(), &selection("eth0"))
            .await
            .expect("ready allocation");
        server.await.expect("read-only API assertions");

        assert_eq!(allocation.mac_address, MacAddress([2, 0, 0, 0, 0, 10]));
        assert_eq!(
            allocation.addresses,
            vec![
                IpAssignment {
                    address: ip("10.42.0.10"),
                    prefix_length: 24
                },
                IpAssignment {
                    address: ip("fd00:42::a"),
                    prefix_length: 64
                },
            ]
        );
        assert_eq!(
            allocation.routes,
            vec![
                route("0.0.0.0", 0, Some("10.42.0.1")),
                route("::", 0, Some("fd00:42::1")),
            ]
        );
        assert_eq!(allocation.mtu, 1400);
        allocation.validate().expect("usable Guest allocation");
        assert_eq!(
            port,
            LogicalPort {
                name: "demo.default".into(),
                host_binding: HostPortBinding::default()
            }
        );
        assert_eq!(
            settings,
            HostPortConfig {
                pod_name: "demo".into(),
                pod_namespace: "default".into(),
                ..HostPortConfig::default()
            }
        );
    }

    #[tokio::test]
    async fn interface_annotations() {
        let mut pod = pod();
        for (field, value) in [
            ("allocated", "true"),
            ("ip_address", "192.0.2.10"),
            ("mac_address", "02:00:00:00:00:0b"),
            ("cidr", "192.0.2.0/24"),
            ("gateway", "192.0.2.1"),
            ("logical_switch", "pod-subnet"),
            ("ingress_rate", "12"),
            ("egress_rate", "34"),
            ("latency", "5"),
            ("jitter", "2"),
            ("limit", "100"),
            ("loss", "1.5"),
            ("default_route", "true"),
            ("mirror", "false"),
            ("routes", "not-json"),
        ] {
            annotate(
                &mut pod,
                &format!("blue.default.ovn.net1.kubernetes.io/{field}"),
                value,
            );
        }
        for (field, value) in [
            ("default_route", "false"),
            ("mirror", "true"),
            ("routes", r#"[{"dst":"198.51.100.42/24","gw":"192.0.2.2"}]"#),
        ] {
            annotate(
                &mut pod,
                &format!("blue.default.ovn.kubernetes.io/{field}"),
                value,
            );
        }
        let (client, server) = mock_api(vec![(SUBNET_PATH, subnet(json!({"mtu": 1500})))]);
        let provider = KubeOvnProvider::new(client, "node-a".into())
            .with_provider("blue.default.ovn")
            .expect("OVN-backed provider")
            .with_pod(pod);
        let (allocation, port, config) = provider
            .resolve_with_port_config(&identity(), &selection("net1"))
            .await
            .expect("interface-scoped allocation");
        server.await.expect("no extra Pod GET for cached readiness");
        assert_eq!(
            allocation.addresses,
            vec![IpAssignment {
                address: ip("192.0.2.10"),
                prefix_length: 24
            }]
        );
        assert_eq!(allocation.mac_address, MacAddress([2, 0, 0, 0, 0, 11]));
        assert_eq!(
            allocation.routes,
            vec![route("198.51.100.0", 24, Some("192.0.2.2"))]
        );
        assert_eq!(port.name, "demo.default.blue.default.ovn.net1");
        assert_eq!(config.ingress_rate_mbps, 12);
        assert_eq!(config.egress_rate_mbps, 34);
        assert_eq!(config.netem.latency_ms, 5);
        assert_eq!(config.netem.jitter_ms, 2);
        assert_eq!(config.netem.limit_packets, 100);
        assert_eq!(config.netem.loss_percent, 1.5);
        assert!(config.mirror);
    }

    #[tokio::test]
    async fn refresh_pending_allocation() {
        let mut pending = pod();
        annotate(&mut pending, "ovn.kubernetes.io/allocated", "false");
        let (client, server) = mock_api(vec![
            (POD_PATH, serde_json::to_value(pod()).expect("Pod JSON")),
            (SUBNET_PATH, subnet(json!({"mtu":1500}))),
        ]);
        let result = KubeOvnProvider::new(client, "node-a".into())
            .with_pod(pending)
            .resolve(&identity(), &selection("eth0"))
            .await;
        server.await.expect("readiness refetched once");
        assert!(result.is_ok(), "allocation became ready: {result:?}");
    }

    #[tokio::test]
    async fn reject_invalid_pod() {
        for (field, value, context) in [
            ("uid", "stale-uid", "UID"),
            ("node", "node-b", "node-b"),
            ("deleting", "2026-10-15T00:00:00Z", "being deleted"),
        ] {
            let mut pod = pod();
            match field {
                "uid" => pod.metadata.uid = Some(value.into()),
                "node" => pod.spec.as_mut().expect("Pod spec").node_name = Some(value.into()),
                _ => {
                    pod.metadata.deletion_timestamp =
                        Some(serde_json::from_value(json!(value)).expect("timestamp"))
                }
            }
            let (client, server) = mock_api(vec![]);
            let error = KubeOvnProvider::new(client, "node-a".into())
                .with_pod(pod)
                .resolve(&identity(), &selection("eth0"))
                .await
                .expect_err("unsafe Pod identity");
            server.await.expect("no API calls");
            assert!(
                matches!(error, ProviderError::InvalidData(message) if message.contains(context))
            );
        }
    }

    #[tokio::test]
    async fn verify_current_port_config() {
        for change in ["unchanged", "policy", "attachment"] {
            let original = pod();
            let mut current = original.clone();
            match change {
                "policy" => annotate(&mut current, "ovn.kubernetes.io/ingress_rate", "100"),
                "attachment" => {
                    for field in [
                        "allocated",
                        "ip_address",
                        "mac_address",
                        "cidr",
                        "gateway",
                        "logical_switch",
                    ] {
                        let value = current
                            .metadata
                            .annotations
                            .as_ref()
                            .expect("annotations")
                            .get(&format!("ovn.kubernetes.io/{field}"))
                            .expect("base field")
                            .clone();
                        annotate(
                            &mut current,
                            &format!("ovn.eth0.kubernetes.io/{field}"),
                            &value,
                        );
                    }
                }
                _ => {}
            }
            let (client, server) = mock_api(vec![
                (SUBNET_PATH, subnet(json!({"mtu":1500}))),
                (POD_PATH, serde_json::to_value(current).expect("Pod JSON")),
            ]);
            let provider = KubeOvnProvider::new(client, "node-a".into()).with_pod(original);
            let (_, port, config) = provider
                .resolve_with_port_config(&identity(), &selection("eth0"))
                .await
                .expect("initial allocation");
            let result = provider
                .verify_port_config(&identity(), &selection("eth0"), &port, &config)
                .await;
            server.await.expect("verification performs a fresh Pod GET");
            if change == "unchanged" {
                assert!(result.is_ok());
            } else {
                assert!(
                    matches!(result, Err(ProviderError::NotReady(_))),
                    "{change}: {result:?}"
                );
            }
        }
    }
}
