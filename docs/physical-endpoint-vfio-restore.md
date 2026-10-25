# Physical VF leftover on vfio-pci after a failed Kata start

Branch: `restore-vf-host-driver-on-attach-failure`

This note records the leak we found, the restore path Kata already had, and
the gaps this change closes.

## Symptom

A Kata pod using an SR-IOV VF (cold-plug / physical endpoint) fails to start.
On the next attempt the same VF is still bound to `vfio-pci`. There is no
host netdev (`mlx5_core` is gone), so CNI / Kata cannot rediscover the
device. The VF stays unusable until a manual rebind or a reboot.

This showed up on DPU-host Kata workloads where the device plugin hands out
a PCI VF that is still a host netdev. Kata is the component that rebinds it
to `vfio-pci` for QEMU passthrough.

OVN-Kubernetes does not bind the VF to `vfio-pci` on this path. It should
not try to unbind it either: that would collide with real VFIO users
(KubeVirt).

## Who binds the VF

Sandbox create walks the pod netns, classifies a PCI netdev as a physical
endpoint, then attaches it:

```
CreateSandbox
  → createNetwork / addAllEndpoints
    → scanEndpointsInNs
      → addSingleEndpoint
           isPhysicalIface() == true
           createPhysicalEndpoint()   // saves BDF + current host driver (mlx5_core)
           endpoint.Attach()
             → bindNICToVFIO()
               → drivers.BindDevicetoVFIO(bdf, "mlx5_core")
```

`BindDevicetoVFIO` is the actual PCI rebind:

1. Write `vfio-pci` to `/sys/bus/pci/devices/<bdf>/driver_override`
2. Unbind from `mlx5_core` (host netdev disappears)
3. Write BDF to `/sys/bus/pci/drivers_probe` so `vfio-pci` claims it
4. Return `/dev/vfio/<iommu-group>` for QEMU

The same sequence exists in the Rust runtime (`bind_device_to_vfio`).

## Restore Kata already had

Kata already knows how to put the VF back. That primitive is `Detach`:

```go
func (endpoint *PhysicalEndpoint) Detach(...) error {
    return bindNICToHost(endpoint) // BindDevicetoHost(bdf, saved Driver)
}
```

`Detach` does **not** require the endpoint to be fully plugged into QEMU.
It only needs `BDF` and `Driver`, which `createPhysicalEndpoint` fills
**before** `Attach`. It also does not need a network namespace.

That primitive is already used from:

1. **Sandbox create failure** (`createSandboxFromConfig`): a `defer` calls
   `removeNetwork` if anything after network setup fails (VM start,
   containers, …).
2. **Sandbox stop**: `removeNetwork` → `RemoveEndpoints` → `Detach` for
   each recorded endpoint.

So a successful Attach followed by a later sandbox failure already
restores the VF.

`removeNetwork` / `RemoveEndpoints` are the **list** wrappers. They only
call `Detach` on endpoints stored in `n.eps`. That is the limitation.

`HotDetach` is a different API: it also removes the device from the
sandbox device manager. It is not valid if `AddDevice` never succeeded.

## The leak

The PCI bind happens **before** the endpoint is recorded:

```
bindNICToVFIO()          // VF is now vfio-pci
AddDevice() / cgroup     // can fail here
n.eps = append(...)      // only on success
```

If `AddDevice` (or cgroup setup) fails, `Attach` returns an error and the
endpoint is never appended to `n.eps`. The existing `removeNetwork` defer
still runs, but it walks an empty list and restores nothing.

A second hole **undoes** the existing restore: `scanEndpointsInNs` on a
later interface failure did `n.eps = n.eps[:epsBefore]` **without**
`Detach`. Endpoints that had already been attached were forgotten, so
the createSandbox defer could not see them either.

The same “not on `n.eps` yet” hole existed if rate-limiter setup failed
after a successful `Attach`.

CRI timeout / SIGKILL still bypasses every in-process path, including
the original defer. This change does not fix that.

## What we changed

### Go runtime — `physical_endpoint.go`

- `Attach` and `HotAttach` use a named return and a `defer`. After a
  successful `bindNICToVFIO`, any later error calls `endpoint.Detach`.
  That reuses the existing PCI restore instead of a parallel helper.
- `HotAttach` calls `Detach`, not `HotDetach`, because `HotDetach` talks
  to the device manager.
- Tests stub PCI rebind via unexported `bindToVFIO` / `bindToHost` fields
  on the endpoint under test, not package-level function variables.

### Go runtime — `network_linux.go`

- `addSingleEndpoint`: if rate-limiter setup fails after Attach, call
  `Detach` before returning. The endpoint is not on `n.eps` yet.
- `scanEndpointsInNs`: on any scan error, `Detach` endpoints added
  during this scan **before** truncating `n.eps`.
- Shared helper `detachEndpointBestEffort` so those two paths use the
  same `Detach` / `HotDetach` choice as teardown.

### Go runtime — `pkg/device/drivers/vfio.go`

There is no `Endpoint` object at this layer, only a BDF. If
`drivers_probe` or VFIO path lookup fails after the unbind, call
`BindDevicetoHost` (the same function `Detach` uses). Otherwise the VF
can sit unbound or on `vfio-pci` with no endpoint to roll back.

### Rust runtime — `physical_endpoint.rs` and `vfio.rs`

Same idea: after `bind_device_to_vfio`, if `get_vfio_device` or
`do_handle_device` fails, restore via `bind_device_to_host`. If the
sysfs probe write fails, restore there too.

The Rust `setup()` loop still does not detach already-attached siblings
if a later endpoint’s `attach()` fails. That is a smaller gap (typical
DPU Kata pods have one physical VF) and was left for a follow-up.

### Tests

- `physical_endpoint_test.go`
  - Attach restores via `Detach` when `AddDevice` fails
  - HotAttach does the same
  - Attach does **not** call restore when the VFIO bind itself fails
- `network_linux_test.go`
  - scan rollback Detaches a physical endpoint
  - scan rollback keeps endpoints that existed before this scan

Linux compile of `virtcontainers` and `pkg/device/drivers` was checked
from macOS with `GOOS=linux GOARCH=amd64 CGO_ENABLED=0 go test -c`.

## Files

| File | Why |
|---|---|
| `src/runtime/virtcontainers/physical_endpoint.go` | Restore on Attach/HotAttach failure using `Detach` |
| `src/runtime/virtcontainers/physical_endpoint_test.go` | Cover that restore |
| `src/runtime/virtcontainers/network_linux.go` | Detach before dropping `n.eps`; Detach after rate-limiter failure |
| `src/runtime/virtcontainers/network_linux_test.go` | Cover scan rollback |
| `src/runtime/pkg/device/drivers/vfio.go` | Restore if bind-to-vfio fails mid-sysfs |
| `src/runtime-rs/crates/resource/src/network/endpoint/physical_endpoint.rs` | Same Attach-failure restore |
| `src/runtime-rs/crates/hypervisor/src/device/driver/vfio.rs` | Same mid-bind restore |

## Still open

- **SIGKILL / CRI timeout:** `kata-runtime` is killed after the bind.
  Nothing in-process can run `Detach`. The VF stays on `vfio-pci` until
  a manual rebind, reboot, or a later safety net (for example DPU-host
  CNI ADD refusing a leftover VFIO device).
- **Rust multi-endpoint `setup()`:** first VF attached, second `attach()`
  fails; first VF is not detached in this patch.
- **Device plugin** still allocates VFs without checking the live driver.
  That is outside Kata.
