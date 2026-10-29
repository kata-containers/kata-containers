// Copyright (c) 2026 Kata Containers community
// SPDX-License-Identifier: Apache-2.0

use std::collections::BTreeSet;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::net::IpAddr;
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::PathBuf;

use cni_protocol::{CniError, CniErrorCode};
use dan_core::{
    AttachmentIdentity, DanDevice, DanEndpoint, IpAssignment, MacAddress, NetworkAllocation, Route,
};
use serde::Deserialize;
use serde_json::{json, Value};

/// Requires a validated CNI request version.
pub(crate) fn encode_device(device: &DanDevice, cni_version: &str) -> (Value, Value) {
    let (endpoint, interfaces, network_type) = match &device.endpoint {
        DanEndpoint::HostTap { tap_name } => (
            json!({"type": "host-tap", "tap_name": tap_name, "queue_num": 1}),
            json!([{"name": tap_name}]),
            "tuntap",
        ),
    };
    let allocation = &device.allocation;
    let mac = allocation
        .mac_address
        .0
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<Vec<_>>()
        .join(":");
    let addresses = allocation
        .addresses
        .iter()
        .map(|ip| format!("{}/{}", ip.address, ip.prefix_length))
        .collect::<Vec<_>>();
    let dan_routes = allocation
        .routes
        .iter()
        .map(|route| {
            json!({
                "dest": format!("{}/{}", route.destination, route.prefix_length),
                "gateway": route.gateway.map(|gw| gw.to_string()).unwrap_or_default()
            })
        })
        .collect::<Vec<_>>();
    let dan = json!({
        "netns": null,
        "devices": [{
            "name": device.guest_if_name, "guest_mac": mac,
            "device": endpoint,
            "network_info": {
                "interface": {"ip_addresses": addresses, "mtu": allocation.mtu, "ntype": network_type, "flags": 0},
                "routes": dan_routes, "neighbors": []
            }
        }]
    });
    let legacy = matches!(cni_version, "0.1.0" | "0.2.0");
    let ips = allocation
        .addresses
        .iter()
        .zip(&addresses)
        .map(|(assignment, address)| {
            let mut ip = if legacy {
                json!({"ip": address})
            } else {
                json!({"address": address})
            };
            if !legacy && cni_version != "1.0.0" {
                ip["version"] = json!(if assignment.address.is_ipv4() {
                    "4"
                } else {
                    "6"
                });
            }
            if let Some(gw) = allocation
                .routes
                .iter()
                .find(|route| {
                    route.prefix_length == 0
                        && route.destination.is_ipv4() == assignment.address.is_ipv4()
                })
                .and_then(|route| route.gateway)
            {
                ip["gateway"] = json!(gw.to_string());
            }
            ip
        })
        .collect::<Vec<_>>();
    let routes = allocation
        .routes
        .iter()
        .map(|route| {
            let mut value =
                json!({"dst": format!("{}/{}", route.destination, route.prefix_length)});
            if let Some(gw) = route.gateway {
                value["gw"] = json!(gw.to_string());
            }
            value
        })
        .collect::<Vec<_>>();
    let result = if legacy {
        let mut result = json!({"cniVersion": cni_version});
        for (assignment, mut ip) in allocation.addresses.iter().zip(ips) {
            let key = if assignment.address.is_ipv4() {
                "ip4"
            } else {
                "ip6"
            };
            // CNI 0.1/0.2 support one address per family.
            if result.get(key).is_some() {
                continue;
            }
            let family_routes = allocation
                .routes
                .iter()
                .zip(&routes)
                .filter(|(route, _)| route.destination.is_ipv4() == assignment.address.is_ipv4())
                .map(|(_, route)| route.clone())
                .collect::<Vec<_>>();
            if !family_routes.is_empty() {
                ip["routes"] = json!(family_routes);
            }
            result[key] = ip;
        }
        result
    } else {
        // Guest addresses have no host interface index: the interface is created in the VM.
        json!({
            "cniVersion": cni_version, "interfaces": interfaces, "ips": ips, "routes": routes
        })
    };
    (dan, result)
}

