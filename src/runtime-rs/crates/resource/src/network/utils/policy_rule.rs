// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use agent::{IPFamily, Rule};
use anyhow::{Context, Result};
use futures::stream::TryStreamExt;
use netlink_packet_route::rule::RuleMessage;
use std::net::IpAddr;

pub(crate) struct NetworkRule {
    pub(crate) rules: Vec<Rule>,
}

impl NetworkRule {
    pub async fn new(handle: &rtnetlink::Handle) -> Result<NetworkRule> {
        let rule_list = handle_rules(handle).await.context("handle rules")?;

        Ok(NetworkRule { rules: rule_list })
    }
}

fn octets_to_addr(octets: &[u8], prefix_len: u8) -> Option<(IpAddr, u8)> {
    return match octets.len() {
        4 => {
            let mut ary: [u8; 4] = Default::default();
            ary.copy_from_slice(octets);
            Some((IpAddr::from(ary), prefix_len))
        }
        16 => {
            let mut ary: [u8; 16] = Default::default();
            ary.copy_from_slice(octets);
            Some((IpAddr::from(ary), prefix_len))
        }
        _ => None,
    };
}

fn generate_rule(mut msg: RuleMessage) -> Result<Option<Rule>> {
    use netlink_packet_route::rule::Nla;

    let mut rule = Rule {
        src: String::new(),
        family: if msg.header.family == libc::AF_INET as u8 {
            IPFamily::V4
        } else {
            IPFamily::V6
        },
        suppress_if_group: -1,
        suppress_prefix_len: -1,
        priority: -1,
        mark: -1,
        mask: -1,
        goto: -1,
        flow: -1,
        table: -1,
    };

    while let Some(attr) = msg.nlas.pop() {
        match attr {
            Nla::Source(src) => {
                let data = octets_to_addr(&src, msg.header.src_len)
                    .map(|(addr, prefix)| format!("{}/{}", addr, prefix))
                    .unwrap_or_default();
                // CIDR notation, such as "192.0.2.0/24"
                rule.src = data;
            }
            Nla::SuppressIfGroup(suppress_group) => {
                if suppress_group != 0xffffffff {
                    rule.suppress_if_group = suppress_group as i64;
                }
            }
            Nla::SuppressPrefixLen(suppress_len) => {
                if suppress_len != 0xffffffff {
                    rule.suppress_prefix_len = suppress_len as i64;
                }
            }
            Nla::FwMark(mark) => {
                rule.mark = mark as i64;
            }
            Nla::FwMask(mask) => {
                rule.mask = mask as i64;
            }
            Nla::Goto(goto) => {
                rule.goto = goto as i64;
            }
            Nla::Flow(flow) => {
                rule.flow = flow as i64;
            }
            Nla::Priority(p) => {
                rule.priority = p as i64;
            }
            Nla::Table(t) => {
                rule.table = t as i64;
            }
            _ => {
                // skip unused attr
            }
        }
    }
    Ok(Some(rule))
}

async fn get_rule_from_msg(
    handle: &rtnetlink::Handle,
    ip_version: rtnetlink::IpVersion,
) -> Result<Vec<Rule>> {
    let mut rules = vec![];
    let mut rule_msg_list = handle.rule().get(ip_version).execute();
    while let Some(rule) = rule_msg_list.try_next().await? {
        if let Some(r) = generate_rule(rule).context("generate rule")? {
            rules.push(r);
        }
    }
    Ok(rules)
}

async fn handle_rules(handle: &rtnetlink::Handle) -> Result<Vec<Rule>> {
    let rules_v4 = get_rule_from_msg(handle, rtnetlink::IpVersion::V4)
        .await
        .context("get ip v4 rule")?;
    let rules_v6 = get_rule_from_msg(handle, rtnetlink::IpVersion::V6)
        .await
        .context("get ip v6 rule")?;
    Ok([rules_v4, rules_v6].concat())
}
