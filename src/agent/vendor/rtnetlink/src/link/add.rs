use byteordered::Endianness;
use futures::stream::StreamExt;

use crate::{
    packet::{
        nlas::link::{Info, InfoData, InfoKind, InfoVlan, InfoVxlan, Nla, VethInfo},
        LinkMessage,
        NetlinkMessage,
        NetlinkPayload,
        RtnlMessage,
        IFF_UP,
        NLM_F_ACK,
        NLM_F_CREATE,
        NLM_F_EXCL,
        NLM_F_REQUEST,
    },
    Error,
    Handle,
};

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
        let ip = match Endianness::native() {
            Endianness::Little => u32::from_le_bytes(addr.octets()),
            Endianness::Big => u32::from_be_bytes(addr.octets()),
        };
        self.info_data.push(InfoVxlan::Group(ip));
        self
    }

    /// Adds the `group` attribute to the VXLAN
    /// This is equivalent to `ip link add name NAME type vxlan id VNI group IPADDR`,
    /// group IPADDR - specifies the multicast IP address to join.
    /// This function takes an IPv6 address
    /// WARNING: only one between `remote` and `group` can be present.
    pub fn group6(mut self, addr: std::net::Ipv6Addr) -> Self {
        let ip: u128 = match Endianness::native() {
            Endianness::Little => u128::from_le_bytes(addr.octets()),
            Endianness::Big => u128::from_be_bytes(addr.octets()),
        };
        self.info_data.push(InfoVxlan::Group6(ip));
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
        let ip = match Endianness::native() {
            Endianness::Little => u32::from_le_bytes(addr.octets()),
            Endianness::Big => u32::from_be_bytes(addr.octets()),
        };

        self.info_data.push(InfoVxlan::Local(ip));
        self
    }

    /// Adds the `local` attribute to the VXLAN
    /// This is equivalent to `ip link add name NAME type vxlan id VNI local IPADDR`,
    /// local IPADDR - specifies the source IP address to use in outgoing packets.
    /// This function takes an IPv6 address.
    pub fn local6(mut self, addr: std::net::Ipv6Addr) -> Self {
        let ip: u128 = match Endianness::native() {
            Endianness::Little => u128::from_le_bytes(addr.octets()),
            Endianness::Big => u128::from_be_bytes(addr.octets()),
        };

        self.info_data.push(InfoVxlan::Local6(ip));
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
}

impl LinkAddRequest {
    pub(crate) fn new(handle: Handle) -> Self {
        LinkAddRequest {
            handle,
            message: LinkMessage::default(),
        }
    }

    /// Execute the request.
    pub async fn execute(self) -> Result<(), Error> {
        let LinkAddRequest {
            mut handle,
            message,
        } = self;
        let mut req = NetlinkMessage::from(RtnlMessage::NewLink(message));
        req.header.flags = NLM_F_REQUEST | NLM_F_ACK | NLM_F_EXCL | NLM_F_CREATE;

        let mut response = handle.request(req)?;
        while let Some(message) = response.next().await {
            if let NetlinkPayload::Error(err) = message.payload {
                return Err(Error::NetlinkError(err));
            }
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

    /// Create a new bridge.
    /// This is equivalent to `ip link add link NAME type bridge`.
    pub fn bridge(self, name: String) -> Self {
        self.name(name.clone())
            .link_info(InfoKind::Bridge, None)
            .append_nla(Nla::IfName(name))
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
