# Setup to run VPP

The Data Plane Development Kit (DPDK) is a set of libraries and drivers for
fast packet processing. Vector Packet Processing (VPP) is a platform
extensible framework that provides out-of-the-box production quality
switch and router functionality. VPP is a high performance packet-processing
stack that can run on commodity CPUs. Enabling VPP with DPDK support can
yield significant performance improvements over a Linux\* bridge providing a
switch with DPDK VHOST-USER ports.

For more information about VPP visit their [wiki](https://wiki.fd.io/view/VPP).

## Install and configure Kata Containers

Follow  the [Kata Containers setup instructions](https://github.com/kata-containers/documentation/wiki/Developer-Guide).

In order to make use of VHOST-USER based interfaces, the container needs to be backed
by huge pages. `HugePages` support is required for the large memory pool allocation used for
DPDK packet buffers.  This is a feature which must be configured within the Linux Kernel. See
[the DPDK documentation](https://doc.dpdk.org/guides/linux_gsg/sys_reqs.html#use-of-hugepages-in-the-linux-environment)
for details on how to enable for the host. After enabling huge pages support on the host system,
update the Kata configuration to enable huge page support in the guest kernel:

```
$ sudo sed -i -e 's/^# *\(enable_hugepages\).*=.*$/\1 = true/g' /usr/share/defaults/kata-containers/configuration.toml
```


## Install VPP

Follow the [VPP installation instructions](https://wiki.fd.io/view/VPP/Installing_VPP_binaries_from_packages).

After a successful installation, your host system is ready to start
connecting Kata Containers with VPP bridges.

### Install the VPP Docker\* plugin

To create a Docker network and connect Kata Containers easily to that network through
Docker, install a VPP Docker plugin.

To install the plugin, follow the [plugin installation instructions](https://github.com/clearcontainers/vpp).

This VPP plugin allows the creation of a VPP network. Every container added
to this network is connected through an L2 bridge-domain provided by VPP.

## Example: Launch two Kata Containers using VPP

To use VPP, use Docker to create a network that makes use of VPP.
For example:

```
$ sudo docker network create -d=vpp --ipam-driver=vpp --subnet=192.168.1.0/24 --gateway=192.168.1.1  vpp_net
```

Test connectivity by launching two containers:
```
$ sudo docker run --runtime=kata-runtime --net=vpp_net --ip=192.168.1.2 --mac-address=CA:FE:CA:FE:01:02 -it busybox bash -c "ip a; ip route; sleep 300"

$ sudo docker run --runtime=kata-runtime --net=vpp_net --ip=192.168.1.3 --mac-address=CA:FE:CA:FE:01:03 -it busybox bash -c "ip a; ip route; ping 192.168.1.2"
```

These commands setup two Kata Containers connected via a VPP L2 bridge
domain. The first of the two VMs displays the networking details and then
sleeps providing a period of time for it to be pinged. The second
VM displays its networking details and then pings the first VM, verifying
connectivity between them.

After verifying connectivity, cleanup with the following commands:

```
$ sudo docker kill $(sudo docker ps --no-trunc -aq)
$ sudo docker rm $(sudo docker ps --no-trunc -aq)
$ sudo docker network rm vpp_net
$ sudo service vpp stop
```

