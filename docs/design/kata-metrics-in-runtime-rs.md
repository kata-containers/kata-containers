# Kata Metrics in Rust Runtime(runtime-rs)

Rust Runtime(runtime-rs) is responsible for:

- Gather metrics about `shim`.
- Gather metrics from `hypervisor` (through `channel`).
- Get metrics from `agent` (through `ttrpc`).

---

Here are listed all the metrics gathered by `runtime-rs`.

> * Current status of each entry is marked as:
>  * ✅：DONE
>   * 🚧：TODO

### Kata Shim

| STATUS | Metric name                                                  | Type        | Units          | Labels                                                       |
| ------ | ------------------------------------------------------------ | ----------- | -------------- | ------------------------------------------------------------ |
| 🚧      | `kata_shim_agent_rpc_durations_histogram_milliseconds`: <br> RPC latency distributions. | `HISTOGRAM` | `milliseconds` | <ul><li>`action` (RPC actions of Kata agent)<ul><li>`grpc.CheckRequest`</li><li>`grpc.CloseStdinRequest`</li><li>`grpc.CopyFileRequest`</li><li>`grpc.CreateContainerRequest`</li><li>`grpc.CreateSandboxRequest`</li><li>`grpc.DestroySandboxRequest`</li><li>`grpc.ExecProcessRequest`</li><li>`grpc.GetMetricsRequest`</li><li>`grpc.GuestDetailsRequest`</li><li>`grpc.ListInterfacesRequest`</li><li>`grpc.ListProcessesRequest`</li><li>`grpc.ListRoutesRequest`</li><li>`grpc.MemHotplugByProbeRequest`</li><li>`grpc.OnlineCPUMemRequest`</li><li>`grpc.PauseContainerRequest`</li><li>`grpc.RemoveContainerRequest`</li><li>`grpc.ReseedRandomDevRequest`</li><li>`grpc.ResumeContainerRequest`</li><li>`grpc.SetGuestDateTimeRequest`</li><li>`grpc.SignalProcessRequest`</li><li>`grpc.StartContainerRequest`</li><li>`grpc.StatsContainerRequest`</li><li>`grpc.TtyWinResizeRequest`</li><li>`grpc.UpdateContainerRequest`</li><li>`grpc.UpdateInterfaceRequest`</li><li>`grpc.UpdateRoutesRequest`</li><li>`grpc.WaitProcessRequest`</li><li>`grpc.WriteStreamRequest`</li></ul></li><li>`sandbox_id`</li></ul> |
| ✅      | `kata_shim_fds`: <br> Kata containerd shim v2 open FDs.      | `GAUGE`     |                | <ul><li>`sandbox_id`</li></ul>                               |
| ✅      | `kata_shim_io_stat`: <br> Kata containerd shim v2 process IO statistics. | `GAUGE`     |                | <ul><li>`item` (see `/proc/<pid>/io`)<ul><li>`cancelledwritebytes`</li><li>`rchar`</li><li>`readbytes`</li><li>`syscr`</li><li>`syscw`</li><li>`wchar`</li><li>`writebytes`</li></ul></li><li>`sandbox_id`</li></ul> |
| ✅      | `kata_shim_netdev`: <br> Kata containerd shim v2 network devices statistics. | `GAUGE`     |                | <ul><li>`interface` (network device name)</li><li>`item` (see `/proc/net/dev`)<ul><li>`recv_bytes`</li><li>`recv_compressed`</li><li>`recv_drop`</li><li>`recv_errs`</li><li>`recv_fifo`</li><li>`recv_frame`</li><li>`recv_multicast`</li><li>`recv_packets`</li><li>`sent_bytes`</li><li>`sent_carrier`</li><li>`sent_colls`</li><li>`sent_compressed`</li><li>`sent_drop`</li><li>`sent_errs`</li><li>`sent_fifo`</li><li>`sent_packets`</li></ul></li><li>`sandbox_id`</li></ul> |
| 🚧      | `kata_shim_pod_overhead_cpu`: <br> Kata Pod overhead for CPU resources(percent). | `GAUGE`     | percent        | <ul><li>`sandbox_id`</li></ul>                               |
| 🚧      | `kata_shim_pod_overhead_memory_in_bytes`: <br> Kata Pod overhead for memory resources(bytes). | `GAUGE`     | `bytes`        | <ul><li>`sandbox_id`</li></ul>                               |
| ✅      | `kata_shim_proc_stat`: <br> Kata containerd shim v2 process statistics. | `GAUGE`     |                | <ul><li>`item` (see `/proc/<pid>/stat`)<ul><li>`cstime`</li><li>`cutime`</li><li>`stime`</li><li>`utime`</li></ul></li><li>`sandbox_id`</li></ul> |
| ✅      | `kata_shim_proc_status`: <br> Kata containerd shim v2 process status. | `GAUGE`     |                | <ul><li>`item` (see `/proc/<pid>/status`)<ul><li>`hugetlbpages`</li><li>`nonvoluntary_ctxt_switches`</li><li>`rssanon`</li><li>`rssfile`</li><li>`rssshmem`</li><li>`vmdata`</li><li>`vmexe`</li><li>`vmhwm`</li><li>`vmlck`</li><li>`vmlib`</li><li>`vmpeak`</li><li>`vmpin`</li><li>`vmpmd`</li><li>`vmpte`</li><li>`vmrss`</li><li>`vmsize`</li><li>`vmstk`</li><li>`vmswap`</li><li>`voluntary_ctxt_switches`</li></ul></li><li>`sandbox_id`</li></ul> |
| 🚧      | `kata_shim_process_cpu_seconds_total`: <br> Total user and system CPU time spent in seconds. | `COUNTER`   | `seconds`      | <ul><li>`sandbox_id`</li></ul>                               |
| 🚧      | `kata_shim_process_max_fds`: <br> Maximum number of open file descriptors. | `GAUGE`     |                | <ul><li>`sandbox_id`</li></ul>                               |
| 🚧      | `kata_shim_process_open_fds`: <br> Number of open file descriptors. | `GAUGE`     |                | <ul><li>`sandbox_id`</li></ul>                               |
| 🚧      | `kata_shim_process_resident_memory_bytes`: <br> Resident memory size in bytes. | `GAUGE`     | `bytes`        | <ul><li>`sandbox_id`</li></ul>                               |
| 🚧      | `kata_shim_process_start_time_seconds`: <br> Start time of the process since `unix` epoch in seconds. | `GAUGE`     | `seconds`      | <ul><li>`sandbox_id`</li></ul>                               |
| 🚧      | `kata_shim_process_virtual_memory_bytes`: <br> Virtual memory size in bytes. | `GAUGE`     | `bytes`        | <ul><li>`sandbox_id`</li></ul>                               |
| 🚧      | `kata_shim_process_virtual_memory_max_bytes`: <br> Maximum amount of virtual memory available in bytes. | `GAUGE`     | `bytes`        | <ul><li>`sandbox_id`</li></ul>                               |
| 🚧      | `kata_shim_rpc_durations_histogram_milliseconds`: <br> RPC latency distributions. | `HISTOGRAM` | `milliseconds` | <ul><li>`action` (Kata shim v2 actions)<ul><li>`checkpoint`</li><li>`close_io`</li><li>`connect`</li><li>`create`</li><li>`delete`</li><li>`exec`</li><li>`kill`</li><li>`pause`</li><li>`pids`</li><li>`resize_pty`</li><li>`resume`</li><li>`shutdown`</li><li>`start`</li><li>`state`</li><li>`stats`</li><li>`update`</li><li>`wait`</li></ul></li><li>`sandbox_id`</li></ul> |
| ✅      | `kata_shim_threads`: <br> Kata containerd shim v2 process threads. | `GAUGE`     |                | <ul><li>`sandbox_id`</li></ul>                               |

