# Kata Containers with Guest Image Management

Kata Containers 3.3.0 introduces the guest image management feature, which enables the guest VM to directly pull images using `nydus snapshotter`. This feature is designed to protect the integrity of container images and guard against any tampering by the host, which is used for confidential containers. Please refer to [kata-guest-image-management-design](../design/kata-guest-image-management-design.md) for details.

## Prerequisites
- The k8s cluster with Kata 3.3.0+ is ready to use.
- `yq` is installed in the host and it's directory is included in the `PATH` environment variable. (optional, for DaemonSet only)

## Deploy `nydus snapshotter` for guest image management

To pull images in the guest, we need to do the following steps:
1. Delete images used for pulling in the guest (optional, for containerd only)
2. Install `nydus snapshotter`:
   1. Install `nydus snapshotter` by k8s DaemonSet (recommended)
   2. Install `nydus snapshotter` manually

### Delete images used for pulling in the guest

Though the `CRI Runtime Specific Snapshotter` is still an [experimental feature](https://github.com/containerd/containerd/blob/main/RELEASES.md#experimental-features) in containerd, which containerd is not supported to manage the same image in different `snapshotters`(The default `snapshotter` in containerd is `overlayfs`). To avoid errors caused by this, it is recommended to delete images (including the pause image) in containerd that needs to be pulled in guest later before configuring `nydus snapshotter` in containerd. 

### Install `nydus snapshotter`

#### Install `nydus snapshotter` by k8s DaemonSet (recommended)

To use DaemonSet to install `nydus snapshotter`, we need to ensure that `yq` exists in the host.

1. Download `nydus snapshotter` repo 
```bash
$ nydus_snapshotter_install_dir="/tmp/nydus-snapshotter"
$ nydus_snapshotter_url=https://github.com/containerd/nydus-snapshotter
$ nydus_snapshotter_version="v0.13.11"
$ git clone -b "${nydus_snapshotter_version}" "${nydus_snapshotter_url}" "${nydus_snapshotter_install_dir}"
```

2. Configure DaemonSet file
```bash
$ pushd "$nydus_snapshotter_install_dir"
$ yq -i \
>	 '.data.FS_DRIVER = "proxy"' -P \
>	 misc/snapshotter/base/nydus-snapshotter.yaml
# Disable to read snapshotter config from configmap
$ yq -i \
>	 'data.ENABLE_CONFIG_FROM_VOLUME = "false"' -P \
>	 misc/snapshotter/base/nydus-snapshotter.yaml
# Enable to run snapshotter as a systemd service 
# (skip if you want to run nydus snapshotter as a standalone process)
$ yq -i \
>	 'data.ENABLE_SYSTEMD_SERVICE = "true"' -P \
>	 misc/snapshotter/base/nydus-snapshotter.yaml
# Enable "runtime specific snapshotter" feature in containerd when configuring containerd for snapshotter
# (skip if you want to configure nydus snapshotter as a global snapshotter in containerd)
$ yq -i \
>	 'data.ENABLE_RUNTIME_SPECIFIC_SNAPSHOTTER = "true"' -P \
>	 misc/snapshotter/base/nydus-snapshotter.yaml
```

3. Install `nydus snapshotter` as a DaemonSet
```bash
$ kubectl create -f "misc/snapshotter/nydus-snapshotter-rbac.yaml"
$ kubectl apply -f "misc/snapshotter/base/nydus-snapshotter.yaml"
```

4. Wait 5 minutes until the DaemonSet is running
```bash
$ kubectl rollout status DaemonSet nydus-snapshotter -n nydus-system --timeout 5m
```

5. Verify whether `nydus snapshotter` is running as a DaemonSet
```bash
$ pods_name=$(kubectl get pods --selector=app=nydus-snapshotter -n nydus-system -o=jsonpath='{.items[*].metadata.name}')
$ kubectl logs "${pods_name}" -n nydus-system
deploying snapshotter
install nydus snapshotter artifacts
configuring snapshotter
Not found nydus proxy plugin!
running snapshotter as systemd service
Created symlink /etc/systemd/system/multi-user.target.wants/nydus-snapshotter.service → /etc/systemd/system/nydus-snapshotter.service.
```

#### Install `nydus snapshotter` manually

1. Download `nydus snapshotter` binary from release 
```bash
$ ARCH=$(uname -m)
$ golang_arch=$(case "$ARCH" in
    aarch64) echo "arm64" ;;
    ppc64le) echo "ppc64le" ;;
    x86_64) echo "amd64" ;;
    s390x) echo "s390x" ;;
esac)
$ release_tarball="nydus-snapshotter-${nydus_snapshotter_version}-linux-${golang_arch}.tar.gz"
$ curl -OL ${nydus_snapshotter_url}/releases/download/${nydus_snapshotter_version}/${release_tarball}
$ sudo tar -xfz ${release_tarball} -C /usr/local/bin --strip-components=1
```

2. Download `nydus snapshotter` configuration file for pulling images in the guest
```bash
$ curl -OL https://github.com/containerd/nydus-snapshotter/blob/main/misc/snapshotter/config-proxy.toml
$ sudo install -D -m 644 config-proxy.toml /etc/nydus/config-proxy.toml
```

3. Run `nydus snapshotter` as a standalone process
```bash
$ /usr/local/bin/containerd-nydus-grpc --config /etc/nydus/config-proxy.toml --log-to-stdout
level=info msg="Start nydus-snapshotter. Version: v0.13.11-308-g106a6cb, PID: 1100169, FsDriver: proxy, DaemonMode: none"
level=info msg="Run daemons monitor..."
```

4. Configure containerd for `nydus snapshotter`

Configure `nydus snapshotter` to enable `CRI Runtime Specific Snapshotter` in containerd. This ensures run kata containers with `nydus snapshotter`. Below, the steps are illustrated using `kata-qemu` as an example.

```toml
# Modify containerd configuration to ensure that the following lines appear in the containerd configuration 
# (Assume that the containerd config is located in /etc/containerd/config.toml)

[plugins."io.containerd.grpc.v1.cri".containerd]
  disable_snapshot_annotations = false
  discard_unpacked_layers = false
[proxy_plugins.nydus]
  type = "snapshot"
  address = "/run/containerd-nydus/containerd-nydus-grpc.sock"
[plugins."io.containerd.grpc.v1.cri".containerd.runtimes.kata-qemu]
  snapshotter = "nydus"
```

> **Notes:** 
> The `CRI Runtime Specific Snapshotter` feature only works for containerd v1.7.0 and above. So for Containerd v1.7.0 below, in addition to the above settings, we need to set the global `snapshotter` to `nydus` in containerd config. For example:

```toml
[plugins."io.containerd.grpc.v1.cri".containerd]
snapshotter = "nydus"
```

5. Restart containerd service
```bash
$ sudo systemctl restart containerd
```

## Verification

To verify pulling images in a guest VM, please refer to the following commands:

1. Run a kata container
```bash
$ cat <<EOF | kubectl apply -f -
apiVersion: v1
kind: Pod
metadata:
  name: busybox
  annotations:
    io.containerd.cri.runtime-handler: kata-qemu
spec:
  runtimeClassName: kata-qemu
  containers:
  - name: busybox
    image: quay.io/prometheus/busybox:latest
    imagePullPolicy: Always
EOF
pod/busybox created
$ kubectl get pods
NAME         READY   STATUS    RESTARTS   AGE
busybox      1/1     Running   0          10s
```

> **Notes:** 
> The `CRI Runtime Specific Snapshotter` is still an experimental feature. To pull images in the guest under the specific kata runtime (such as `kata-qemu`), we need to add the following annotation in metadata to each pod yaml: `io.containerd.cri.runtime-handler: kata-qemu`. By adding the annotation, we can ensure that the feature works as expected.

2. Verify that the pod's images have been successfully downloaded in the guest.
If images intended for deployment are deleted prior to deploying with `nydus snapshotter`, the root filesystems required for the pod's images (including the pause image and the container image) should not be present on the host.
```bash
$ sandbox_id=$(ps -ef| grep containerd-shim-kata-v2| grep -oP '(?<=-id\s)[a-f0-9]+'| tail -1)
$ rootfs_count=$(find /run/kata-containers/shared/sandboxes/$sandbox_id -name rootfs -type d| grep -o "rootfs" | wc -l)
$ echo $rootfs_count
0
```