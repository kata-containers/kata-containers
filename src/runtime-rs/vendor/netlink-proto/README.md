[![Build Status](https://travis-ci.org/little-dude/netlink.svg?branch=master)](https://travis-ci.org/little-dude/netlink)

# Netlink

This project aims at providing building blocks for [netlink][man-netlink] (see `man 7 netlink`).

## Organization

- the [`netlink_sys`](./netlink-sys) crate provides netlink sockets. Integration with [`mio`][mio] and [`tokio`][tokio]
  is optional.
- Each netlink protocol has a `netlink-packet-<protocol_name>` crate that provides the packets for this protocol:
    - [`netlink-packet-route`](./netlink-packet-route) provides messages for the [route protocol][man-rtnetlink]
    - [`netlink-packet-audit`](./netlink-packet-audit) provides messages for the [audit][man-audit] protocol
    - [`netlink-packet-sock-diag`](./netlink-packet-sock-diag) provides messages for the [sock-diag][man-sock-diag]
      protocol
    - [`netlink-packet-generic`](./netlink-packet-generic) provides message for the [generic netlink][man-genl]
      protocol
    - [`netlink-packet-netfilter`](./netlink-packet-netfilter) provides message for the `NETLINK_NETFILTER`
      protocol
- the [`netlink-packet-core`](./netlink-packet-core) is the glue for all the other `netlink-packet-*` crates. It
  provides a `NetlinkMessage<T>` type that represent any netlink message for any sub-protocol.
- the [`netlink_proto`](./netlink-proto) crate is an asynchronous implementation of the netlink protocol. It only
  depends on `netlink-packet-core` for the `NetlinkMessage` type and `netlink-sys` for the socket.
- the [`rtnetlink`](./rtnetlink) crate provides higher level abstraction for the [route protocol][man-rtnetlink]
- the [`audit`](./audit) crate provides higher level abstractions for the audit protocol.
- the [`genetlink`](./genetlink) crate provide higher level abstraction for the
  [generic netlink protocol][man-genl]
- the [`ethtool`](./ethtool) crate provide higher level abstraction for
  [ethtool netlink protocol][ethtool-kernel-doc]


## Altnernatives

- https://github.com/jbaublitz/neli: the main alternative to these crates, as it is actively developed.
- Other but less actively developed alternatives:
  - https://github.com/achanda/netlink
  - https://github.com/polachok/pnetlink
  - https://github.com/crhino/netlink-rs
  - https://github.com/carrotsrc/rsnl
  - https://github.com/TaborKelly/nl-utils

## Credits

My main resource so far has been the source code of [`pyroute2`][pyroute2] (python) and [`netlink`][netlink-go] (golang)
a lot. These two projects are great, and very nicely written. As someone who does not read C fluently, and that does not
know much about netlink, they have been invaluable.

I'd also like to praise [`libnl`][libnl] for its documentation. It helped me a lot in understanding the protocol basics.

The whole packet parsing logic is inspired by @whitequark excellent blog posts ([part 1][whitequark-1], [part
2][whitequark-2] and [part 3][whitequark-3], although I've only really used the concepts described in the first blog
post).

Thanks also to the people behind [tokio](tokio.rs) for the amazing
tool they are building, and the support they provide.

[man-netlink]: https://www.man7.org/linux/man-pages/man7/netlink.7.html
[man-audit]: https://man7.org/linux/man-pages/man3/audit_open.3.html
[man-sock-diag]: https://www.man7.org/linux/man-pages/man7/sock_diag.7.html
[man-rtnetlink]: https://www.man7.org/linux/man-pages/man7/rtnetlink.7.html
[man-genl]: https://www.man7.org/linux/man-pages/man8/genl.8.html
[generic-netlink-lwn]: https://lwn.net/Articles/208755/
[mio]: https://github.com/tokio-rs/mio
[tokio]: https://github.com/tokio-rs/tokio
[route-proto-doc]: https://www.infradead.org/~tgr/libnl/doc/route.html
[netlink-go]: https://github.com/vishvananda/netlink
[pyroute2]: https://github.com/svinota/pyroute2/tree/master/pyroute2/netlink
[libnl]: https://www.infradead.org/~tgr/libnl
[whitequark-1]: https://lab.whitequark.org/notes/2016-12-13/abstracting-over-mutability-in-rust
[whitequark-2]: https://lab.whitequark.org/notes/2016-12-17/owning-collections-in-heap-less-rust
[whitequark-3]: https://lab.whitequark.org/notes/2017-01-16/abstracting-over-mutability-in-rust-macros
[ethtool-kernel-doc]: https://www.kernel.org/doc/html/latest/networking/ethtool-netlink.html
