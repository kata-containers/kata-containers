# Providing initdata from the node

Initdata is a document injected into a confidential guest at launch, carrying
configuration that the guest components need before any workload runs: the
`aa.toml` that tells the attestation agent where the Key Broker Service lives,
the `cdh.toml` for the Confidential Data Hub, and optionally a `policy.rego` for
the agent policy engine. Its integrity comes from the TEE launch measurement —
the runtime digests the document and binds that digest into the hardware launch
(`HOSTDATA` on AMD SEV-SNP, `MRCONFIGID` on Intel TDX, the sealed header under
IBM Secure Execution), and the attestation agent re-hashes the document it
receives and checks it against the report.

Normally each workload supplies its own initdata through the
`io.katacontainers.config.hypervisor.cc_init_data` pod annotation. That is the
right model for anything workload-specific, but it means every pod on a cluster
has to repeat the cluster's own configuration — the KBS endpoint above all — as a
base64-encoded, gzipped blob in its manifest.

`initdata_path` lets a node carry that shared part instead.

!!! info "Runtime support"
    `initdata_path` is read by `runtime-rs` only, and applies where init data is
    attached to the guest as a block device: QEMU and Cloud Hypervisor. Under
    the Go runtime, and on the remote hypervisor, init data still comes
    exclusively from the pod annotation.

## Pointing a runtime class at a document

Set `initdata_path` in the hypervisor table, through a drop-in file so the
setting survives kata-deploy upgrades (see
[Runtime Configuration](../runtime-configuration.md) for how drop-ins are
layered):

```toml title="/opt/kata/share/defaults/kata-containers/runtimes/qemu-snp/config.d/50-initdata.toml"
[hypervisor.qemu]
initdata_path = "/opt/kata/share/kata-containers/initdata.toml"
```

Every sandbox started with that configuration now gets the document, with no
annotation required. Because each runtime class reads its own configuration
file, this is also the granularity at which the setting applies: dropping it
into the `qemu-snp` configuration leaves other runtime classes untouched.

The file is read and parsed when the configuration is loaded, so a malformed
document is reported when the runtime starts rather than silently failing at the
first pod that would have used it.

!!! tip "Check the permissions when running rootless"
    A packed image is opened directly by the VMM, so with `rootless = true` — where
    the VMM runs as an unprivileged, randomly generated user — the file has to be
    readable by that user. World-readable (`0644`) is the simplest way to get
    there.

## The two accepted file forms

The path may point at either a plain initdata document or an already-packed
initdata disk image. Kata tells them apart by the magic number that starts a
packed image, so both work under the same configuration key.

=== "TOML document"

    A document is the easier form to own: it is readable, it shows up in a diff,
    and it can be dropped onto a node from a ConfigMap. Kata packs it into a
    disk image for each sandbox.

    ```toml title="/opt/kata/share/kata-containers/initdata.toml"
    version = "0.1.0"
    algorithm = "sha384"

    [data]
    "aa.toml" = '''
    [token_configs]
    [token_configs.coco_as]
    url = 'http://kbs-service.coco.svc.cluster.local:8080'

    [token_configs.kbs]
    url = 'http://kbs-service.coco.svc.cluster.local:8080'
    '''

    "cdh.toml" = '''
    socket = 'unix:///run/guest-services/cdh.sock'
    credentials = []

    [kbc]
    name = 'cc_kbc'
    url = 'http://kbs-service.coco.svc.cluster.local:8080'
    '''
    ```

=== "Packed image"

    A packed image is a versioned artifact: build it once, ship it with the rest
    of your node payload, and Kata attaches it to the guest exactly as it sits
    on disk without repacking anything per sandbox.

    Build one from a document in the form above with
    [`gen-initdata-image.sh`](https://github.com/kata-containers/kata-containers/blob/main/tools/packaging/scripts/gen-initdata-image.sh):

    ```bash
    ./tools/packaging/scripts/gen-initdata-image.sh \
        -o /opt/kata/share/kata-containers/initdata.img initdata.toml
    ```

    The script sources nothing, so it can be copied to a build machine on its
    own. Its output is reproducible: the same document always packs to the same
    bytes.

    Passing `-d` dumps the document back out of an image, which is the quickest
    way to confirm what a shipped artifact really contains:

    ```bash
    ./tools/packaging/scripts/gen-initdata-image.sh -d initdata.img | diff -u initdata.toml -
    ```

    If you would rather build the image from your own tooling, the layout is the
    8-byte magic `initdata`, an 8-byte little-endian payload length, the gzipped
    document, then zero padding up to a 512-byte sector boundary. The digest
    that gets bound into the launch measurement covers the document rather than
    the image, so the compression level you choose does not affect attestation.

    ```toml title="config.d/50-initdata.toml"
    [hypervisor.qemu]
    initdata_path = "/opt/kata/share/kata-containers/initdata.img"
    ```

## What happens when a workload also passes initdata

When a pod carries the `cc_init_data` annotation *and* the node has a document,
the annotation is overlaid on the node document entry by entry. A pod can
therefore add its own `policy.rego` while still inheriting the node's `aa.toml`,
or deliberately replace an entry it names.

| Node `initdata_path` | Pod annotation | Result |
| --- | --- | --- |
| unset | unset | No initdata is attached. |
| unset | set | The annotation is used exactly as given. |
| set | unset | The node document is used. |
| set | set | The annotation's entries are overlaid on the node document. |

The merged document also carries the annotation's `algorithm` and `version`,
since the algorithm is what selects the digest.

!!! warning "The merged document has its own digest"
    A merged document is a new document, so its digest differs from either
    input. Whatever verifies your guests has to expect the digest of the
    *merged* result. If you rely on precomputed digests, either keep the node
    document and pod annotations mutually exclusive, or make the node document
    authoritative as shown below.

## Making the node document authoritative

Merging is only reachable because the annotation is allowed in the first place.
Hypervisor annotations are gated by `enable_annotations`, so removing
`cc_init_data` from that list stops workloads from contributing initdata at all,
and leaves the node document as the only source:

```toml title="config.d/50-initdata.toml"
[hypervisor.qemu]
initdata_path = "/opt/kata/share/kata-containers/initdata.toml"
enable_annotations = ["kernel", "image", "initrd"]
```

!!! note
    `enable_annotations` is a list, and a drop-in replaces a list rather than
    appending to it. Re-state the annotations you still want to allow, taking
    the base configuration's value as your starting point.
