# NUMA Topology Check Container

Minimal container image that reads guest NUMA topology from sysfs and
prints structured output to stdout.  Used by `k8s-nvidia-numa.bats` to
verify guest NUMA node count, vCPU distribution, and memory layout
without needing `kubectl exec` (which requires CoCo policy overrides).

## Image

`quay.io/kata-containers/numa:<date>`

## Build and push (multi-arch)

```bash
cd tests/integration/kubernetes/runtimeclass_workloads/numa/

docker buildx build --platform linux/amd64,linux/arm64 \
    -t quay.io/kata-containers/numa:$(date +%Y-%m-%d) --push .
```

After pushing, update the image reference (including digest) in
`numa-topology-test.yaml.in`.

## Output format

The entrypoint prints one `key: value` pair per line:

```
numa_online: 0-1
node0_cpus: 32
node1_cpus: 32
node0_mem_kb: 37078332
node1_mem_kb: 37125524
```

The bats test parses this output from `kubectl logs`.
