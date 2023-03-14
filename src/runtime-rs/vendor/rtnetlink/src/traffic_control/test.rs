// SPDX-License-Identifier: MIT

use std::process::Command;

use futures::stream::TryStreamExt;
use tokio::runtime::Runtime;

use crate::{
    new_connection,
    packet::{
        rtnl::tc::nlas::Nla::{Chain, HwOffload, Kind},
        ErrorMessage,
        TcMessage,
        AF_UNSPEC,
    },
    Error::NetlinkError,
};

static TEST_DUMMY_NIC: &str = "netlink-test";

async fn _get_qdiscs() -> Vec<TcMessage> {
    let (connection, handle, _) = new_connection().unwrap();
    tokio::spawn(connection);
    let mut qdiscs_iter = handle.qdisc().get().execute();
    let mut qdiscs = Vec::new();
    while let Some(nl_msg) = qdiscs_iter.try_next().await.unwrap() {
        qdiscs.push(nl_msg.clone());
    }
    qdiscs
}

#[test]
fn test_get_qdiscs() {
    let qdiscs = Runtime::new().unwrap().block_on(_get_qdiscs());
    let qdisc_of_loopback_nic = &qdiscs[0];
    assert_eq!(qdisc_of_loopback_nic.header.family, AF_UNSPEC as u8);
    assert_eq!(qdisc_of_loopback_nic.header.index, 1);
    assert_eq!(qdisc_of_loopback_nic.header.handle, 0);
    assert_eq!(qdisc_of_loopback_nic.header.parent, u32::MAX);
    assert_eq!(qdisc_of_loopback_nic.header.info, 2); // refcount
    assert_eq!(qdisc_of_loopback_nic.nlas[0], Kind("noqueue".to_string()));
    assert_eq!(qdisc_of_loopback_nic.nlas[1], HwOffload(0));
}

async fn _get_tclasses(ifindex: i32) -> Vec<TcMessage> {
    let (connection, handle, _) = new_connection().unwrap();
    tokio::spawn(connection);
    let mut tclasses_iter = handle.traffic_class(ifindex).get().execute();
    let mut tclasses = Vec::new();
    while let Some(nl_msg) = tclasses_iter.try_next().await.unwrap() {
        tclasses.push(nl_msg.clone());
    }
    tclasses
}

// Return 0 for not found
fn _get_test_dummy_interface_index() -> i32 {
    let output = Command::new("ip")
        .args(&["-o", "link", "show", TEST_DUMMY_NIC])
        .output()
        .expect("failed to run ip command");
    if !output.status.success() {
        0
    } else {
        let line = std::str::from_utf8(&output.stdout).unwrap();
        line.split(": ").next().unwrap().parse::<i32>().unwrap()
    }
}

fn _add_test_dummy_interface() -> i32 {
    if _get_test_dummy_interface_index() == 0 {
        let output = Command::new("ip")
            .args(&["link", "add", TEST_DUMMY_NIC, "type", "dummy"])
            .output()
            .expect("failed to run ip command");
        if !output.status.success() {
            eprintln!(
                "Failed to create dummy interface {} : {:?}",
                TEST_DUMMY_NIC, output
            );
        }
        assert!(output.status.success());
    }

    _get_test_dummy_interface_index()
}

fn _remove_test_dummy_interface() {
    let output = Command::new("ip")
        .args(&["link", "del", TEST_DUMMY_NIC])
        .output()
        .expect("failed to run ip command");
    if !output.status.success() {
        eprintln!(
            "Failed to remove dummy interface {} : {:?}",
            TEST_DUMMY_NIC, output
        );
    }
    assert!(output.status.success());
}

fn _add_test_tclass_to_dummy() {
    let output = Command::new("tc")
        .args(&[
            "qdisc",
            "add",
            "dev",
            TEST_DUMMY_NIC,
            "root",
            "handle",
            "1:",
            "htb",
            "default",
            "6",
        ])
        .output()
        .expect("failed to run tc command");
    if !output.status.success() {
        eprintln!(
            "Failed to add qdisc to dummy interface {} : {:?}",
            TEST_DUMMY_NIC, output
        );
    }
    assert!(output.status.success());
    let output = Command::new("tc")
        .args(&[
            "class",
            "add",
            "dev",
            TEST_DUMMY_NIC,
            "parent",
            "1:",
            "classid",
            "1:1",
            "htb",
            "rate",
            "10mbit",
            "ceil",
            "10mbit",
        ])
        .output()
        .expect("failed to run tc command");
    if !output.status.success() {
        eprintln!(
            "Failed to add traffic class to dummy interface {}: {:?}",
            TEST_DUMMY_NIC, output
        );
    }
    assert!(output.status.success());
}

