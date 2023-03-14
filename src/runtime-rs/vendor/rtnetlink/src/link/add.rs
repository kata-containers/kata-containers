// SPDX-License-Identifier: MIT

use futures::stream::StreamExt;
use std::net::{Ipv4Addr, Ipv6Addr};

use crate::{
    packet::{
        nlas::link::{
            Info,
            InfoBond,
            InfoData,
            InfoKind,
            InfoMacVlan,
            InfoVlan,
            InfoVxlan,
            Nla,
            VethInfo,
        },
        LinkMessage,
        NetlinkMessage,
        RtnlMessage,
        IFF_UP,
        NLM_F_ACK,
        NLM_F_CREATE,
        NLM_F_EXCL,
        NLM_F_REPLACE,
        NLM_F_REQUEST,
    },
    try_nl,
    Error,
    Handle,
};

pub struct BondAddRequest {
    request: LinkAddRequest,
    info_data: Vec<InfoBond>,
}

impl BondAddRequest {
    /// Execute the request.
    pub async fn execute(self) -> Result<(), Error> {
        let s = self
            .request
            .link_info(InfoKind::Bond, Some(InfoData::Bond(self.info_data)));
        s.execute().await
    }

    /// Sets the interface up
    /// This is equivalent to `ip link set up dev NAME`.
    pub fn up(mut self) -> Self {
        self.request = self.request.up();
        self
    }

    /// Adds the `mode` attribute to the bond
    /// This is equivalent to `ip link add name NAME type bond mode MODE`.
    pub fn mode(mut self, mode: u8) -> Self {
        self.info_data.push(InfoBond::Mode(mode));
        self
    }

    /// Adds the `active_slave` attribute to the bond, where `active_slave`
    /// is the ifindex of an interface attached to the bond.
    /// This is equivalent to `ip link add name NAME type bond active_slave ACTIVE_SLAVE_NAME`.
    pub fn active_slave(mut self, active_slave: u32) -> Self {
        self.info_data.push(InfoBond::ActiveSlave(active_slave));
        self
    }

    /// Adds the `miimon` attribute to the bond
    /// This is equivalent to `ip link add name NAME type bond miimon MIIMON`.
    pub fn miimon(mut self, miimon: u32) -> Self {
        self.info_data.push(InfoBond::MiiMon(miimon));
        self
    }

    /// Adds the `updelay` attribute to the bond
    /// This is equivalent to `ip link add name NAME type bond updelay UPDELAY`.
    pub fn updelay(mut self, updelay: u32) -> Self {
        self.info_data.push(InfoBond::UpDelay(updelay));
        self
    }

    /// Adds the `downdelay` attribute to the bond
    /// This is equivalent to `ip link add name NAME type bond downdelay DOWNDELAY`.
    pub fn downdelay(mut self, downdelay: u32) -> Self {
        self.info_data.push(InfoBond::DownDelay(downdelay));
        self
    }

    /// Adds the `use_carrier` attribute to the bond
    /// This is equivalent to `ip link add name NAME type bond use_carrier USE_CARRIER`.
    pub fn use_carrier(mut self, use_carrier: u8) -> Self {
        self.info_data.push(InfoBond::UseCarrier(use_carrier));
        self
    }

    /// Adds the `arp_interval` attribute to the bond
    /// This is equivalent to `ip link add name NAME type bond arp_interval ARP_INTERVAL`.
    pub fn arp_interval(mut self, arp_interval: u32) -> Self {
        self.info_data.push(InfoBond::ArpInterval(arp_interval));
        self
    }

    /// Adds the `arp_validate` attribute to the bond
    /// This is equivalent to `ip link add name NAME type bond arp_validate ARP_VALIDATE`.
    pub fn arp_validate(mut self, arp_validate: u32) -> Self {
        self.info_data.push(InfoBond::ArpValidate(arp_validate));
        self
    }

