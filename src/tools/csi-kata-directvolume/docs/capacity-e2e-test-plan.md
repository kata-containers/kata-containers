# CSI directvolume capacity-aware scheduling: E2E test plan

This is the execution plan for the two-node Kubernetes E2E test that
issue #14151 lists as a deliverable ("a reproducible E2E report"). The
driver-side work it validates is in PR #14254 (on top of #14253); the
unit and manifest layers are already covered in-tree by their test
suites, so everything here is what only a real cluster can prove:
scheduler filtering, published-object honesty, exhaustion, recovery,
and restart consistency.

The plan follows the issue's acceptance criteria section by section.
Each step lists the exact command and the assertion to make, so the
run can be executed by anyone (or scripted later) and the recorded
output becomes the E2E report verbatim. Where the criteria leave a
choice ("structured event reason where available"), the plan pins
one.

Relation to other test work: #14222 adds a kubectl-driven integration
test in `test/capacity_scheduling_integration_test.go` with the same
two-node shape. The intent is that this document is the full
acceptance matrix (including restart consistency and the negative
publication invariants, which that test does not cover), and the
script derives from it — landing the plan first lets reviewers agree
on the procedure before anyone spends cluster time on it.

## Test environment

| Component | Version |
|---|---|
| Kubernetes | v1.28.x (any 1.24+; 1.28 for the recorded report) |
| Nodes | 2 schedulable workers + 1 control-plane (kind or real) |
| Kata RuntimeClass | runtime-rs with QEMU (or Dragonball) |
| csi-kata-directvolume | built from this branch |
| external-provisioner | v3.6.0 (image pinned in the manifest) |
| Driver deploy | `deploy/deploy.sh` with `CAPACITY_POLL_INTERVAL=10s` |

Per-node capacity comes from the `csi-kata-directvol-capacity`
ConfigMap keyed by node name (the mechanism #14151 asks to be
documented). The values are the acceptance criteria's asymmetric
pair, using the pool name `gold`:

```text
node-a: gold=2Gi
node-b: gold=8Gi
```

Record in the report: Kata version, Kubernetes version, node names,
kernel, host distribution, and the commit under test.

## Setup

Build and load the driver image, deploy, label the two nodes, create
the ConfigMap, then verify the Basics before any workload:

```shell
# from src/tools/csi-kata-directvolume
make
kind load docker-image localhost/kata-directvolume:v1.0.19 --name <cluster>
CAPACITY_POLL_INTERVAL=10s ./deploy/directvol-deploy.sh        # or deploy.sh incl. RBAC

kubectl label node <node-a> capacity-test=small
kubectl label node <node-b> capacity-test=large

kubectl -n kata-directvolume create configmap csi-kata-directvol-capacity \
  --from-literal=<node-a>='gold=2Gi' --from-literal=<node-b>='gold=8Gi'
kubectl -n kata-directvolume rollout restart daemonset/csi-kata-directvol-plugin
kubectl -n kata-directvolume rollout status daemonset/csi-kata-directvol-plugin
```

Applying the ConfigMap files in `examples/pod-with-capacity/` (after
renaming the node keys) is equivalent; the `kubectl create` form only
avoids the rename.

For each worker, one readiness gate before proceeding:

```shell
kubectl -n kata-directvolume logs daemonset/csi-kata-directvol-plugin -c kata-directvolume \
  --field-selector spec.nodeName=<node> | grep "loaded per-node capacity"
```

`node-a` and `node-b` must each log `loaded per-node capacity pools
from /etc/csi-kata-directvolume/capacity: map[gold:2Gi]` (respectively
8Gi). If a node logs `no per-node capacity entry`, the rollout restart
did not take effect — resolve before continuing, do not re-trust
later assertions.

Create the test StorageClass (from `examples/pod-with-capacity/`, pool
renamed to `gold` or applied as-is if the ConfigMap used its name):

```shell
kubectl apply -f examples/pod-with-capacity/csi-storageclass.yaml
```

## §1 Capacity publication

**A. CSIDriver flag** becomes the first published sanity:

