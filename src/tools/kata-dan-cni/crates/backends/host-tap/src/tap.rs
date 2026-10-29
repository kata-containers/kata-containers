// Copyright (c) 2026 Kata Containers community
// SPDX-License-Identifier: Apache-2.0

use dan_core::BackendError;
use futures_util::TryStreamExt;
use rtnetlink::packet_core::{Nla, NLA_TYPE_MASK};
use rtnetlink::packet_route::link::{
    InfoData, InfoKind, InfoTun, LinkAttribute, LinkFlags, LinkInfo, LinkLayerType, LinkMessage,
};
use rtnetlink::{Handle, LinkUnspec};
use tappers::{linux::Tap, Interface};

const OWNER_ALIAS_PREFIX: &str = "kata-dan:";
const IFLA_TUN_TYPE: u16 = 3;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct TapSpec {
    pub tap_name: String,
    pub owner_id: String,
    pub mtu: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct TapInfo {
    index: u32,
    name: String,
    owner_alias: Option<String>,
    mtu: u32,
    admin_up: bool,
    is_tap: bool,
}

impl TapInfo {
    fn from_message(message: LinkMessage) -> Result<Self, BackendError> {
        let mut name = None;
        let mut owner_alias = None;
        let mut mtu = None;
        let mut is_tuntap = false;
        let mut tun_type = None;

        for attribute in &message.attributes {
            match attribute {
                LinkAttribute::IfName(value) => name = Some(value.clone()),
                LinkAttribute::IfAlias(value) => owner_alias = Some(value.clone()),
                LinkAttribute::Mtu(value) => mtu = Some(*value),
                LinkAttribute::LinkInfo(infos) => {
                    for info in infos {
                        match info {
                            LinkInfo::Kind(InfoKind::Tun) => is_tuntap = true,
                            LinkInfo::Data(InfoData::Tun(attributes)) => {
                                for attribute in attributes {
                                    let InfoTun::Other(raw) = attribute else {
                                        continue;
                                    };
                                    if raw.kind() & NLA_TYPE_MASK == IFLA_TUN_TYPE {
                                        tun_type = nla_u8(raw);
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                }
                _ => {}
            }
        }

        let name = name.ok_or_else(|| {
            BackendError::OperationFailed(
                "kernel link response did not contain an interface name".to_string(),
            )
        })?;
        let mtu = mtu.ok_or_else(|| {
            BackendError::OperationFailed(format!(
                "kernel link response for `{name}` did not contain an MTU"
            ))
        })?;

        Ok(Self {
            index: message.header.index,
            name,
            owner_alias,
            mtu,
            admin_up: message.header.flags.contains(LinkFlags::Up),
            is_tap: is_tuntap
                && message.header.link_layer_type == LinkLayerType::Ether
                && tun_type == Some(libc::IFF_TAP as u8),
        })
    }

    fn validate_owned(&self, spec: &TapSpec) -> Result<(), BackendError> {
        if self.name != spec.tap_name {
            return Err(BackendError::OperationFailed(format!(
                "looked up TAP `{}`, but the kernel returned `{}`",
                spec.tap_name, self.name
            )));
        }
        if !self.is_tap {
            return Err(BackendError::OperationFailed(format!(
                "interface `{}` exists but is not a TAP device",
                spec.tap_name
            )));
        }
        let expected_alias = owner_alias(&spec.owner_id);
        if self.owner_alias.as_deref() != Some(expected_alias.as_str()) {
            return Err(BackendError::OperationFailed(format!(
                "TAP `{}` is not owned by this attachment",
                spec.tap_name
            )));
        }

        Ok(())
    }

    fn validate_ready(&self, spec: &TapSpec) -> Result<(), BackendError> {
        self.validate_owned(spec)?;
        if self.mtu != spec.mtu {
            return Err(BackendError::OperationFailed(format!(
                "TAP `{}` has MTU {}, expected {}",
                spec.tap_name, self.mtu, spec.mtu
            )));
        }
        if !self.admin_up {
            return Err(BackendError::OperationFailed(format!(
                "TAP `{}` is administratively down",
                spec.tap_name
            )));
        }
        Ok(())
    }
}

pub(crate) async fn create(spec: &TapSpec) -> Result<(), BackendError> {
    let interface = Interface::new(&spec.tap_name).map_err(|error| {
        BackendError::InvalidInput(format!("invalid TAP name `{}`: {error}", spec.tap_name))
    })?;
    let handle = new_netlink_handle()?;

    let tap = Tap::create_named(interface).map_err(|error| {
        BackendError::OperationFailed(format!("failed to create TAP `{}`: {error}", spec.tap_name))
    })?;

    let created = lookup(&handle, &spec.tap_name).await?.ok_or_else(|| {
        BackendError::OperationFailed(format!(
            "TAP `{}` disappeared immediately after creation",
            spec.tap_name
        ))
    })?;
    configure(&handle, created.index, spec).await?;
    tap.set_persistent(true).map_err(|error| {
        BackendError::OperationFailed(format!(
            "failed to make TAP `{}` persistent: {error}",
            spec.tap_name
        ))
    })?;
    drop(tap);

    Ok(())
}

pub(crate) async fn check(spec: &TapSpec) -> Result<(), BackendError> {
    let handle = new_netlink_handle()?;
    check_with_handle(&handle, spec).await
}

pub(crate) async fn delete(tap_name: &str, owner_id: &str) -> Result<(), BackendError> {
    let handle = new_netlink_handle()?;
    delete_with_handle(&handle, tap_name, owner_id).await
}

async fn check_with_handle(handle: &Handle, spec: &TapSpec) -> Result<(), BackendError> {
    let info = lookup(handle, &spec.tap_name).await?.ok_or_else(|| {
        BackendError::OperationFailed(format!("TAP `{}` does not exist", spec.tap_name))
    })?;
    info.validate_ready(spec)
}

async fn delete_with_handle(
    handle: &Handle,
    tap_name: &str,
    owner_id: &str,
) -> Result<(), BackendError> {
    let Some(info) = lookup(handle, tap_name).await? else {
        return Ok(());
    };
    let spec = TapSpec {
        tap_name: tap_name.to_string(),
        owner_id: owner_id.to_string(),
        mtu: info.mtu,
    };
    info.validate_owned(&spec)?;

    match handle.link().del(info.index).execute().await {
        Ok(()) => Ok(()),
        Err(error) if is_no_device(&error) => Ok(()),
        Err(error) => Err(BackendError::OperationFailed(format!(
            "failed to delete TAP `{tap_name}`: {error}"
        ))),
    }
}

async fn configure(handle: &Handle, index: u32, spec: &TapSpec) -> Result<(), BackendError> {
    handle
        .link()
        .change(
            LinkUnspec::new_with_index(index)
                .alias(owner_alias(&spec.owner_id))
                .mtu(spec.mtu)
                .up()
                .build(),
        )
        .execute()
        .await
        .map_err(|error| {
            BackendError::OperationFailed(format!(
                "failed to configure TAP `{}`: {error}",
                spec.tap_name
            ))
        })
}

async fn lookup(handle: &Handle, tap_name: &str) -> Result<Option<TapInfo>, BackendError> {
    let mut links = handle.link().get().match_name(tap_name).execute();
    match links.try_next().await {
        Ok(Some(message)) => TapInfo::from_message(message).map(Some),
        Ok(None) => Ok(None),
        Err(error) if is_no_device(&error) => Ok(None),
        Err(error) => Err(BackendError::OperationFailed(format!(
            "failed to inspect interface `{tap_name}`: {error}"
        ))),
    }
}

fn new_netlink_handle() -> Result<Handle, BackendError> {
    let (connection, handle, _) = rtnetlink::new_connection().map_err(|error| {
        BackendError::OperationFailed(format!("failed to open route netlink connection: {error}"))
    })?;
    let _connection_task = tokio::spawn(connection);
    Ok(handle)
}

fn owner_alias(owner_id: &str) -> String {
    format!("{OWNER_ALIAS_PREFIX}{owner_id}")
}

fn nla_u8(attribute: &impl Nla) -> Option<u8> {
    if attribute.value_len() != 1 {
        return None;
    }
    let mut value = [0_u8; 1];
    attribute.emit_value(&mut value);
    Some(value[0])
}

fn is_no_device(error: &rtnetlink::Error) -> bool {
    matches!(
        error,
        rtnetlink::Error::NetlinkError(message)
            if message.to_io().raw_os_error() == Some(libc::ENODEV)
    )
}

#[cfg(test)]
mod tests {
    //! Kernel response validation without creating or deleting host links.

    use rtnetlink::packet_core::DefaultNla;

    use super::*;

    fn spec() -> TapSpec {
        TapSpec {
            tap_name: "kdt-owned".into(),
            owner_id: "owner".into(),
            mtu: 1450,
        }
    }

    fn kernel_link() -> LinkMessage {
        let mut message = LinkMessage::default();
        message.header.index = 31;
        message.header.flags = LinkFlags::Up;
        message.header.link_layer_type = LinkLayerType::Ether;
        message.attributes = vec![
            LinkAttribute::IfName("kdt-owned".into()),
            LinkAttribute::IfAlias("kata-dan:owner".into()),
            LinkAttribute::Mtu(1450),
            LinkAttribute::LinkInfo(vec![
                LinkInfo::Kind(InfoKind::Tun),
                LinkInfo::Data(InfoData::Tun(vec![InfoTun::Other(DefaultNla::new(
                    IFLA_TUN_TYPE,
                    vec![libc::IFF_TAP as u8],
                ))])),
            ]),
        ];
        message
    }

    #[test]
    fn tap_ownership() {
        let mutations: [fn(&mut TapInfo); 4] = [
            |info| info.name = "another-link".into(),
            |info| info.owner_alias = None,
            |info| info.owner_alias = Some("kata-dan:another-owner".into()),
            |info| info.is_tap = false,
        ];
        for mutate in mutations {
            let mut info = TapInfo::from_message(kernel_link()).expect("kernel response");
            mutate(&mut info);
            assert!(info.validate_owned(&spec()).is_err());
        }
    }

    #[test]
    fn tap_readiness() {
        let ready = TapInfo::from_message(kernel_link()).expect("kernel response");
        ready.validate_ready(&spec()).expect("ready TAP");
        for down in [true, false] {
            let mut info = TapInfo::from_message(kernel_link()).expect("kernel response");
            if down {
                info.admin_up = false;
            } else {
                info.mtu = 9000;
            }
            info.validate_owned(&spec())
                .expect("owned despite configuration drift");
            assert!(info.validate_ready(&spec()).is_err());
        }
    }
}