#[derive(Deserialize)]
struct RuntimeConfig {
    devices: Vec<RuntimeDevice>,
}
#[derive(Deserialize)]
struct RuntimeDevice {
    name: String,
    guest_mac: String,
    device: RuntimeEndpoint,
    network_info: RuntimeNetwork,
}
#[derive(Deserialize)]
#[serde(tag = "type")]
enum RuntimeEndpoint {
    #[serde(rename = "host-tap")]
    HostTap { tap_name: String },
}
#[derive(Deserialize)]
struct RuntimeNetwork {
    interface: RuntimeInterface,
    #[serde(default)]
    routes: Vec<RuntimeRoute>,
}
#[derive(Deserialize)]
struct RuntimeInterface {
    ip_addresses: Vec<String>,
    mtu: u32,
}
#[derive(Deserialize)]
struct RuntimeRoute {
    dest: String,
    #[serde(default)]
    gateway: String,
}

pub(crate) struct DanFile {
    directory: PathBuf,
}

impl DanFile {
    pub(crate) fn new(directory: PathBuf) -> Self {
        Self { directory }
    }
    fn path(&self, sandbox_id: &str) -> PathBuf {
        self.directory.join(format!("{sandbox_id}.json"))
    }

    pub(crate) fn read(&self, attachment: &AttachmentIdentity) -> Result<DanDevice, CniError> {
        let value = self
            .read_value(&attachment.container_id)?
            .ok_or_else(|| invalid_dan("DAN file is missing"))?;
        decode_devices(&value)?
            .into_iter()
            .find(|device| device.guest_if_name == attachment.if_name)
            .ok_or_else(|| invalid_dan("DAN file has no device for this interface"))
    }

    fn read_value(&self, sandbox_id: &str) -> Result<Option<Value>, CniError> {
        match fs::read(self.path(sandbox_id)) {
            Ok(bytes) => serde_json::from_slice(&bytes)
                .map(Some)
                .map_err(|error| invalid_dan(error.to_string())),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(io_error(error)),
        }
    }

    /// The CNI runtime serializes all attachments for the same sandbox.
    pub(crate) fn publish(&self, sandbox_id: &str, value: &Value) -> Result<(), CniError> {
        decode_devices(value)?;
        let existing = self.read_value(sandbox_id)?;
        let mut merged = match &existing {
            Some(existing) => {
                decode_devices(existing)?;
                existing.clone()
            }
            None => value.clone(),
        };
        let incoming = value["devices"]
            .as_array()
            .ok_or_else(|| invalid_dan("DAN devices must be an array"))?;
        let devices = merged["devices"]
            .as_array_mut()
            .ok_or_else(|| invalid_dan("DAN devices must be an array"))?;
        for device in incoming {
            match devices.iter().find(|saved| saved["name"] == device["name"]) {
                Some(saved) if saved != device => {
                    return Err(invalid_dan(
                        "a different DAN device already exists for this interface",
                    ));
                }
                Some(_) => {}
                None => devices.push(device.clone()),
            }
        }
        // Keep the primary interface first and make the rest independent of ADD order.
        // Kata uses each device's name and MAC for Guest configuration.
        devices.sort_by(|left, right| {
            let left = left["name"].as_str().unwrap_or_default();
            let right = right["name"].as_str().unwrap_or_default();
            (left != "eth0", left).cmp(&(right != "eth0", right))
        });
        if existing.as_ref() == Some(&merged) {
            return Ok(());
        }
        self.replace(sandbox_id, &merged)
    }