```shell
kubectl get csidriver directvolume.csi.katacontainers.io \
  -o jsonpath='{.spec.storageCapacity}{"\n"}'
```

Assert: `true`.

**B. One CSIStorageCapacity object per positive-capacity node**, and
its values:

```shell
kubectl get csistoragecapacity -n kata-directvolume | wc -l   # data rows = 2
kubectl get csistoragecapacity -n kata-directvolume -o custom-columns='NAME:.metadata.name,POOL:.storageClassName,CAPACITY:.capacity,TOPOLOGY:.nodeTopology'
```

Assert for each of the two nodes: `storageClassName` is the test
class (or its parameters match it), `nodeTopology` selects exactly
that one node, `capacity` is the ConfigMap value (2Gi / 8Gi — note
CSIStorageCapacity reports `available` here, the pool minus
reservations, initially the full value).

Wait bound: within 30s of the rollout completing.

**C. Invariants on values** (assert on the raw quantities):

```text
capacity >= 0
maximumVolumeSize >= 0, when present
maximumVolumeSize <= capacity, when both present
```

(These are the driver's clamping invariants from #14253, viewed
through the published objects: an unclamped negative or an
over-maximum value — the original bug class — would surface here
exactly.)

**D. Zero-capacity node semantics** (optional but cheap): set a third
key or temporary ConfigMap change giving some node `gold=0`, restart
rollout. Per the criteria, either an object with `capacity: 0` or the
object's disappearance is acceptable; record which one happens. This
also exercises ConfigMap-change-application in isolation.

## §2 Scheduler filtering

Create the 4Gi PVC and its consumer (use `examples/pod-with-capacity/`
PVC+app, or the same manifests with the pool renamed):

```shell
kubectl apply -f examples/pod-with-capacity/csi-pvc.yaml
kubectl apply -f examples/pod-with-capacity/csi-app.yaml
kubectl wait --for=jsonpath={.status.phase}=Bound pvc/csi-directvolume-gold-pvc --timeout=120s
kubectl wait --for=jsonpath={.status.phase}=Running pod/kata-directvol-capacity-01 --timeout=180s
```

Assert all four:

1. PVC `Bound`:
   `kubectl get pvc csi-directvolume-gold-pvc -o jsonpath='{.status.phase}'` → `Bound`
2. Pod `Running` **on node-b**:
   `kubectl get pod kata-directvol-capacity-01 -o jsonpath='{.spec.nodeName}'` → `<node-b>`
3. PV node affinity selects node-b:
   `kubectl get pv $(kubectl get pvc csi-directvolume-gold-pvc -o jsonpath='{.spec.volumeName}') -o jsonpath='{.spec.nodeAffinity}'` contains `<node-b>`, not `<node-a>`
4. **No CreateVolume on node-a**: the plugin logs on node-a contain no
   `createVolume` line for this volume, and its state dir / storage
   path contains no new volume directory:
   ```shell
   kubectl -n kata-directvolume logs daemonset/csi-kata-directvol-plugin -c kata-directvolume --field-selector spec.nodeName=<node-a> | grep -c "created volume"   # 0
   ```
   This is the "placement does not depend on failing CreateVolume
   attempts" criterion: one failed attempt may occur transiently, a
   bounded retry loop may not.

Scheduler event wording varies by Kubernetes version — assert on the
outcome and, where available, the event reason
(`kubectl get events --field-selector involvedObject.name=kata-directvol-capacity-01`),
not message text.

## §3 Exhaustion

While the 4Gi allocation is reserved, request 5Gi more in the same
pool:

Apply the same PVC/Pod manifests with name suffix `-five` and
`storage: 5Gi`.

- Wait one poll interval plus slack (≥20s) so the second object's
  residual is reflected.
- Assert the second pod stays `Pending` (phase `Pending`,
  `spec.nodeName` empty) and its PVC stays unbound.
- Assert **neither node** created backend storage: `created volume`
  count unchanged on both nodes, no new state entries. A node offered
  a `ResourceExhausted` CreateVolume here is a filtering failure —
  record it and stop; do not "pass with a failed attempt".