### Kata Hypervisor

Different from golang runtime, hypervisor and shim in runtime-rs belong to the **same process**, so all previous metrics for hypervisor and shim only need to be gathered once. Thus, we currently only collect previous metrics in kata shim.

At the same time, we added the interface(`VmmAction::GetHypervisorMetrics`) to gather hypervisor metrics, in case we design tailor-made metrics for hypervisor in the future. Here're metrics exposed from [src/dragonball/src/metric.rs](https://github.com/kata-containers/kata-containers/blob/main/src/dragonball/src/metric.rs).

| Metric name                                                  | Type       | Units | Labels                                                       |
| ------------------------------------------------------------ | ---------- | ----- | ------------------------------------------------------------ |
| `kata_hypervisor_scrape_count`: <br> Metrics scrape count    | `COUNTER`  |       | <ul><li>`sandbox_id`</li></ul>                               |
| `kata_hypervisor_vcpu`: <br>Hypervisor metrics specific to VCPUs' mode of functioning. | `IntGauge` |       | <ul><li>`item`<ul><li>`exit_io_in`</li><li>`exit_io_out`</li><li>`exit_mmio_read`</li><li>`exit_mmio_write`</li><li>`failures`</li><li>`filter_cpuid`</li></ul></li><li>`sandbox_id`</li></ul> |
| `kata_hypervisor_seccomp`: <br> Hypervisor metrics for the seccomp filtering. | `IntGauge` |       | <ul><li>`item`<ul><li>`num_faults`</li></ul></li><li>`sandbox_id`</li></ul> |
| `kata_hypervisor_seccomp`: <br> Hypervisor metrics for the seccomp filtering. | `IntGauge` |       | <ul><li>`item`<ul><li>`sigbus`</li><li>`sigsegv`</li></ul></li><li>`sandbox_id`</li></ul> |