    /// Adds the `arp_all_targets` attribute to the bond
    /// This is equivalent to `ip link add name NAME type bond arp_all_targets ARP_ALL_TARGETS`
    pub fn arp_all_targets(mut self, arp_all_targets: u32) -> Self {
        self.info_data
            .push(InfoBond::ArpAllTargets(arp_all_targets));
        self
    }

    /// Adds the `primary` attribute to the bond, where `primary` is the ifindex
    /// of an interface.
    /// This is equivalent to `ip link add name NAME type bond primary PRIMARY_NAME`
    pub fn primary(mut self, primary: u32) -> Self {
        self.info_data.push(InfoBond::Primary(primary));
        self
    }

    /// Adds the `primary_reselect` attribute to the bond
    /// This is equivalent to `ip link add name NAME type bond primary_reselect PRIMARY_RESELECT`.
    pub fn primary_reselect(mut self, primary_reselect: u8) -> Self {
        self.info_data
            .push(InfoBond::PrimaryReselect(primary_reselect));
        self
    }

    /// Adds the `fail_over_mac` attribute to the bond
    /// This is equivalent to `ip link add name NAME type bond fail_over_mac FAIL_OVER_MAC`.
    pub fn fail_over_mac(mut self, fail_over_mac: u8) -> Self {
        self.info_data.push(InfoBond::FailOverMac(fail_over_mac));
        self
    }

    /// Adds the `xmit_hash_policy` attribute to the bond
    /// This is equivalent to `ip link add name NAME type bond xmit_hash_policy XMIT_HASH_POLICY`.
    pub fn xmit_hash_policy(mut self, xmit_hash_policy: u8) -> Self {
        self.info_data
            .push(InfoBond::XmitHashPolicy(xmit_hash_policy));
        self
    }

    /// Adds the `resend_igmp` attribute to the bond
    /// This is equivalent to `ip link add name NAME type bond resend_igmp RESEND_IGMP`.
    pub fn resend_igmp(mut self, resend_igmp: u32) -> Self {
        self.info_data.push(InfoBond::ResendIgmp(resend_igmp));
        self
    }

    /// Adds the `num_peer_notif` attribute to the bond
    /// This is equivalent to `ip link add name NAME type bond num_peer_notif NUM_PEER_NOTIF`.
    pub fn num_peer_notif(mut self, num_peer_notif: u8) -> Self {
        self.info_data.push(InfoBond::NumPeerNotif(num_peer_notif));
        self
    }

    /// Adds the `all_slaves_active` attribute to the bond
    /// This is equivalent to `ip link add name NAME type bond all_slaves_active ALL_SLAVES_ACTIVE`.
    pub fn all_slaves_active(mut self, all_slaves_active: u8) -> Self {
        self.info_data
            .push(InfoBond::AllSlavesActive(all_slaves_active));
        self
    }

    /// Adds the `min_links` attribute to the bond
    /// This is equivalent to `ip link add name NAME type bond min_links MIN_LINKS`.
    pub fn min_links(mut self, min_links: u32) -> Self {
        self.info_data.push(InfoBond::MinLinks(min_links));
        self
    }

    /// Adds the `lp_interval` attribute to the bond
    /// This is equivalent to `ip link add name NAME type bond lp_interval LP_INTERVAL`.
    pub fn lp_interval(mut self, lp_interval: u32) -> Self {
        self.info_data.push(InfoBond::LpInterval(lp_interval));
        self
    }

    /// Adds the `packets_per_slave` attribute to the bond
    /// This is equivalent to `ip link add name NAME type bond packets_per_slave PACKETS_PER_SLAVE`.
    pub fn packets_per_slave(mut self, packets_per_slave: u32) -> Self {
        self.info_data
            .push(InfoBond::PacketsPerSlave(packets_per_slave));
        self
    }

    /// Adds the `ad_lacp_rate` attribute to the bond
    /// This is equivalent to `ip link add name NAME type bond ad_lacp_rate AD_LACP_RATE`.
    pub fn ad_lacp_rate(mut self, ad_lacp_rate: u8) -> Self {
        self.info_data.push(InfoBond::AdLacpRate(ad_lacp_rate));
        self
    }

