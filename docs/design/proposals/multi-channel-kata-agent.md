# Kata agent multi-channel ttrpc proposal

This proposal is a draft.

## Why

runtime-rs currently sends health, control, lifecycle, process I/O, and
diagnostic RPCs over one long-lived ttrpc connection to the Kata agent. It
clones that client for every agent and health call. The Go runtime uses an
equivalent single-connection design, but is outside the implementation scope of
this proposal.

### Current design

Both generated service clients share one underlying connection:

```text
+----------------------+      one ttrpc connection      +----------------------+
| Kata runtime         | =============================> | Kata agent           |
|                      |                                |                      |
| Health               |                                | Health service       |
| Control              |                                | AgentService         |
| Lifecycle            |                                |                      |
| Process I/O          |                                |                      |
| Diagnostics and bulk |                                |                      |
+----------------------+                                +----------------------+
                              shared reader,
                         writer, queues, and socket
```

The [runtime-rs `KataAgentInner`][runtime-rs-agent-client] stores one client and
clones it for both services. The deprecated
[Go `AgentClient`][go-agent-client] constructs its clients the same way and is
useful evidence of the shared-connection behavior, but does not need to
implement this proposal.

ttrpc multiplexes logical streams over a connection, but those streams still
share the connection's reader, writer, queues, socket, and failure state. A
stalled frame can therefore prevent unrelated RPCs from making transport
progress.

Two upstream issues describe concrete examples:

- [containerd/ttrpc issue 258][go-ttrpc-send-lock] describes how the Go
  client's send lock can keep later requests blocked behind a stalled write
  without observing their own contexts.
- [containerd/ttrpc-rust issue 317][rust-ttrpc-outbound-timeout] describes how
  outbound queue admission and socket writes are outside the async request
  timeout. It also covers the server's single writer and bounded response
  queue.

Kata has also seen starvation inside the agent:

- [Kata issue 13452][kata-stats-starvation] describes a blocking
  `StatsContainer` cgroup read holding the global `Sandbox` mutex and starving
  `SignalProcess`, leaving a pod stuck terminating.
- [Kata pull request 13491][kata-metrics-containment] contains a targeted
  mitigation for a different metrics path using blocking-work isolation,
  single-flight collection, caching, and runtime-side request limits.

These are complementary problems. Handler containment protects the agent
scheduler and shared state; separate connections prevent the same traffic from
also blocking unrelated calls at the transport layer.

Fixing ttrpc cancellation is still desirable, but it cannot make two frames on
one serial connection progress independently. Similarly, multiple connections
cannot fix the global mutex contention in issue 13452. Kata needs both
transport isolation and agent-side execution isolation.

The desired transport guarantee is:

> A blocked bulk, diagnostic, or process-I/O connection must not prevent
> health and process-control requests from reaching the agent.

## What

The runtime will open four independent ttrpc connections, called *lanes*, to
the existing agent address and port:

### Scope

The implementation targets runtime-rs. The Go runtime is deprecated for Kata
4.0 and will retain its existing single connection. Supporting the same lane
mapping in both runtimes is not a requirement.

### Control lane

Reserved for small, latency-sensitive calls:

- `Health.Check`
- `Health.Version`
- `AgentService.SignalProcess`
- `AgentService.CloseStdin`
- `AgentService.TtyWinResize`

### Process-I/O lane

Used for data-path and long-poll calls:

- `AgentService.WaitProcess`
- `AgentService.WriteStdin`
- `AgentService.ReadStdout`
- `AgentService.ReadStderr`
- `AgentService.GetOOMEvent`

### Bulk lane

Used for potentially large or diagnostic operations:

- `AgentService.RemoveStaleVirtiofsShareMounts`
- `AgentService.GetDiagnosticData`
- `AgentService.GetIPTables`
- `AgentService.SetIPTables`
- `AgentService.GetMetrics`
- `AgentService.GetGuestDetails`
- `AgentService.CopyFile`
- `AgentService.GetVolumeStats`
- `AgentService.SetPolicy`

### Management lane

Used for all remaining explicitly classified `AgentService` methods, including
container and sandbox lifecycle, networking, resource updates, storage, and
guest management.

The runtime-rs routing table must enumerate every protobuf RPC exactly once.
Adding an RPC without assigning a lane must fail validation rather than
silently selecting one.

The lanes are a quality-of-service mechanism, not a security boundary. Every
connection exposes the same services and all requests continue through the
same agent policy checks.

This proposal contains connection-level head-of-line blocking rather than
eliminating it. Calls within one lane can still block each other, and all lanes
still share the agent process, Tokio runtime, CPU, memory, policy engine, and
service state.

## How

### Connection topology

```text
+----------------------+          +----------------------+
| Kata runtime         |          | Kata agent           |
|                      |          |                      |
| control lane      +-----------------> connection       |
| management lane   +-----------------> connection       |
| process-I/O lane  +-----------------> connection       |
| bulk lane         +-----------------> connection       |
|                      |          |                      |
+----------------------+          | one ttrpc server     |
                                  +----------------------+
```

The current agent server accepts multiple connections on its existing
listener. Each connection receives independent transport state while exposing
the complete `Health` and `AgentService` services.

#### Multiple connections on one vsock port

A vsock port is a listener, similar to a TCP port. Several clients can connect
to the same guest CID and port because every connection has a different source
socket and is accepted independently:

```text
listener = vsock.listen(port=1024)

for lane in [control, management, process_io, bulk]:
    clients[lane] = ttrpc.Client(vsock.connect(guest_cid, 1024))

while socket = listener.accept():
    spawn ttrpc.serve(socket, Health, AgentService)
```

Each accepted socket has its own ttrpc reader, writer, queues, and stream-ID
namespace. Logical stream `1` can therefore exist on both the control and bulk
connections without conflict. A blocked writer on the bulk connection does
not block the control connection's writer, although both handlers still share
the agent runtime and service state.

The initial implementation requires:

- no protobuf changes;
- no new agent port;
- no Kata agent change;
- no ttrpc or ttrpc-rust change; and
- no Go runtime change.

### runtime-rs client

runtime-rs will introduce an `AgentClientSet` containing one ttrpc client and
generated service client per lane. The `KataAgentInner` method macro will
include the lane for each method and retrieve that client from the set. The
stdout and stderr methods will use the process-I/O client.

An explicit runtime-rs routing table will be the source of truth and must be
checked against the generated `Health` and `AgentService` methods.

### Admission and failure handling

Each lane will use independent, bounded, context-aware admission before calling
ttrpc. Admission must fail fast or use a bounded waiting queue, and waiting
must honor the request deadline. Capacity is never borrowed from the control
lane.

A failed request must not be silently retried on another lane because that
would reintroduce the dependency the design is intended to remove. Errors,
connection state, saturation, and reconnect activity must identify the lane.

The persistent connection set is initially created and closed together,
matching the current client lifecycle. Partial creation must close every
connection already opened. Independent lane reconnect can be considered later
once agent-restart and connection-generation semantics are defined.

Older runtime-rs versions and the Go runtime continue to use one connection. A
multi-channel runtime-rs client can connect several clients to an unchanged
agent listener. A temporary single-channel configuration may be retained for
transports that fail compatibility testing, but it must emit a warning because
isolation is lost.

### Trade-offs

- Replacing one connection with four consumes three additional socket file
  descriptors in both the runtime and agent for every sandbox. Hypervisors or
  socket proxies may allocate additional per-connection resources.
- Each connection adds reader and writer tasks, transport queues, stream
  bookkeeping, socket buffers, and shutdown state. Host and guest memory usage
  therefore increases.
- Additional vsock or hybrid-vsock handshakes may increase sandbox startup
  latency, and partial connection failures make setup and recovery more
  complex.
- Reserved capacity trades utilization for isolation. Control capacity may sit
  idle while the bulk lane is saturated, and head-of-line blocking still
  exists within each lane.
- Independent connections do not provide global submission ordering. Callers
  that require ordering must await prerequisites or synchronize explicitly.
- Health can succeed while another lane is unavailable, so diagnostics must
  represent partial availability instead of one agent-wide state.
- Every new RPC must be classified in runtime-rs, increasing routing,
  observability, and maintenance cost.
- More connections can expose more concurrent work to the shared agent.
  Bounded admission and agent-side execution isolation remain necessary.

## Alternatives

### Wait for upstream ttrpc fixes

Upstream fixes should be pursued, but dependency upgrades take time and cannot
allow independent frame progress on one connection.

### Increase timeouts or queue sizes

This delays or hides saturation without creating an independent control path
and can increase memory consumption.

### Isolate individual agent handlers

Moving synchronous handlers off Tokio workers and shortening lock scope is
necessary complementary work, but it cannot unblock a stalled ttrpc writer.

### Configure lanes dynamically

Instead of defining four fixed lanes, runtime configuration could define any
number of named lanes and map RPC methods to them:

```text
lanes:
  urgent:   [Health.Check, Health.Version, SignalProcess]
  io:       [ReadStdout, ReadStderr, WriteStdin]
  metrics:  [GetMetrics, StatsContainer]
  default:  ["*"]
```

Each configured lane would create one connection, allowing deployments to
trade isolation against file descriptors and memory. For example, a deployment
could give metrics a dedicated lane or use only control and default lanes.

This is more flexible but makes the availability guarantee dependent on local
configuration. The schema would need a maximum lane count, exactly one mapping
per RPC, a required default, bounded admission settings, and warnings when
critical and bulk methods share a lane. Arbitrary lane names also complicate
metrics, support, and compatibility testing. The fixed mapping is preferred
initially because it provides a predictable and testable control path.

### Open one connection per RPC

This gives stronger isolation but adds connection latency and churn, consumes
more resources under concurrency, and complicates long-running calls.

### Use separate agent ports

Separate listeners and runtimes could provide a stronger server-side control
boundary, but require agent configuration, port allocation, capability
negotiation, and compatibility handling.

[go-agent-client]: ../../../src/runtime/virtcontainers/pkg/agent/protocols/client/client.go
[go-ttrpc-send-lock]: https://github.com/containerd/ttrpc/issues/258
[kata-metrics-containment]: https://github.com/kata-containers/kata-containers/pull/13491
[kata-stats-starvation]: https://github.com/kata-containers/kata-containers/issues/13452
[runtime-rs-agent-client]: ../../../src/runtime-rs/crates/agent/src/kata/mod.rs
[rust-ttrpc-outbound-timeout]: https://github.com/containerd/ttrpc-rust/issues/317
