# Example drop-ins for genpolicy settings

Copy the drop-in file(s) you need into the `genpolicy-settings.d/` subdirectory next to your `genpolicy-settings.json`, then point `genpolicy -j` at the parent directory. For example:

```
my-settings/
  genpolicy-settings.json
  genpolicy-settings.d/
    10-non-coco-drop-in.json
    20-oci-1.2.1-drop-in.json
```

```sh
genpolicy -j my-settings/ ...
```

Each drop-in is an [RFC 6902 JSON Patch](https://datatracker.ietf.org/doc/html/rfc6902): a JSON array of operations (`add`, `remove`, `replace`, `move`, `copy`, `test`). Use `replace` for existing paths, `add` for new keys or array append (path ending in `/-`), and optional `test` to assert values before changing them.

Drop-ins are layered: `10-*` files set the platform base, `20-*` files overlay OCI version and other adjustments. You can combine multiple drop-ins (e.g. `10-non-coco-drop-in.json` + `20-oci-1.2.1-drop-in.json`).

| Drop-in file | Use case |
|--------------|----------|
| `10-non-coco-drop-in.json` | Non-confidential guest (e.g. standard VMs) |
| `10-non-coco-aks-drop-in.json` | Non-confidential guest on AKS |
| `10-non-coco-aks-cbl-mariner-drop-in.json` | Non-confidential guest on AKS with CBL-Mariner host |
| `20-oci-1.2.0-drop-in.json` | OCI bundle version 1.2.0 |
| `20-oci-1.2.1-drop-in.json` | OCI bundle version 1.2.1 (e.g. k3s, rke2, NVIDIA GPU, CBL-Mariner) |
| `20-oci-1.3.0-drop-in.json` | OCI bundle version 1.3.0 (e.g. containerd 2.2.x) |
| `20-experimental-force-guest-pull-drop-in.json` | Disable guest pull |

Request/exec overrides (e.g. allowing `kubectl exec` or specific ttRPC requests) are not shipped as drop-in examples; build your own drop-in or merge the needed `request_defaults` into a local file in `genpolicy-settings.d/`.