    /// Adds the `ad_select` attribute to the bond
    /// This is equivalent to `ip link add name NAME type bond ad_select AD_SELECT`.
    pub fn ad_select(mut self, ad_select: u8) -> Self {
        self.info_data.push(InfoBond::AdSelect(ad_select));
        self
    }

    /// Adds the `ad_actor_sys_prio` attribute to the bond
    /// This is equivalent to `ip link add name NAME type bond ad_actor_sys_prio AD_ACTOR_SYS_PRIO`.
    pub fn ad_actor_sys_prio(mut self, ad_actor_sys_prio: u16) -> Self {
        self.info_data
            .push(InfoBond::AdActorSysPrio(ad_actor_sys_prio));
        self
    }

    /// Adds the `ad_user_port_key` attribute to the bond
    /// This is equivalent to `ip link add name NAME type bond ad_user_port_key AD_USER_PORT_KEY`.
    pub fn ad_user_port_key(mut self, ad_user_port_key: u16) -> Self {
        self.info_data
            .push(InfoBond::AdUserPortKey(ad_user_port_key));
        self
    }

    /// Adds the `ad_actor_system` attribute to the bond
    /// This is equivalent to `ip link add name NAME type bond ad_actor_system AD_ACTOR_SYSTEM`.
    pub fn ad_actor_system(mut self, ad_actor_system: [u8; 6]) -> Self {
        self.info_data
            .push(InfoBond::AdActorSystem(ad_actor_system));
        self
    }

    /// Adds the `tlb_dynamic_lb` attribute to the bond
    /// This is equivalent to `ip link add name NAME type bond tlb_dynamic_lb TLB_DYNAMIC_LB`.
    pub fn tlb_dynamic_lb(mut self, tlb_dynamic_lb: u8) -> Self {
        self.info_data.push(InfoBond::TlbDynamicLb(tlb_dynamic_lb));
        self
    }

    /// Adds the `peer_notif_delay` attribute to the bond
    /// This is equivalent to `ip link add name NAME type bond peer_notif_delay PEER_NOTIF_DELAY`.
    pub fn peer_notif_delay(mut self, peer_notif_delay: u32) -> Self {
        self.info_data
            .push(InfoBond::PeerNotifDelay(peer_notif_delay));
        self
    }

    /// Adds the `ad_lacp_active` attribute to the bond
    /// This is equivalent to `ip link add name NAME type bond ad_lacp_active AD_LACP_ACTIVE`.
    pub fn ad_lacp_active(mut self, ad_lacp_active: u8) -> Self {
        self.info_data.push(InfoBond::AdLacpActive(ad_lacp_active));
        self
    }

    /// Adds the `missed_max` attribute to the bond
    /// This is equivalent to `ip link add name NAME type bond missed_max MISSED_MAX`.
    pub fn missed_max(mut self, missed_max: u8) -> Self {
        self.info_data.push(InfoBond::MissedMax(missed_max));
        self
    }

    /// Adds the `arp_ip_target` attribute to the bond
    /// This is equivalent to `ip link add name NAME type bond arp_ip_target LIST`.
    pub fn arp_ip_target(mut self, arp_ip_target: Vec<Ipv4Addr>) -> Self {
        self.info_data.push(InfoBond::ArpIpTarget(arp_ip_target));
        self
    }

    /// Adds the `ns_ip6_target` attribute to the bond
    /// This is equivalent to `ip link add name NAME type bond ns_ip6_target LIST`.
    pub fn ns_ip6_target(mut self, ns_ip6_target: Vec<Ipv6Addr>) -> Self {
        self.info_data.push(InfoBond::NsIp6Target(ns_ip6_target));
        self
    }
}

/// A request to create a new vxlan link.
///  This is equivalent to `ip link add NAME vxlan id ID ...` commands.
/// It provides methods to customize the creation of the vxlan interface
/// It provides almost all parameters that are listed by `man ip link`.
pub struct VxlanAddRequest {
    request: LinkAddRequest,
    info_data: Vec<InfoVxlan>,
}