fn _add_test_filter_to_dummy() {
    let output = Command::new("tc")
        .args(&[
            "filter",
            "add",
            "dev",
            TEST_DUMMY_NIC,
            "parent",
            "1:",
            "basic",
            "match",
            "meta(priority eq 6)",
            "classid",
            "1:1",
        ])
        .output()
        .expect("failed to run tc command");
    if !output.status.success() {
        eprintln!("Failed to add trafice filter to lo: {:?}", output);
    }
    assert!(output.status.success());
}

fn _remove_test_tclass_from_dummy() {
    Command::new("tc")
        .args(&[
            "class",
            "del",
            "dev",
            TEST_DUMMY_NIC,
            "parent",
            "1:",
            "classid",
            "1:1",
        ])
        .status()
        .unwrap_or_else(|_| {
            panic!(
                "failed to remove tclass from dummy interface {}",
                TEST_DUMMY_NIC
            )
        });
    Command::new("tc")
        .args(&["qdisc", "del", "dev", TEST_DUMMY_NIC, "root"])
        .status()
        .unwrap_or_else(|_| {
            panic!(
                "failed to remove qdisc from dummy interface {}",
                TEST_DUMMY_NIC
            )
        });
}

fn _remove_test_filter_from_dummy() {
    Command::new("tc")
        .args(&["filter", "del", "dev", TEST_DUMMY_NIC])
        .status()
        .unwrap_or_else(|_| {
            panic!(
                "failed to remove filter from dummy interface {}",
                TEST_DUMMY_NIC
            )
        });
}

async fn _get_filters(ifindex: i32) -> Vec<TcMessage> {
    let (connection, handle, _) = new_connection().unwrap();
    tokio::spawn(connection);
    let mut filters_iter = handle.traffic_filter(ifindex).get().execute();
    let mut filters = Vec::new();
    while let Some(nl_msg) = filters_iter.try_next().await.unwrap() {
        filters.push(nl_msg.clone());
    }
    filters
}

async fn _get_chains(ifindex: i32) -> Vec<TcMessage> {
    let (connection, handle, _) = new_connection().unwrap();
    tokio::spawn(connection);
    let mut chains_iter = handle.traffic_chain(ifindex).get().execute();
    let mut chains = Vec::new();
    // The traffic control chain is only supported by kernel 4.19+,
    // hence we might get error: 95 Operation not supported
    loop {
        match chains_iter.try_next().await {
            Ok(Some(nl_msg)) => {
                chains.push(nl_msg.clone());
            }
            Ok(None) => {
                break;
            }
            Err(NetlinkError(ErrorMessage { code, header: _ })) => {
                assert_eq!(code, -95);
                eprintln!(
                    "The chain in traffic control is not supported, \
                     please upgrade your kernel"
                );
            }
            _ => {}
        }
    }
    chains
}

// The `cargo test` by default run all tests in parallel, in stead
// of create random named veth/dummy for test, just place class, filter, and
// chain query test in one test case is much simpler.
#[test]
#[cfg_attr(not(feature = "test_as_root"), ignore)]
fn test_get_traffic_classes_filters_and_chains() {
    let ifindex = _add_test_dummy_interface();
    _add_test_tclass_to_dummy();
    _add_test_filter_to_dummy();
    let tclasses = Runtime::new().unwrap().block_on(_get_tclasses(ifindex));
    let filters = Runtime::new().unwrap().block_on(_get_filters(ifindex));
    let chains = Runtime::new().unwrap().block_on(_get_chains(ifindex));
    _remove_test_filter_from_dummy();
    _remove_test_tclass_from_dummy();
    _remove_test_dummy_interface();
    assert_eq!(tclasses.len(), 1);
    let tclass = &tclasses[0];
    assert_eq!(tclass.header.family, AF_UNSPEC as u8);
    assert_eq!(tclass.header.index, ifindex);
    assert_eq!(tclass.header.parent, u32::MAX);
    assert_eq!(tclass.nlas[0], Kind("htb".to_string()));
    assert_eq!(filters.len(), 2);
    assert_eq!(filters[0].header.family, AF_UNSPEC as u8);
    assert_eq!(filters[0].header.index, ifindex);
    assert_eq!(filters[0].header.parent, u16::MAX as u32 + 1);
    assert_eq!(filters[0].nlas[0], Kind("basic".to_string()));
    assert_eq!(filters[1].header.family, AF_UNSPEC as u8);
    assert_eq!(filters[1].header.index, ifindex);
    assert_eq!(filters[1].header.parent, u16::MAX as u32 + 1);
    assert_eq!(filters[1].nlas[0], Kind("basic".to_string()));
    assert!(chains.len() <= 1);
    if chains.len() == 1 {
        assert_eq!(chains[0].header.family, AF_UNSPEC as u8);
        assert_eq!(chains[0].header.index, ifindex);
        assert_eq!(chains[0].header.parent, u16::MAX as u32 + 1);
        assert_eq!(chains[0].nlas[0], Chain([0u8, 0, 0, 0].to_vec()));
    }
}