- Assert both published `CSIDriver` objects remain non-negative and
  the node-b object reads `4Gi` residual at most.

## §4 Capacity recovery

Delete the first workload and its claim, and wait for the full
reclaim chain (reclaim policy `Delete` — PV, then the driver's
`DeleteVolume`, then the published refresh):

```shell
kubectl delete -f examples/pod-with-capacity/csi-app.yaml
kubectl delete -f examples/pod-with-capacity/csi-pvc.yaml
kubectl wait --for=delete pvc/csi-directvolume-gold-pvc --timeout=60s
```

Within 30s after the driver logs the delete:

- node-b's published object recovers an amount covering 5Gi
  (approximately `8Gi` again, modulo other claims);
- the `claim-five-...` PVC becomes `Bound`;
- its pod becomes `Running` on node-b.

A concise way to read the driver's deletion timing:

```shell
kubectl -n kata-directvolume logs daemonset/csi-kata-directvol-plugin -c kata-directvolume --field-selector spec.nodeName=<node-b> --since=5m | grep -E "deleted direct volume|created volume"
```

## §5 Restart consistency

Precondition: at least one provisioned volume (recreate a small claim
from §2's manifests if the §4 sequence left none).

```shell
kubectl -n kata-directvolume rollout restart daemonset/csi-kata-directvol-plugin
kubectl -n kata-directvolume rollout status daemonset/csi-kata-directvol-plugin
```

After one poll interval + slack (≥20s):

1. **Persistent accounting**: node-b's published capacity is back to
   (pool − reserved) — with the surviving volume's size still
   deducted; the driver log shows `loaded per-node capacity` with the
   ConfigMap value (unchanged) and a state reload without duplicate
   entries.
2. **No double-count**: the residual drop equals the volume's size
   exactly once, e.g. `8Gi → 4Gi` for one 4Gi volume, not 0Gi.
3. **Volume reachable**: a pod using the surviving volume `Running`
   and can write (the direct-volume path asserts the `mountInfo.json`
   is intact in `/run/kata-containers/shared/direct-volumes/`).
4. **Not advertising free what is reserved**: the CSIStorageCapacity
   value must not jump up after restart (the pre-restart value minus
   reserved is the floor — restart reloads state, it does not reset
   it).

## Negative and environment checks (not in the criteria, cheap to add)

These catch regression classes this issue specifically called out:

- **Manifest-enablement regression**: run the §1.A/B checks
  immediately after a `deploy.sh` without `CAPACITY_POLL_INTERVAL`
  and with no ConfigMap — the DaemonSet must come up, and with
  `storageCapacity: true` on and GetCapacity reporting zero (the
  #14253 no-pool semantic), published objects show zero capacity, not
  a phantom 1TiB: this is the exact gap-2a default of pre-fix main
  that would otherwise let the scheduler route pods onto nodes with
  no room.
- **Malformed config fails loudly**: create a ConfigMap entry for a
  node whose value is `not-an-entry` (no `=` sign), restart: the plugin
  container on that node must fail to start (CrashLoopBackOff) with
  `Failed to load per-node capacity configuration` in logs, and other
  nodes are unaffected; then restore the good ConfigMap.
- **kind: loop the whole plan once on kind with a stock runtimeClass
  default (no Kata) for a scheduler-only dry run**: Volume deletion,
  publication, filtering all work without a guest. This isolates
  "capacity plumbing" failures from "Kata volume plumbing" ones —
  the plan's §1-§4 can be pre-flighted without Kata entirely.

## Report template

Record, in this order:

1. the environment table from the first section, filled in;
2. the test's commit SHA (branch of #14254) and the merge-base with
   #14253;
3. for each section §1-§5: the commands' raw output (or command +
   abbreviated output) and the assertion verdict;
4. notable observations: scheduler event reasons observed, node
   remainders converge timings, restart accounting details;
5. total wall-clock time and poll interval used.

This output, attached to the #14151 thread, is deliverable 3.