impl VxlanAddRequest {
    /// Execute the request.
    pub async fn execute(self) -> Result<(), Error> {
        let s = self
            .request
            .link_info(InfoKind::Vxlan, Some(InfoData::Vxlan(self.info_data)));
        s.execute().await
    }

    /// Sets the interface up
    /// This is equivalent to `ip link set up dev NAME`.
    pub fn up(mut self) -> Self {
        self.request = self.request.up();
        self
    }

    /// Adds the `dev` attribute to the VXLAN
    /// This is equivalent to `ip link add name NAME type vxlan id VNI dev LINK`,
    ///  dev LINK - specifies the physical device to use
    ///  for tunnel endpoint communication.
    /// But instead of specifing a link name (`LINK`), we specify a link index.
    pub fn link(mut self, index: u32) -> Self {
        self.info_data.push(InfoVxlan::Link(index));
        self
    }

    /// Adds the `dstport` attribute to the VXLAN
    /// This is equivalent to `ip link add name NAME type vxlan id VNI dstport PORT`.
    /// dstport PORT - specifies the UDP destination port to
    /// communicate to the remote VXLAN tunnel endpoint.
    pub fn port(mut self, port: u16) -> Self {
        self.info_data.push(InfoVxlan::Port(port));
        self
    }

    /// Adds the `group` attribute to the VXLAN
    /// This is equivalent to `ip link add name NAME type vxlan id VNI group IPADDR`,
    /// group IPADDR - specifies the multicast IP address to join.
    /// This function takes an IPv4 address
    /// WARNING: only one between `remote` and `group` can be present.
    pub fn group(mut self, addr: std::net::Ipv4Addr) -> Self {
        self.info_data
            .push(InfoVxlan::Group(addr.octets().to_vec()));
        self
    }

    /// Adds the `group` attribute to the VXLAN
    /// This is equivalent to `ip link add name NAME type vxlan id VNI group IPADDR`,
    /// group IPADDR - specifies the multicast IP address to join.
    /// This function takes an IPv6 address
    /// WARNING: only one between `remote` and `group` can be present.
    pub fn group6(mut self, addr: std::net::Ipv6Addr) -> Self {
        self.info_data
            .push(InfoVxlan::Group6(addr.octets().to_vec()));
        self
    }

    /// Adds the `remote` attribute to the VXLAN
    /// This is equivalent to `ip link add name NAME type vxlan id VNI remote IPADDR`,
    /// remote IPADDR - specifies the unicast destination IP
    /// address to use in outgoing packets when the
    /// destination link layer address is not known in the
    /// VXLAN device forwarding database.
    /// This function takes an IPv4 address.
    /// WARNING: only one between `remote` and `group` can be present.
    pub fn remote(self, addr: std::net::Ipv4Addr) -> Self {
        self.group(addr)
    }

    /// Adds the `remote` attribute to the VXLAN
    /// This is equivalent to `ip link add name NAME type vxlan id VNI remote IPADDR`,
    /// remote IPADDR - specifies the unicast destination IP
    /// address to use in outgoing packets when the
    /// destination link layer address is not known in the
    /// VXLAN device forwarding database.
    /// This function takes an IPv6 address.
    /// WARNING: only one between `remote` and `group` can be present.
    pub fn remote6(self, addr: std::net::Ipv6Addr) -> Self {
        self.group6(addr)
    }

    /// Adds the `local` attribute to the VXLAN
    /// This is equivalent to `ip link add name NAME type vxlan id VNI local IPADDR`,
    /// local IPADDR - specifies the source IP address to use in outgoing packets.
    /// This function takes an IPv4 address.
    pub fn local(mut self, addr: std::net::Ipv4Addr) -> Self {
        self.info_data
            .push(InfoVxlan::Local(addr.octets().to_vec()));
        self
    }