    fn replace(&self, sandbox_id: &str, value: &Value) -> Result<(), CniError> {
        fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(&self.directory)
            .map_err(io_error)?;
        let temporary = self
            .directory
            .join(format!(".{sandbox_id}.{}.tmp", std::process::id()));
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&temporary)
            .map_err(io_error)?;
        let result = (|| {
            serde_json::to_writer(&mut file, value)
                .map_err(|error| invalid_dan(error.to_string()))?;
            file.flush().map_err(io_error)?;
            fs::rename(&temporary, self.path(sandbox_id)).map_err(io_error)
        })();
        let _ = fs::remove_file(temporary);
        result
    }

    pub(crate) fn remove(&self, attachment: &AttachmentIdentity) -> Result<(), CniError> {
        let Some(mut value) = self.read_value(&attachment.container_id)? else {
            return Ok(());
        };
        decode_devices(&value)?;
        let devices = value["devices"]
            .as_array_mut()
            .ok_or_else(|| invalid_dan("DAN devices must be an array"))?;
        let length = devices.len();
        devices.retain(|device| device["name"].as_str() != Some(&attachment.if_name));
        if devices.len() == length {
            return Ok(());
        }
        if !devices.is_empty() {
            return self.replace(&attachment.container_id, &value);
        }
        match fs::remove_file(self.path(&attachment.container_id)) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(io_error(error)),
        }
    }
}

fn decode_devices(value: &Value) -> Result<Vec<DanDevice>, CniError> {
    let RuntimeConfig { devices } =
        serde_json::from_value(value.clone()).map_err(|error| invalid_dan(error.to_string()))?;
    if devices.is_empty() {
        return Err(invalid_dan("DAN file has no devices"));
    }
    let mut names = BTreeSet::new();
    devices
        .into_iter()
        .map(|device| {
            if device.name.is_empty() || !names.insert(device.name.clone()) {
                return Err(invalid_dan("DAN device names must be nonempty and unique"));
            }
            device.into_device()
        })
        .collect()
}

impl RuntimeDevice {
    fn into_device(self) -> Result<DanDevice, CniError> {
        let device = self;
        let endpoint = match device.device {
            RuntimeEndpoint::HostTap { tap_name } => DanEndpoint::HostTap { tap_name },
        };
        let mac = device
            .guest_mac
            .split(':')
            .map(|byte| u8::from_str_radix(byte, 16))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| invalid_dan(error.to_string()))?;
        let addresses = device
            .network_info
            .interface
            .ip_addresses
            .iter()
            .map(|value| {
                let (address, prefix_length) = parse_cidr(value)?;
                Ok(IpAssignment {
                    address,
                    prefix_length,
                })
            })
            .collect::<Result<Vec<_>, CniError>>()?;
        let routes = device
            .network_info
            .routes
            .iter()
            .map(|route| {
                let (destination, prefix_length) = parse_cidr(&route.dest)?;
                let gateway = if route.gateway.is_empty() {
                    None
                } else {
                    Some(
                        route
                            .gateway
                            .parse::<IpAddr>()
                            .map_err(|error| invalid_dan(error.to_string()))?,
                    )
                };
                Ok(Route {
                    destination,
                    prefix_length,
                    gateway,
                })
            })
            .collect::<Result<Vec<_>, CniError>>()?;
        let allocation = NetworkAllocation {
            mac_address: MacAddress(
                mac.try_into()
                    .map_err(|_| invalid_dan("MAC address must have six bytes"))?,
            ),
            addresses,
            routes,
            mtu: device.network_info.interface.mtu,
        };
        allocation
            .validate()
            .map_err(|error| invalid_dan(error.to_string()))?;
        Ok(DanDevice {
            guest_if_name: device.name,
            endpoint,
            allocation,
        })
    }
}

fn parse_cidr(value: &str) -> Result<(IpAddr, u8), CniError> {
    let (address, prefix) = value
        .split_once('/')
        .ok_or_else(|| invalid_dan("IP address requires a prefix"))?;
    let address = address
        .parse::<IpAddr>()
        .map_err(|error| invalid_dan(error.to_string()))?;
    let prefix = prefix
        .parse::<u8>()
        .map_err(|error| invalid_dan(error.to_string()))?;
    Ok((address, prefix))
}

fn invalid_dan(details: impl Into<String>) -> CniError {
    CniError::new(
        CniErrorCode::InvalidNetworkConfig,
        "invalid DAN configuration",
        details,
    )
}

