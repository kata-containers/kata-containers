# Kata Containers and service mesh for Kubernetes

* [Assumptions](#assumptions)
* [How do they work?](#how-do-they-work)
* [Prerequisites](#prerequisites)
    * [Kata and Kubernetes](#kata-and-kubernetes)
    * [Restrictions](#restrictions)
* [Install and deploy your service mesh](#install-and-deploy-your-service-mesh)
    * [Istio](#istio)
    * [Conduit](#conduit)
* [Inject your services with sidecars](#inject-your-services-with-sidecars)
    * [Istio](#istio-1)
    * [Conduit](#conduit-1)
* [Run your services with Kata](#run-your-services-with-kata)
    * [Lower privileges](#lower-privileges)
    * [Add annotations](#add-annotations)
    * [Deploy!](#deploy)

A service mesh is a way to monitor and control the traffic between
micro-services running in your Kubernetes cluster. It is a powerful
tool that you might want to use in combination with the security
brought by Kata Containers.

## Assumptions

You are expected to be familiar with concepts such as __pods__,
__containers__, __control plane__, __data plane__, and __sidecar__.

## How do they work?

Istio and Conduit both rely on the same model, where they run controller
applications in the control plane, and inject a proxy as a sidecar inside
the pod running the service. The proxy registers in the control plane as
a first step, and it constantly sends different sorts of information about
the service running inside the pod. That information comes from the
filtering performed when receiving all the traffic initially intended for
the service. That is how the interaction between the control plane and the
proxy allows the user to apply load balancing and authentication rules to
the incoming and outgoing traffic, inside the cluster, and between multiple
micro-services.

This cannot not happen without a good amount of `iptables` rules ensuring
the packets reach the proxy instead of the expected service. Rules are
setup through an __init__ container because they have to be there as soon
as the proxy starts.

## Prerequisites

### Kata and Kubernetes

Follow the [instructions](https://github.com/kata-containers/documentation/blob/master/install/README.md)
to get Kata Containers properly installed and configured with Kubernetes.
You can choose between CRI-O and CRI-containerd, both are supported
through this document.

For both cases, select the workloads as _trusted_ by default. This way,
your cluster and your service mesh run with `runc`, and only the containers
you choose to annotate run with Kata Containers.

### Restrictions

As documented [here](https://github.com/runconduit/conduit/issues/982),
a kernel version between 4.14.22 and 4.14.40 causes a deadlock when
`getsockopt()` gets called with the `SO_ORIGINAL_DST` option. Unfortunately,
both service meshes use this system call with this same option from the
proxy container running inside the VM. This means that you cannot run
this kernel version range as the guest kernel for Kata if you want your
service mesh to work.

As mentioned when explaining the basic functioning of those service meshes,
`iptables` are heavily used, and they need to be properly enabled through
the guest kernel config. If they are not properly enabled, the init container
is not able to perform a proper setup of the rules.

## Install and deploy your service mesh

### Istio

As a reference, you can follow Istio [instructions](https://istio.io/docs/setup/kubernetes/quick-start/#download-and-prepare-for-the-installation).

The following is a summary of what you need to install Istio on your system:
```
$ curl -L https://git.io/getLatestIstio | sh -
$ cd istio-*
$ export PATH=$PWD/bin:$PATH
```

Now deploy Istio in the control plane of your cluster with the following:
```
$ kubectl apply -f install/kubernetes/istio-demo.yaml
```

To verify that the control plane is properly deployed, you can use both of
the following commands:
```
$ kubectl get svc -n istio-system
$ kubectl get pods -n istio-system -o wide
```

### Conduit

As a reference, follow the Conduit [instructions](https://conduit.io/getting-started/index.html).

The following is a summary of what you need to install Conduit on your system:
```
$ curl https://run.conduit.io/install | sh
$ export PATH=$PATH:$HOME/.conduit/bin
```

Now deploy Conduit in the control plane of your cluster with the following:
```
$ conduit install | kubectl apply -f -
```

To verify that the control plane is properly deployed, you can use both of
the following commands:
```
$ kubectl get svc -n conduit
$ kubectl get pods -n conduit -o wide
```

## Inject your services with sidecars

Once the control plane is running, you need a deployment to define a few
services that rely on each other. Then, you inject the YAML file with the
sidecar proxy using the tools provided by each service mesh.

If you do not have such a deployment ready, refer to the samples provided
by each project.

### Istio

Istio provides a [bookinfo](https://istio.io/docs/guides/bookinfo/)
sample, which you can rely on to inject their `envoy` proxy as a
sidecar.

You need to use their tool called `istioctl kube-inject` to inject
your YAML file. We use their [bookinfo](https://istio.io/docs/guides/bookinfo/)
sample as example:
```
$ istioctl kube-inject -f samples/bookinfo/kube/bookinfo.yaml -o bookinfo-injected.yaml
```

### Conduit

Conduit provides an [emojivoto](https://conduit.io/getting-started/index.html)
sample, which you can rely on to inject their `conduit` proxy as a
sidecar.

You need to use their tool called `conduit inject` to inject your YAML
file. We use their [emojivoto](https://conduit.io/getting-started/index.html)
sample as example:
```
$ wget https://raw.githubusercontent.com/runconduit/conduit-examples/master/emojivoto/emojivoto.yml
$ conduit inject emojivoto.yml > emojivoto-injected.yaml
```

## Run your services with Kata

Now that your service deployment is injected with the appropriate sidecar
containers, manually edit your deployment to make it work with Kata.

### Lower privileges

In Kubernetes, the __init__ container is often `privileged` as it needs to
setup the environment, which often needs some root privileges. In the case
of those services meshes, all they need is the `NET_ADMIN` capability to
modify the underlying network rules. Conduit, by default, does not use
`privileged` container, but Istio does.

Because of the previous reason, if you use Istio you need to switch all
containers with `privileged: true` to `privileged: false`.

### Add annotations

There is no difference between Istio and Conduit in this section. It is
about which CRI implementation you use.

For both CRI-O and CRI-containerd, you have to add an annotation indicating
the workload for this deployment is not _trusted_, which will trigger
`kata-runtime` to be called instead of `runc`.

__CRI-O:__

Add the following annotation for CRI-O
```yaml
io.kubernetes.cri-o.TrustedSandbox: "false"
```
The following is an example of what your YAML can look like: 

```yaml
...
apiVersion: extensions/v1beta1
kind: Deployment
metadata:
  creationTimestamp: null
  name: details-v1
spec:
  replicas: 1
  strategy: {}
  template:
    metadata:
      annotations:
        io.kubernetes.cri-o.TrustedSandbox: "false"
        sidecar.istio.io/status: '{"version":"55c9e544b52e1d4e45d18a58d0b34ba4b72531e45fb6d1572c77191422556ffc","initContainers":["istio-init"],"containers":["istio-proxy"],"volumes":["istio-envoy","istio-certs"],"imagePullSecrets":null}'
      creationTimestamp: null
      labels:
        app: details
        version: v1
...
```

__CRI-containerd:__

Add the following annotation for CRI-containerd
```yaml
io.kubernetes.cri.untrusted-workload: "true"
```
The following is an example of what your YAML can look like: 

```yaml
...
apiVersion: extensions/v1beta1
kind: Deployment
metadata:
  creationTimestamp: null
  name: details-v1
spec:
  replicas: 1
  strategy: {}
  template:
    metadata:
      annotations:
        io.kubernetes.cri.untrusted-workload: "true"
        sidecar.istio.io/status: '{"version":"55c9e544b52e1d4e45d18a58d0b34ba4b72531e45fb6d1572c77191422556ffc","initContainers":["istio-init"],"containers":["istio-proxy"],"volumes":["istio-envoy","istio-certs"],"imagePullSecrets":null}'
      creationTimestamp: null
      labels:
        app: details
        version: v1
...
```

### Deploy!

Deploy your application by using the following:
```
$ kubectl apply -f myapp-injected.yaml
```