    /// Adds the `local` attribute to the VXLAN
    /// This is equivalent to `ip link add name NAME type vxlan id VNI local IPADDR`,
    /// local IPADDR - specifies the source IP address to use in outgoing packets.
    /// This function takes an IPv6 address.
    pub fn local6(mut self, addr: std::net::Ipv6Addr) -> Self {
        self.info_data
            .push(InfoVxlan::Local6(addr.octets().to_vec()));
        self
    }

    /// Adds the `tos` attribute to the VXLAN
    /// This is equivalent to `ip link add name NAME type vxlan id VNI tos TOS`.
    /// tos TOS - specifies the TOS value to use in outgoing packets.
    pub fn tos(mut self, tos: u8) -> Self {
        self.info_data.push(InfoVxlan::Tos(tos));
        self
    }

    /// Adds the `ttl` attribute to the VXLAN
    /// This is equivalent to `ip link add name NAME type vxlan id VNI ttl TTL`.
    /// ttl TTL - specifies the TTL value to use in outgoing packets.
    pub fn ttl(mut self, ttl: u8) -> Self {
        self.info_data.push(InfoVxlan::Ttl(ttl));
        self
    }

    /// Adds the `flowlabel` attribute to the VXLAN
    /// This is equivalent to `ip link add name NAME type vxlan id VNI flowlabel LABEL`.
    /// flowlabel LABEL - specifies the flow label to use in outgoing packets.
    pub fn label(mut self, label: u32) -> Self {
        self.info_data.push(InfoVxlan::Label(label));
        self
    }

    /// Adds the `learning` attribute to the VXLAN
    /// This is equivalent to `ip link add name NAME type vxlan id VNI [no]learning`.
    /// [no]learning - specifies if unknown source link layer
    /// addresses and IP addresses are entered into the VXLAN
    /// device forwarding database.
    pub fn learning(mut self, learning: u8) -> Self {
        self.info_data.push(InfoVxlan::Learning(learning));
        self
    }

    /// Adds the `ageing` attribute to the VXLAN
    /// This is equivalent to `ip link add name NAME type vxlan id VNI ageing SECONDS`.
    /// ageing SECONDS - specifies the lifetime in seconds of
    /// FDB entries learnt by the kernel.
    pub fn ageing(mut self, seconds: u32) -> Self {
        self.info_data.push(InfoVxlan::Ageing(seconds));
        self
    }

    /// Adds the `maxaddress` attribute to the VXLAN
    /// This is equivalent to `ip link add name NAME type vxlan id VNI maxaddress LIMIT`.
    /// maxaddress LIMIT - specifies the maximum number of
    /// FDB entries.
    pub fn limit(mut self, limit: u32) -> Self {
        self.info_data.push(InfoVxlan::Limit(limit));
        self
    }

    /// Adds the `srcport` attribute to the VXLAN
    /// This is equivalent to `ip link add name NAME type vxlan id VNI srcport MIN MAX`.
    /// srcport MIN MAX - specifies the range of port numbers
    /// to use as UDP source ports to communicate to the
    /// remote VXLAN tunnel endpoint.
    pub fn port_range(mut self, min: u16, max: u16) -> Self {
        self.info_data.push(InfoVxlan::PortRange((min, max)));
        self
    }

    /// Adds the `proxy` attribute to the VXLAN
    /// This is equivalent to `ip link add name NAME type vxlan id VNI [no]proxy`.
    /// [no]proxy - specifies ARP proxy is turned on.
    pub fn proxy(mut self, proxy: u8) -> Self {
        self.info_data.push(InfoVxlan::Proxy(proxy));
        self
    }

    /// Adds the `rsc` attribute to the VXLAN
    /// This is equivalent to `ip link add name NAME type vxlan id VNI [no]rsc`.
    /// [no]rsc - specifies if route short circuit is turned on.
    pub fn rsc(mut self, rsc: u8) -> Self {
        self.info_data.push(InfoVxlan::Rsc(rsc));
        self
    }

