# Setting Sysctls with Kata

## Sysctls
In Linux, the sysctl interface allows an administrator to modify kernel 
parameters at runtime. Parameters are available via the `/proc/sys/` virtual 
process file system. 

The parameters include the following subsystems among others:
- `fs` (file systems)
- `kernel` (kernel)
- `net` (networking)
- `vm` (virtual memory)

To get a complete list of kernel parameters, run:
```
$ sudo sysctl -a
```

Both Docker and Kubernetes provide mechanisms for setting namespaced sysctls. 
Namespaced sysctls can be set per pod in the case of Kubernetes or per container
in case of Docker.
The following sysctls are known to be namespaced and can be set with 
Docker and Kubernetes:

- `kernel.shm*`
- `kernel.msg*`
- `kernel.sem`
- `fs.mqueue.*`
- `net.*`

### Namespaced Sysctls:

Kata Containers supports setting namespaced sysctls with Docker and Kubernetes.
All namespaced sysctls can be set in the same way as regular Linux based
containers, the difference being, in the case of Kata they are set inside the guest.

#### Setting Namespaced Sysctls with Docker:

```
$ sudo docker run --runtime=kata-runtime -it alpine cat /proc/sys/fs/mqueue/queues_max
256
$ sudo docker run --runtime=kata-runtime --sysctl fs.mqueue.queues_max=512 -it alpine cat /proc/sys/fs/mqueue/queues_max
512
```

... and:

```
$ sudo docker run --runtime=kata-runtime -it alpine cat /proc/sys/kernel/shmmax
18446744073692774399
$ sudo docker run --runtime=kata-runtime --sysctl kernel.shmmax=1024 -it alpine cat /proc/sys/kernel/shmmax
1024
```

For additional documentation on setting sysctls with Docker please refer to [Docker-sysctl-doc](https://docs.docker.com/engine/reference/commandline/run/#configure-namespaced-kernel-parameters-sysctls-at-runtime).


#### Setting Namespaced Sysctls with Kubernetes:

Kubernetes considers certain sysctls as safe and others as unsafe. For detailed
information about what sysctls are considered unsafe, please refer to the [Kubernetes sysctl docs](https://kubernetes.io/docs/tasks/administer-cluster/sysctl-cluster/).
For using unsafe sysctls, the cluster admin would need to allow these as:

```
$ kubelet --allowed-unsafe-sysctls 'kernel.msg*,net.ipv4.route.min_pmtu' ...
```

or using the declarative approach as:

```
$ cat kubeadm.yaml
apiVersion: kubeadm.k8s.io/v1alpha3
kind: InitConfiguration
nodeRegistration:
  kubeletExtraArgs:
    allowed-unsafe-sysctls: "kernel.msg*,kernel.shm.*,net.*"
...
```

The above YAML can then be passed to `kubeadm init` as:
```
$ sudo -E kubeadm init --config=kubeadm.yaml
```

Both safe and unsafe sysctls can be enabled in the same way in the Pod YAML:

```
apiVersion: v1
kind: Pod
metadata:
  name: sysctl-example
spec:
  securityContext:
    sysctls:
    - name: kernel.shm_rmid_forced
      value: "0"
    - name: net.ipv4.route.min_pmtu
      value: "1024"
```

### Non-Namespaced Sysctls:

Docker and Kubernetes disallow sysctls without a namespace.
The recommendation is to set them directly on the host or use a privileged
container in the case of Kubernetes.

In the case of Kata, the approach of setting sysctls on the host does not
work since the host sysctls have no effect on a Kata Container running 
inside a guest. Kata gives you the ability to set non-namespaced sysctls using a privileged container.
This has the advantage that the non-namespaced sysctls are set inside the guest 
without having any effect on the `/proc/sys` values of any other pod or the 
host itself. 

The recommended approach to do this would be to set the sysctl value in a
privileged init container. In this way, the application containers do not need any elevated 
privileges.

```
apiVersion: v1
kind: Pod
metadata:
  name: busybox-kata
spec:
  runtimeClassName: kata-qemu
  securityContext:
    sysctls:
    - name: kernel.shm_rmid_forced
      value: "0"
  containers:
  - name: busybox-container
    securityContext:
      privileged: true
    image: debian
    command:
        - sleep
        - "3000"
  initContainers:
  - name: init-sys
    securityContext:
      privileged: true
    image: busybox
    command: ['sh', '-c', 'echo "64000" > /proc/sys/vm/max_map_count']
```
