# User Guide: SPDK Volume CSI Driver

## 1. Prerequisites
* A running **Kubernetes cluster with Kata Containers (Dragonball)** enabled.
* The `csi-kata-directvolume` is deployed.
* **SPDK** is built and `spdk_tgt` is available.

## 2. Start the SPDK Service

```sh
# Environment variables
# By default, the driver uses `/var/lib/spdk/vhost` and `/var/lib/spdk/rawdisks` 
# you need to set your own paths according to your environment.
export SPDK_DEVEL=<path-to-your-spdk>
export VHU_UDS_PATH=<your_vhost_path>
export RAW_DISKS=<your_rawdisk_path>

# Stop existing process
sudo pkill spdk_tgt || true

# Reset and allocate hugepages
cd $SPDK_DEVEL
sudo ./scripts/setup.sh reset
sudo sysctl -w vm.nr_hugepages=2048
sudo HUGEMEM=4096 ./scripts/setup.sh

# Start SPDK vhost target
sudo mkdir -p $VHU_UDS_PATH
sudo $SPDK_DEVEL/build/bin/spdk_tgt -S $VHU_UDS_PATH -s 1024 -m 0x3 &
```

> Notes:
>
> * `-s 1024`: size of the hugepage memory pool in MB.
> * `-m 0x3`: CPU mask specifying which cores SPDK will use.

## 3. Deploy Kubernetes Resources

Run the example script:

```sh
cd kata-containers/src/tools/csi-kata-directvolume/examples/pod-with-spdkvol
kubectl apply -f csi-storageclass.yaml
kubectl apply -f csi-pvc.yaml
kubectl apply -f csi-app.yaml
```

This creates:

* Storage Class `spdk-test-adapted`
* PVC `kata-spdk-directvolume-pvc`
* Pod `spdk-pod-test`

## 4. Verify the Volume Inside the Pod

Check the mounted block device:

```sh
$ kubectl exec -it spdk-pod-test -- /bin/sh

$ lsblk
NAME   MAJ:MIN RM  SIZE RO TYPE MOUNTPOINTS
vda    254:0    0  256M  1 disk 
`-vda1 254:1    0  253M  1 part 
vdb    254:16   0    2G  0 disk /data

$ echo "hello spdk" > /data/test.txt
$ cat /data/test.txt
hello spdk
```

The SPDK-backed volume `/dev/vdb` is mounted to `/data` inside the container.

## 5. Verify Data on the Host

On the host, inspect the raw backing file:

```sh
# Locate the raw file
ls /var/lib/spdk/rawdisks

# Attach it as a loop device
sudo losetup -fP /var/lib/spdk/rawdisks/pvc-xxxx.raw

# List loop devices
sudo losetup -a

# Mount and check data
sudo mkdir -p /mnt/testvol
sudo mount /dev/loopX /mnt/testvol
ls /mnt/testvol
cat /mnt/testvol/test.txt
hello spdk

# Cleanup
sudo umount /mnt/testvol
sudo losetup -d /dev/loopX
```

## 6. Cleanup

```sh
kubectl delete -f csi-app.yaml
kubectl delete -f csi-pvc.yaml
kubectl delete -f csi-storageclass.yaml
```

---

## Notes
* The SPDK CSI driver exposes **SPDK-managed volumes via vhost-user-blk** into Kata containers.
* Usage is the same as regular PVCs — specify `volumetype=spdkvol` in the Storage Class.