    // Adds the `l2miss` attribute to the VXLAN
    /// This is equivalent to `ip link add name NAME type vxlan id VNI [no]l2miss`.
    /// [no]l2miss - specifies if netlink LLADDR miss notifications are generated.
    pub fn l2miss(mut self, l2miss: u8) -> Self {
        self.info_data.push(InfoVxlan::L2Miss(l2miss));
        self
    }

    // Adds the `l3miss` attribute to the VXLAN
    /// This is equivalent to `ip link add name NAME type vxlan id VNI [no]l3miss`.
    /// [no]l3miss - specifies if netlink IP ADDR miss notifications are generated.
    pub fn l3miss(mut self, l3miss: u8) -> Self {
        self.info_data.push(InfoVxlan::L3Miss(l3miss));
        self
    }

    pub fn collect_metadata(mut self, collect_metadata: u8) -> Self {
        self.info_data
            .push(InfoVxlan::CollectMetadata(collect_metadata));
        self
    }

    // Adds the `udp_csum` attribute to the VXLAN
    /// This is equivalent to `ip link add name NAME type vxlan id VNI [no]udp_csum`.
    /// [no]udpcsum - specifies if UDP checksum is calculated for transmitted packets over IPv4.
    pub fn udp_csum(mut self, udp_csum: u8) -> Self {
        self.info_data.push(InfoVxlan::UDPCsum(udp_csum));
        self
    }
}

/// A request to create a new link. This is equivalent to the `ip link add` commands.
///
/// A few methods for common actions (creating a veth pair, creating a vlan interface, etc.) are
/// provided, but custom requests can be made using the [`message_mut()`](#method.message_mut)
/// accessor.
pub struct LinkAddRequest {
    handle: Handle,
    message: LinkMessage,
    replace: bool,
}

impl LinkAddRequest {
    pub(crate) fn new(handle: Handle) -> Self {
        LinkAddRequest {
            handle,
            message: LinkMessage::default(),
            replace: false,
        }
    }

    /// Execute the request.
    pub async fn execute(self) -> Result<(), Error> {
        let LinkAddRequest {
            mut handle,
            message,
            replace,
        } = self;
        let mut req = NetlinkMessage::from(RtnlMessage::NewLink(message));
        let replace = if replace { NLM_F_REPLACE } else { NLM_F_EXCL };
        req.header.flags = NLM_F_REQUEST | NLM_F_ACK | replace | NLM_F_CREATE;

        let mut response = handle.request(req)?;
        while let Some(message) = response.next().await {
            try_nl!(message);
        }
        Ok(())
    }

    /// Return a mutable reference to the request message.
    ///
    /// # Example
    ///
    /// Let's say we want to create a vlan interface on a link with id 6. By default, the
    /// [`vlan()`](#method.vlan) method would create a request with the `IFF_UP` link set, so that the
    /// interface is up after creation. If we want to create a interface tha tis down by default we
    /// could do:
    ///
    /// ```rust,no_run
    /// use futures::Future;
    /// use rtnetlink::{Handle, new_connection, packet::IFF_UP};
    ///
    /// async fn run(handle: Handle) -> Result<(), String> {
    ///     let vlan_id = 100;
    ///     let link_id = 6;
    ///     let mut request = handle.link().add().vlan("my-vlan-itf".into(), link_id, vlan_id);
    ///     // unset the IFF_UP flag before sending the request
    ///     request.message_mut().header.flags &= !IFF_UP;
    ///     request.message_mut().header.change_mask &= !IFF_UP;
    ///     // send the request
    ///     request.execute().await.map_err(|e| format!("{}", e))
    /// }
    pub fn message_mut(&mut self) -> &mut LinkMessage {
        &mut self.message
    }

    /// Create a dummy link.
    /// This is equivalent to `ip link add NAME type dummy`.
    pub fn dummy(self, name: String) -> Self {
        self.name(name).link_info(InfoKind::Dummy, None).up()
    }

