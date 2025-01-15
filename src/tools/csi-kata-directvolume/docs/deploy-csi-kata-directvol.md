# Deploy Kata Direct Volume CSI and Do Validation

## How to Deploy Kata Direct Volume CSI

First, you need to make sure you have a healthy Kubernetes(1.20+) cluster and have the permissions to create Kata pods.

*WARNING* If you select a `K8S` with lower version, It cannot ensure that it will work well.

The `CSI driver` is deployed as a `daemonset` and the pods of the `daemonset` contain 4 containers:

1. `Kata Direct Volume CSI Driver`, which is the key implementation in it
2. [CSI-External-Provisioner](https://github.com/kubernetes-csi/external-provisioner)
3. [CSI-Liveness-Probe](https://github.com/kubernetes-csi/livenessprobe)
4. [CSI-Node-Driver-Registrar](https://github.com/kubernetes-csi/node-driver-registrar)

The easiest way to deploy the `Direct Volume CSI driver` is to run the `deploy.sh` script for the Kubernetes version used by
the cluster as shown below for Kubernetes 1.28.2.

```shell
$ ./deploy/deploy.sh
Creating Namespace kata-directvolume ...
kubectl apply -f /tmp/tmp.lAAPNQ1aI2/kata-directvol-ns.yaml
namespace/kata-directvolume created
Namespace kata-directvolume created Done !
Applying RBAC rules ...
curl https://raw.githubusercontent.com/kubernetes-csi/external-provisioner/v3.6.0/deploy/kubernetes/rbac.yaml --output /tmp/tmp.lAAPNQ1aI2/rbac.yaml --silent --location
kubectl apply -f ./deploy/kata-directvolume/kata-directvol-rbac.yaml
serviceaccount/csi-provisioner created
clusterrole.rbac.authorization.k8s.io/external-provisioner-runner created
clusterrolebinding.rbac.authorization.k8s.io/csi-provisioner-role created
role.rbac.authorization.k8s.io/external-provisioner-cfg created
rolebinding.rbac.authorization.k8s.io/csi-provisioner-role-cfg created
Applying RBAC rules Done!
deploying kata directvolume components
   ./deploy/kata-directvolume/csi-directvol-driverinfo.yaml
csidriver.storage.k8s.io/directvolume.csi.katacontainers.io created
   ./deploy/kata-directvolume/csi-directvol-plugin.yaml
kata-directvolume plugin        using           image: registry.k8s.io/sig-storage/csi-provisioner:v3.6.0
kata-directvolume plugin        using           image: registry.k8s.io/sig-storage/csi-node-driver-registrar:v2.9.0
kata-directvolume plugin        using           image: localhost/kata-directvolume:v1.0.19
kata-directvolume plugin        using           image: registry.k8s.io/sig-storage/livenessprobe:v2.8.0
daemonset.apps/csi-kata-directvol-plugin created
NAMESPACE           NAME                                                        READY   STATUS      RESTARTS       AGE
kata-directvolume   pod/csi-kata-directvol-plugin-9vvhc                         4/4     Running     0              3s
[...TRUNCATED...]

NAMESPACE           NAME                                        DESIRED   CURRENT   READY   UP-TO-DATE   AVAILABLE   NODE SELECTOR              AGE
kata-directvolume   daemonset.apps/csi-kata-directvol-plugin    1         1         1       1            1           <none>                     3s
[...TRUNCATED...]
```


## How to Run a Kata Pod and Validate it


First, ensure all expected containers are running properly:

```shell
$ kubectl get po -n kata-directvolume
NAME                              READY   STATUS    RESTARTS   AGE
csi-kata-directvol-plugin-9vvhc   4/4     Running   0          6m14s
```

Deploy the application pods including a storage class, a `PVC`, and a
pod which uses direct block device based volume:

```shell
$ cd src/tools/csi-kata-directvolume/examples/pod-with-directvol
$ kubectl apply -f csi-storageclass.yaml
$ kubectl apply -f csi-pvc.yaml
$ kubectl apply -f csi-app.yaml
```

Let's validate the components are deployed:

```shell
$ kubectl get po -A
NAMESPACE      NAME                              READY   STATUS    RESTARTS       AGE
kata-directvolume        csi-kata-directvol-plugin-dlphw   4/4     Running   0              68m
default        kata-driectvol-01                 1/1     Running   0              67m

$ kubectl get sc,pvc -A
NAME                                                   PROVISIONER                          RECLAIMPOLICY   VOLUMEBINDINGMODE   ALLOWVOLUMEEXPANSION   AGE
storageclass.storage.k8s.io/csi-kata-directvolume-sc   directvolume.csi.katacontainers.io   Delete          Immediate           false                  71m

NAMESPACE   NAME                                         STATUS   VOLUME                                     CAPACITY   ACCESS MODES   STORAGECLASS               AGE
default     persistentvolumeclaim/csi-directvolume-pvc   Bound    pvc-d7644547-f850-4bdf-8c93-aa745c7f31b5   1Gi        RWO            csi-kata-directvolume-sc   71m

```

Finally, inspect the application pod `kata-driectvol-01`  which running with direct block device based volume:

```shell
$ kubectl describe po kata-driectvol-01
Name:                kata-driectvol-01
Namespace:           kata-directvolume
Priority:            0
Runtime Class Name:  kata
Service Account:     default
Node:                node001/10.10.1.19
Start Time:          Sat, 09 Dec 2023 23:06:49 +0800
Labels:              <none>
Annotations:         <none>
Status:              Running
IP:                  10.244.0.232
IPs:
  IP:  10.244.0.232
Containers:
  first-container:
    Container ID:  containerd://c5eec9d645a67b982549321f382d83c56297d9a2a705857e8f3eaa6c6676908e
    Image:         ubuntu:22.04
    Image ID:      docker.io/library/ubuntu@sha256:2b7412e6465c3c7fc5bb21d3e6f1917c167358449fecac8176c6e496e5c1f05f
    Port:          <none>
    Host Port:     <none>
    Command:
      sleep
      1000000
    State:          Running
      Started:      Sat, 09 Dec 2023 23:06:51 +0800
    Ready:          True
    Restart Count:  0
    Environment:    <none>
    Mounts:
      /data from kata-driectvol0-volume (rw)
      /var/run/secrets/kubernetes.io/serviceaccount from kube-api-access-zs9tm (ro)
Conditions:
  Type              Status
  Initialized       True 
  Ready             True 
  ContainersReady   True 
  PodScheduled      True 
Volumes:
  kata-driectvol0-volume:
    Type:       PersistentVolumeClaim (a reference to a PersistentVolumeClaim in the same namespace)
    ClaimName:  csi-directvolume-pvc
    ReadOnly:   false
  kube-api-access-zs9tm:
    Type:                    Projected (a volume that contains injected data from multiple sources)
    TokenExpirationSeconds:  3607
    ConfigMapName:           kube-root-ca.crt
    ConfigMapOptional:       <nil>
    DownwardAPI:             true
QoS Class:                   BestEffort
Node-Selectors:              <none>
Tolerations:                 node.kubernetes.io/not-ready:NoExecute op=Exists for 300s
                             node.kubernetes.io/unreachable:NoExecute op=Exists for 300s
Events:                      <none>

```
