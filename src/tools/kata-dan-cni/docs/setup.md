# Install with one command

kata-dan-cni provides an installation script and uses `host-tap` by default. Complete the [prerequisites](prepare.md) before installing.

## Install with the script

Run the following command from the kata-dan-cni directory:

```bash
bash deploy/install.sh
```

Check the installation status:

```bash
kubectl -n kata-dan-system get pods -o wide
```

The `kata-dan-cni` Pods should be in the `Running` phase.

## CNI configuration

After `install.sh` runs, each node has a configuration similar to the following in `/etc/cni/net.d/01-kata-dan.conflist`. The installer fills in `nodeName` with that node's name.

```json
{
  "cniVersion": "0.3.1",
  "name": "kata-dan",
  "plugins": [
    {
      "type": "kata-dan-cni",
      "capabilities": {
        "io.kubernetes.cri.pod-annotations": true
      },
      "danRuntimeClass": "kata-qemu-runtime-rs",
      "backend": {
        "type": "host-tap"
      },
      "danConfDir": "/run/kata-containers/dans",
      "kubeconfig": "/etc/kata-dan-cni/kubeconfig",
      "nodeName": "worker-1",
      "delegate": {
        "name": "kube-ovn",
        "plugins": [
          {
            "type": "kube-ovn",
            "server_socket": "/run/openvswitch/kube-ovn-daemon.sock"
          }
        ]
      }
    },
    {
      "type": "portmap",
      "capabilities": {
        "portMappings": true
      }
    }
  ]
}
```

+ `delegate`: The Kube-OVN CNI configuration used for ordinary Pods.

+ `danConfDir`: The directory for DAN JSON files. It must match Kata's `runtime.dan_conf`.

+ `danRuntimeClass`: The RuntimeClass name for the Kata runtime. Defaults to `kata-qemu-runtime-rs`.

+ `backend.type`: Set to `host-tap`.

## Example Pod configuration

Example `pod.yaml`:

```yaml
apiVersion: v1
kind: Pod
metadata:
  name: kata-dan-example
  namespace: default
  annotations:
    networking.katacontainers.io/dan: "true"
spec:
  runtimeClassName: kata-qemu-runtime-rs
  containers:
    - name: shell
      image: busybox:1.36.1
      command: ["sh", "-c", "sleep 3600"]
```

- `networking.katacontainers.io/dan: "true"`: Enables DAN networking.
- `runtimeClassName: kata-qemu-runtime-rs`: Selects the Kata runtime for the Pod. It must match `danRuntimeClass` in the CNI configuration.

## Uninstall

```bash
bash deploy/uninstall.sh
```