    /// Create a veth pair.
    /// This is equivalent to `ip link add NAME1 type veth peer name NAME2`.
    pub fn veth(self, name: String, peer_name: String) -> Self {
        // NOTE: `name` is the name of the peer in the netlink message (ie the link created via the
        // VethInfo::Peer attribute, and `peer_name` is the name in the main netlink message.
        // This is a bit weird, but it's all hidden from the user.

        let mut peer = LinkMessage::default();
        // FIXME: we get a -107 (ENOTCONN) (???) when trying to set `name` up.
        // peer.header.flags = LinkFlags::from(IFF_UP);
        // peer.header.change_mask = LinkFlags::from(IFF_UP);
        peer.nlas.push(Nla::IfName(name));
        let link_info_data = InfoData::Veth(VethInfo::Peer(peer));
        self.name(peer_name)
            .up() // iproute2 does not set this one up
            .link_info(InfoKind::Veth, Some(link_info_data))
    }

    /// Create VLAN on a link.
    /// This is equivalent to `ip link add link LINK name NAME type vlan id VLAN_ID`,
    /// but instead of specifying a link name (`LINK`), we specify a link index.
    pub fn vlan(self, name: String, index: u32, vlan_id: u16) -> Self {
        self.name(name)
            .link_info(
                InfoKind::Vlan,
                Some(InfoData::Vlan(vec![InfoVlan::Id(vlan_id)])),
            )
            .append_nla(Nla::Link(index))
            .up()
    }

    /// Create macvlan on a link.
    /// This is equivalent to `ip link add name NAME link LINK type macvlan mode MACVLAN_MODE`,
    ///   but instead of specifying a link name (`LINK`), we specify a link index.
    /// The MACVLAN_MODE is an integer consisting of flags from MACVLAN_MODE (netlink-packet-route/src/rtnl/constants.rs)
    ///   being: _PRIVATE, _VEPA, _BRIDGE, _PASSTHRU, _SOURCE, which can be *combined*.
    pub fn macvlan(self, name: String, index: u32, mode: u32) -> Self {
        self.name(name)
            .link_info(
                InfoKind::MacVlan,
                Some(InfoData::MacVlan(vec![InfoMacVlan::Mode(mode)])),
            )
            .append_nla(Nla::Link(index))
            .up()
    }

    /// Create a VxLAN
    /// This is equivalent to `ip link add name NAME type vxlan id VNI`,
    /// it returns a VxlanAddRequest to further customize the vxlan
    /// interface creation.
    pub fn vxlan(self, name: String, vni: u32) -> VxlanAddRequest {
        let s = self.name(name);
        VxlanAddRequest {
            request: s,
            info_data: vec![InfoVxlan::Id(vni)],
        }
    }

    /// Create a new bond.
    /// This is equivalent to `ip link add link NAME type bond`.
    pub fn bond(self, name: String) -> BondAddRequest {
        let s = self.name(name);
        BondAddRequest {
            request: s,
            info_data: vec![],
        }
    }

    /// Create a new bridge.
    /// This is equivalent to `ip link add link NAME type bridge`.
    pub fn bridge(self, name: String) -> Self {
        self.name(name.clone())
            .link_info(InfoKind::Bridge, None)
            .append_nla(Nla::IfName(name))
    }

    /// Replace existing matching link.
    pub fn replace(self) -> Self {
        Self {
            replace: true,
            ..self
        }
    }

    fn up(mut self) -> Self {
        self.message.header.flags = IFF_UP;
        self.message.header.change_mask = IFF_UP;
        self
    }

    fn link_info(self, kind: InfoKind, data: Option<InfoData>) -> Self {
        let mut link_info_nlas = vec![Info::Kind(kind)];
        if let Some(data) = data {
            link_info_nlas.push(Info::Data(data));
        }
        self.append_nla(Nla::Info(link_info_nlas))
    }

    fn name(mut self, name: String) -> Self {
        self.message.nlas.push(Nla::IfName(name));
        self
    }

    fn append_nla(mut self, nla: Nla) -> Self {
        self.message.nlas.push(nla);
        self
    }
}
