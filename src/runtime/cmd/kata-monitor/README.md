# Kata monitor

## Overview
`kata-monitor` is a daemon able to collect and expose metrics related to all the Kata Containers workloads running on the same host.
Once started, it detects all the running Kata Containers runtimes (`containerd-shim-kata-v2`) in the system and exposes few http endpoints to allow the retrieval of the available data.
The main endpoint is the `/metrics` one which aggregates metrics from all the kata workloads.
Available metrics include:
  * Kata runtime metrics
  * Kata agent metrics
  * Kata guest OS metrics
  * Hypervisor metrics
  * Firecracker metrics
  * Kata monitor metrics

All the provided metrics are in Prometheus format. While `kata-monitor` can be used as a standalone daemon on any host running Kata Containers workloads and can be used for retrieving profiling data from the running Kata runtimes, its main expected usage is to be deployed as a DaemonSet on a Kubernetes cluster: there Prometheus should scrape the metrics from the kata-monitor endpoints.
For more information on the Kata Containers metrics architecture and a detailed list of the available metrics provided by Kata monitor check the [Kata 2.0 Metrics Design](../../../../docs/design/kata-2-0-metrics.md) document.

## Local network considerations

The `kata-monitor` daemon is not run unless explicitly
[started](#kata-monitor-arguments). However, when it is running it
will accept connections on the `localhost` network interface (by
default) and provide metrics to any client process that connects to
it, whether they are privileged or not.

## Usage
Each `kata-monitor` instance detects and monitors the Kata Container workloads running on the same node.

### Kata monitor arguments
The `kata-monitor` binary accepts the following arguments:

* `--listen-address` _IP:PORT_
* `--runtime-enpoint` _PATH_TO_THE_CONTAINER_MANAGER_CRI_INTERFACE_
* `--log-level` _[ trace | debug | info | warn | error | fatal | panic ]_

The **listen-address** specifies the IP and TCP port where the kata-monitor HTTP endpoints will be exposed. It defaults to `127.0.0.1:8090`.

The **runtime-endpoint** is the CRI of a CRI compliant container manager: it will be used to retrieve the CRI `PodSandboxMetadata` (`uid`, `name` and `namespace`) which will be attached to the Kata metrics through the labels `cri_uid`, `cri_name` and `cri_namespace`. It defaults to the containerd socket: `/run/containerd/containerd.sock`.

The **log-level** allows the chose how verbose the logs should be. The default is `info`.
### Kata monitor HTTP endpoints
`kata-monitor` exposes the following endpoints:
  * `/metrics`             : get Kata sandboxes metrics.
  * `/sandboxes`           : list all the Kata sandboxes running on the host.
  * `/agent-url`           : Get the agent URL of a Kata sandbox.
  * `/debug/vars`          : Internal data of the Kata runtime shim.
  * `/debug/pprof/`        : Golang profiling data of the Kata runtime shim: index page.
  * `/debug/pprof/cmdline` : Golang profiling data of the Kata runtime shim: `cmdline` endpoint.
  * `/debug/pprof/profile` : Golang profiling data of the Kata runtime shim: `profile` endpoint (CPU profiling).
  * `/debug/pprof/symbol`  : Golang profiling data of the Kata runtime shim: `symbol` endpoint.
  * `/debug/pprof/trace`   : Golang profiling data of the Kata runtime shim: `trace` endpoint.

**NOTE: The debug endpoints are available only if the [Kata Containers configuration file](https://github.com/kata-containers/kata-containers/blob/9d5b03a1b70bbd175237ec4b9f821d6ccee0a1f6/src/runtime/config/configuration-qemu.toml.in#L590-L592) includes** `enable_pprof = true` **in the** `[runtime]` **section**.

The `/metrics` has a query parameter `filter_family`, which filter Kata sandboxes metrics with specific names. If `filter_family` is set to `A` (and `B`, split with `,`), metrics with prefix `A` (and `B`) will only be returned.

The `/sandboxes` endpoint lists the _sandbox ID_ of all the detected Kata runtimes. If accessed via a web browser, it provides html links to the endpoints available for each sandbox.

In order to retrieve data for a specific Kata workload, the _sandbox ID_ should be passed in the query string using the _sandbox_ key. The `/agent-url`, and all the `/debug/`* endpoints require `sandbox_id` to be specified in the query string.
<br>
#### Examples
Retrieve the IDs of the available sandboxes:
```bash
$ curl 127.0.0.1:8090/sandboxes
```
output:
```
6fcf0a90b01e90d8747177aa466c3462d02e02a878bc393649df83d4c314af0c
df96b24bd49ec437c872c1a758edc084121d607ce1242ff5d2263a0e1b693343
```
Retrieve the `agent-url` of the sandbox with ID _df96b24bd49ec437c872c1a758edc084121d607ce1242ff5d2263a0e1b693343_:
```bash
$ curl 127.0.0.1:8090/agent-url?sandbox=df96b24bd49ec437c872c1a758edc084121d607ce1242ff5d2263a0e1b693343
```
output:
```
vsock://830455376:1024
```
