# Networking

Containers typically live in their own, possibly shared, networking namespace.
At some point in a container lifecycle, container engines will set up that namespace
to add the container to a network which is isolated from the host network.

In order to setup the network for a container, container engines call into a
networking plugin. The network plugin will usually create a virtual
ethernet (`veth`) pair adding one end of the `veth` pair into the container
networking namespace, while the other end of the `veth` pair is added to the
host networking namespace.

This is a very namespace-centric approach as many hypervisors or VM
Managers (VMMs) such as `virt-manager` cannot handle `veth`
interfaces. Typically, [`TAP`](https://www.kernel.org/doc/Documentation/networking/tuntap.txt)
interfaces are created for VM connectivity.
There are various internetworking models that overcome incompatibility between typical
container engines expectations and virtual machines,

### TC-Filter

Kata Containers networking transparently connects `veth`
interfaces with `TAP` ones using [Traffic Control](https://man7.org/linux/man-pages/man8/tc.8.html):

![Kata Containers networking](../arch-images/network.png)

With a TC filter rules in place, a redirection is created between the container network
 and the virtual machine. As an example, the network plugin may place a device,
`eth0`, in the container's network namespace, which is one end of a VETH device.
Kata Containers will create a tap device for the VM, `tap0_kata`,
and setup a TC redirection filter to redirect traffic from `eth0`'s ingress to `tap0_kata`'s egress,
and a second TC filter to redirect traffic from `tap0_kata`'s ingress to `eth0`'s egress.

### MACVTAP

Kata Containers maintains support for MACVTAP, which was an earlier implementation used in Kata.
With this method, Kata created a MACVTAP device to connect directly to the `eth0` device.
TC-filter is the default because it allows for simpler configuration, better CNI plugin
compatibility, and performance on par with MACVTAP.

Kata Containers has deprecated support for bridge due to lacking performance relative to TC-filter and MACVTAP.

Kata Containers supports both
[CNM](https://github.com/moby/libnetwork/blob/master/docs/design.md#the-container-network-model)
and [CNI](https://github.com/containernetworking/cni) for networking management.

### L3 Forwarding (Experimental)

With L3 forwarding, we modify the route table on the host-side network namespace to route traffic to the pod IP through the tap device.
This is useful for integrating with service meshes such as Istio Ambient, where the CNI sets up iptables rules and a node proxy listens on the host-side network namespace.
This model supports IPv4 and IPv6.
For IPv6 the pod needs a route via a gateway, because proxy NDP answers only for the gateway addresses, not for a whole on-link prefix: the VM gets each IPv6 pod address as a `/128`, with a host route to each gateway, and reaches even its same-prefix peers through the pod network namespace.
An IPv6 route without a gateway is rejected, other than the host route to a gateway, and so is a pod with IPv6 addresses but no IPv6 route via a gateway.
An IPv6 pod address is moved off the pod interface, and the pod network namespace answers for it with proxy NDP instead: while it keeps the address, it would solicit its own neighbours from it, and their answers would be routed to the VM.
It solicits them from the pod interface's link-local address instead, so a pod interface without one is rejected.
IPv6 routes learned from Router Advertisements are rejected too: once forwarding is enabled, the pod network namespace ignores the Router Advertisements that keep those routes alive.
So are other IPv6 routes that expire, since the VM's copy of a route has no lifetime.
So are multipath (ECMP) IPv6 routes: only routes with a single next hop are passed to the guest.
Port forwarding (`kubectl port-forward`) works over IPv4 only.

## Network Hotplug

Kata Containers has developed a set of network sub-commands and APIs to add, list and
remove a guest network endpoint and to manipulate the guest route table.

The following diagram illustrates the Kata Containers network hotplug workflow.

![Network Hotplug](../arch-images/kata-containers-network-hotplug.png)
