# Capacity-aware scheduling example

This example shows the per-node capacity pools and capacity-aware
scheduling of #14151 end to end: a `WaitForFirstConsumer` storage class
whose volumes are charged to a pool, and a node quota configured per
node so the scheduler places the pod where the pool has room.

It is the capacity-aware counterpart of `pod-with-directvol`, which
uses `Immediate` binding and no pool: neither of those is changed.

## Prerequisites

- the driver deployed as in [the deployment docs](../../docs/deploy-csi-kata-directvol.md)
  (needs Kubernetes 1.24+ for `CSIStorageCapacity`; the deployed
  DaemonSet mounts the `csi-kata-directvol-capacity` ConfigMap at
  `--capacity-config-dir`)
- a `kata` RuntimeClass (see `../pod-with-directvol/runtimeclass.yaml`)

## Walkthrough

1. Edit `capacity-configmap.yaml`: replace `worker-1` and `worker-2`
   with the names of two of your nodes (`kubectl get nodes`), keeping
   the asymmetric `gold=8Gi` / `gold=2Gi` values (edit those too, the
   point is: 4Gi fits on one node and not the other).

```shell
kubectl apply -f capacity-configmap.yaml
kubectl -n kata-directvolume rollout restart daemonset/csi-kata-directvol-plugin
kubectl -n kata-directvolume rollout status  daemonset/csi-kata-directvol-plugin
```

   The rollout restart is needed because the driver reads the
   ConfigMap once at startup.

2. Check that each node's capacity is published as
   `CSIStorageCapacity` objects (the csi-provisioner sidecar polls
   them from the driver's `GetCapacity`, by default once a minute):

```shell
kubectl get csistoragecapacity -n kata-directvolume -o yaml | less
```

   Each object names the node of its storage capacity in
   `nodeTopology` — one entry per populated node — so the two nodes
   of this example show up with the two different `gold` values.

3. Create the storage class, PVC and pod:

```shell
kubectl apply -f csi-storageclass.yaml
kubectl apply -f csi-pvc.yaml
kubectl apply -f csi-app.yaml
```

4. Verify capacity-aware placement — the pod can only be scheduled on
   the node whose `gold` pool covers the 4Gi PVC:

```shell
kubectl get pod kata-directvol-capacity-01 -o wide
kubectl get pvc csi-directvolume-gold-pvc
```

5. See pool draining: create more pods with a PVC of the same class.
   Once the sums reach the node's pool size, new PVCs stay Pending
   with `capacity`-related events, and creation on the driver side is
   rejected with `ResourceExhausted` instead of over-committing.

## Troubleshooting

- Pod stays Pending with "no capacity": check the
  1.24+/`storageCapacity` prerequisites, and that the rollout restart
  in step 1 ran the driver with your ConfigMap values.
- `kubectl -n kata-directvolume logs <plugin-pod> -c kata-directvolume`
  prints `loaded per-node capacity pools from ...` at startup when
  the node had an entry, and `no per-node capacity entry ...` when it
  did not.

## Teardown

```shell
kubectl delete -f csi-app.yaml
kubectl delete -f csi-pvc.yaml
kubectl delete -f csi-storageclass.yaml
kubectl -n kata-directvolume delete configmap csi-kata-directvol-capacity
```