fn io_error(error: std::io::Error) -> CniError {
    CniError::new(
        CniErrorCode::IoFailure,
        "DAN file operation failed",
        error.to_string(),
    )
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;
    use crate::test_support::{attachment, device, TestDir};

    #[test]
    fn attachment_lifecycle() {
        let directory = TestDir::new();
        let store = DanFile::new(directory.0.join("dan"));
        for name in ["net1", "eth0", "eth0"] {
            store
                .publish("sandbox-42", &encode_device(&device(name), "1.0.0").0)
                .expect("publish attachment");
        }
        let path = directory.0.join("dan/sandbox-42.json");
        let saved: Value = serde_json::from_slice(&fs::read(&path).expect("DAN file"))
            .expect("valid runtime JSON");
        let names: Vec<_> = saved["devices"]
            .as_array()
            .expect("devices")
            .iter()
            .map(|value| value["name"].as_str().expect("name"))
            .collect();
        assert_eq!(names, ["eth0", "net1"]);
        for name in names {
            assert_eq!(
                store.read(&attachment(name)).expect("round trip"),
                device(name)
            );
        }
        store.remove(&attachment("net1")).expect("delete secondary");
        store.remove(&attachment("net1")).expect("repeat delete");
        assert_eq!(
            store.read(&attachment("eth0")).expect("primary preserved"),
            device("eth0")
        );
        assert!(store.read(&attachment("net1")).is_err());
        store.remove(&attachment("eth0")).expect("delete primary");
        store
            .remove(&attachment("eth0"))
            .expect("missing file delete");
        assert!(!path.exists());
    }

    #[test]
    fn publish_conflict() {
        let directory = TestDir::new();
        let store = DanFile::new(directory.0.clone());
        let (valid, _) = encode_device(&device("eth0"), "1.0.0");
        store
            .publish("sandbox-42", &valid)
            .expect("initial publish");
        let path = directory.0.join("sandbox-42.json");
        let initial = fs::read(&path).expect("original bytes");
        let mut conflict = valid;
        conflict["devices"][0]["device"]["tap_name"] = json!("another-tap");
        assert_eq!(
            store
                .publish("sandbox-42", &conflict)
                .expect_err("reject conflict")
                .code(),
            CniErrorCode::InvalidNetworkConfig.value()
        );
        assert_eq!(fs::read(&path).expect("preserved bytes"), initial);
    }

    #[test]
    fn corrupt_file_preserved() {
        let directory = TestDir::new();
        let store = DanFile::new(directory.0.clone());
        let path = directory.0.join("sandbox-42.json");
        let bytes = b"{incomplete";
        fs::write(&path, bytes).expect("seed corrupt file");
        assert!(store.read(&attachment("eth0")).is_err());
        assert!(store.remove(&attachment("eth0")).is_err());
        assert!(store
            .publish("sandbox-42", &encode_device(&device("eth0"), "1.0.0").0)
            .is_err());
        assert_eq!(fs::read(&path).expect("corrupt file preserved"), bytes);
    }

    #[test]
    fn host_tap_encoding() {
        let device = device("eth0");
        for version in ["0.3.1", "1.0.0"] {
            let (dan, result) = encode_device(&device, version);
            assert_eq!(dan["devices"][0]["guest_mac"], "02:00:00:00:00:42");
            assert_eq!(dan["devices"][0]["device"]["queue_num"], 1);
            assert_eq!(
                dan["devices"][0]["network_info"]["interface"]["ntype"],
                "tuntap"
            );
            assert_eq!(
                dan["devices"][0]["network_info"]["interface"]["ip_addresses"],
                json!(["10.42.0.9/24", "fd42::9/64"])
            );
            assert_eq!(
                dan["devices"][0]["network_info"]["routes"][2]["gateway"],
                ""
            );
            assert_eq!(result["interfaces"], json!([{"name": "tap-eth0"}]));
            assert_eq!(result["ips"][0]["gateway"], "10.42.0.1");
            assert_eq!(result["ips"][1]["gateway"], "fd42::1");
            for ip in result["ips"].as_array().expect("IPs") {
                assert!(ip.get("interface").is_none());
                assert_eq!(ip.get("version").is_some(), version != "1.0.0");
            }
        }
    }
}
