The rtnetlink dump was generated with:

```
sudo ip link add name qemu-br1 type bridge
sudo ip link set qemu-br1 up
sudo ip address add 192.168.10.1/24 dev qemu-br1

docker run -d -it busybox /bin/sh

sudo ip netns add blue
sudo ip link add veth0 type veth peer name veth1
sudo ip netns list
sudo ip link set veth1 netns blue
sudo ip -6 link add vxlan100 type vxlan id 100 dstport 4789 local 2001:db8:1::1 group ff05::100 dev veth0 ttl 5
sudo brctl addbr br100
sudo brctl addif br100 vxlan100
sudo ip link show
sudo ip address show
sudo ip neigh show
sudo ip route show

tc qdisc show
```
