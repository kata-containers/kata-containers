# Kata Tracing proposals

## Overview

This document summarises a set of proposals triggered by the
[tracing documentation PR][tracing-doc-pr].

## Required context

This section explains some terminology required to understand the proposals.
Further details can be found in the
[tracing documentation PR][tracing-doc-pr].

### Agent trace mode terminology

| Trace mode | Description | Use-case |
|-|-|-|
| Static |  Trace agent from startup to shutdown | Entire lifespan |
| Dynamic | Toggle tracing on/off as desired | On-demand "snapshot" |

### Agent trace type terminology

| Trace type | Description | Use-case |
|-|-|-|
| isolated | traces all relate to single component | Observing lifespan |
| collated | traces "grouped" (runtime+agent) | Understanding component interaction |

### Container lifespan

| Lifespan | trace mode | trace type |
|-|-|-|
| short-lived | static | collated if possible, else isolated? |
| long-running | dynamic | collated? (to see interactions) |

## Original plan for agent

- Implement all trace types and trace modes for agent.

- Why?
  - Maximum flexibility.

    > **Counterargument:**
    >
    > Due to the intrusive nature of adding tracing, we have
    > learnt that landing small incremental changes is simpler and quicker!

  - Compatibility with [Kata 1.x tracing][kata-1x-tracing].

    > **Counterargument:**
    >
    > Agent tracing in Kata 1.x was extremely awkward to setup (to the extent
    > that it's unclear how many users actually used it!)
    >
    > This point, coupled with the new architecture for Kata 2.x, suggests
    > that we may not need to supply the same set of tracing features (in fact
    > they may not make sense)).

## Agent tracing proposals

### Agent tracing proposal 1: Don't implement dynamic trace mode

- All tracing will be static.

- Why?
  - Because dynamic tracing will always be "partial"

    > In fact, not only would it be only a "snapshot" of activity, it may not
    > even be possible to create a complete "trace transaction". If this is
    > true, the trace output would be partial and would appear "unstructured".

### Agent tracing proposal 2: Simplify handling of trace type

- Agent tracing will be "isolated" by default.
- Agent tracing will be "collated" if runtime tracing is also enabled.

- Why?
  - Offers a graceful fallback for agent tracing if runtime tracing disabled.
  - Simpler code!

## Questions to ask yourself (part 1)

- Are your containers long-running or short-lived?

- Would you ever need to turn on tracing "briefly"?
  - If "yes", is a "partial trace" useful or useless?

    > Likely to be considered useless as it is a partial snapshot.
    > Alternative tracing methods may be more appropriate to dynamic
    > OpenTelemetry tracing.

## Questions to ask yourself (part 2)

- Are you happy to stop a container to enable tracing?
  If "no", dynamic tracing may be required.

- Would you ever want to trace the agent and the runtime "in isolation" at the
  same time?
  - If "yes", we need to fully implement `trace_mode=isolated`

    > This seems unlikely though.

## Trace collection

The second set of proposals affect the way traces are collected.

### Motivation

Currently:

- The runtime sends trace spans to Jaeger directly.
- The agent will send trace spans to the [`trace-forwarder`][trace-forwarder] component.
- The trace forwarder will send trace spans to Jaeger.

Kata agent tracing overview:

```
+-------------------------------------------+
| Host                                      |
|                                           |
| +-----------+                             |
| | Trace     |                             |
| | Collector |                             |
| +-----+-----+                             |
|       ^                  +--------------+ |
|       | spans            | Kata VM      | |
| +-----+-----+            |              | |
| | Kata      |    spans   |     +-----+  | |
| | Trace     |<-----------------|Kata |  | |
| | Forwarder |    VSOCK   |     |Agent|  | |
| +-----------+    Channel |     +-----+  | |
|                          +--------------+ |
+-------------------------------------------+
```

Currently:

- If agent tracing is enabled but the trace forwarder is not running,
  the agent will error.

- If the trace forwarder is started but Jaeger is not running,
  the trace forwarder will error.

### Goals

- The runtime and agent should:
  - Use the same trace collection implementation.
  - Use the most the common configuration items.

- Kata should should support more trace collection software or `SaaS`
  (for example `Zipkin`, `datadog`).

- Trace collection should not block normal runtime/agent operations
  (for example if `vsock-exporter`/Jaeger is not running, Kata Containers should work normally).

### Trace collection proposals

#### Trace collection proposal 1: Send all spans to the trace forwarder as a span proxy

Kata runtime/agent all send spans to trace forwarder, and the trace forwarder,
acting as a tracing proxy, sends all spans to a tracing back-end, such as Jaeger or `datadog`.

**Pros:**

- Runtime/agent will be simple.
- Could update trace collection target while Kata Containers are running.

**Cons:**

- Requires the trace forwarder component to be running (that is a pressure to operation).

#### Trace collection proposal 2: Send spans to collector directly from runtime/agent

Send spans to collector directly from runtime/agent, this proposal need
network accessible to the collector.

**Pros:**

- No additional trace forwarder component needed.

**Cons:**

- Need more code/configuration to support all trace collectors.

## Future work

- We could add dynamic and fully isolated tracing at a later stage,
  if required.

## Further details

- See the new [GitHub project](https://github.com/orgs/kata-containers/projects/28).
- [kata-containers-tracing-status](https://gist.github.com/jodh-intel/0ee54d41d2a803ba761e166136b42277) gist.
- [tracing documentation PR][tracing-doc-pr].

## Summary

### Time line

- 2021-07-01: A summary of the discussion was
  [posted to the mail list](http://lists.katacontainers.io/pipermail/kata-dev/2021-July/001996.html).
- 2021-06-22: These proposals were
  [discussed in the Kata Architecture Committee meeting](https://etherpad.opendev.org/p/Kata_Containers_2021_Architecture_Committee_Mtgs).
- 2021-06-18: These proposals where
  [announced on the mailing list](http://lists.katacontainers.io/pipermail/kata-dev/2021-June/001980.html).

### Outcome

- Nobody opposed the agent proposals, so they are being implemented.
- The trace collection proposals are still being considered.

[kata-1x-tracing]: https://github.com/kata-containers/agent/blob/master/TRACING.md
[trace-forwarder]: /src/tools/trace-forwarder
[tracing-doc-pr]: https://github.com/kata-containers/kata-containers/pull/1937
