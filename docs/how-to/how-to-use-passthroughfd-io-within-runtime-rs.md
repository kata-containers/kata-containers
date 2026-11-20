# How to Use Passthrough-FD IO within Runtime-rs

This document describes the Passthrough-FD (pass-fd) technology implemented in Kata Containers to optimize IO performance. By bypassing the intermediate proxy layers, this technology significantly reduces latency and CPU overhead for container IO streams.

## Important Limitation

Before diving into the technical details, please note the following restriction:

- Full FD Passthrough (recv-fd): This variant is currently implemented only for Kata Containers' built-in VMM, Dragonball, by extending its hybrid-vsock implementation with recv-fd support.
- UDS Pull (CLH): Cloud Hypervisor does not implement recv-fd, but supports the same `use_passfd_io` knob through a "UDS pull" mode: runtime-rs connects to the agent's vsock listener over the hybrid-vsock UDS, keeps the streams, and actively copies stdout/stderr (see below).
- Unsupported VMMs: QEMU and Firecracker do not support this feature at this time.

## Overview

The original IO implementation in Kata Containers suffered from an excessively long data path, leading to poor efficiency. For instance, copying a 10GB file could take as long as 10 minutes.

To address this, Kata AC member @lifupan and @frezcirno introduced a series of optimizations using passthrough-fd technology. This approach allows the VMM to directly handle file descriptors (FDs), dramatically improving IO throughput.

## Traditional IO Path

Before the introduction of Passthrough-FD, Kata's IO streams were implemented using `ttrpc + virtio-vsock`.

The data flow was as follows:

```mermaid
graph LR
    subgraph Host ["Host"]
        direction LR
        Containerd["Containerd"]

        subgraph KS ["kata-shim"]
            buffer(("buffer"))
        end

        Vsock["vsock"]

        subgraph VM ["vm"]
            Agent["kata-agent"]
            Container["container"]
        end
    end

    Containerd -->|stdin| buffer
    buffer --> Vsock
    Vsock --> Agent
    Agent -.-> Container

    %% Style Rendering
    style Host fill:#f0f8ff,stroke:#333,stroke-dasharray: 5 5
    style VM fill:#fff9c4,stroke:#e0e0e0
    style buffer fill:#c8e6c9,stroke:#ff9800,stroke-dasharray: 5 5
    style Vsock fill:#bbdefb,stroke:#2196f3
    style Containerd fill:#f5f5f5,stroke:#333
    style Agent fill:#fff,stroke:#333
    style Container fill:#fff,stroke:#333

```

The kata-shim (containerd-shim-kata-v2) on the Host opens the FIFO pipes provided by containerd via the shimv2 interface.
This results in three FDs (stdin, stdout, and stderr).
The kata-shim manages three separate threads to handle these streams.
The Bottleneck: kata-shim acts as a "middleman," maintaining three internal buffers. It must read data from the FDs into its own buffers before forwarding them via ttrpc over vsock to the destination.
This multi-threaded proxying and buffering in the shim layer introduced significant overhead.


## What is Passthrough-FD?

Passthrough-FD technology enhances the Dragonball VMM's hybrid-vsock implementation with support for recv-fd.

```mermaid
graph LR
    subgraph Host ["Host"]
        direction LR
        Containerd["Containerd"]

        Vsock["vsock"]

        subgraph VM ["vm"]
            Agent["kata-agent"]
            Container["container"]
        end
    end

    Containerd -->|stdin| Vsock
    Vsock --> Agent
    Agent -.-> Container

    %% Style Rendering
    style Host fill:#f0f8ff,stroke:#333,stroke-dasharray: 5 5
    style VM fill:#fff9c4,stroke:#e0e0e0
    style Vsock fill:#bbdefb,stroke:#2196f3
    style Containerd fill:#f5f5f5,stroke:#333
    style Agent fill:#fff,stroke:#333
    style Container fill:#fff,stroke:#333
```

Instead of requiring an intermediate layer to read and forward data, the hybrid-vsock module can now directly receive file descriptors from the Host. This allows the system to "pass through" the host's FDs directly to the kata-agent. By eliminating the proxying logic in kata-shim, the IO stream is effectively connected directly to the guest environment.

## Technical Details

The end-to-end process follows these steps:

```mermaid
sequenceDiagram
    autonumber

    box rgb(220,235,255) Guest (VM)
        participant Agent as kata-agent<br/>(Server)
        participant VSOCK as AF_VSOCK socket<br/>(Hybrid Vsock)
    end

    box rgb(255,240,220) Host
        participant Shim as kata-shim<br/>(Client)
        participant FIFO as File or FIFO<br/>(stdin/stdout/stderr)
    end

    Note over Agent: Agent Initialization:<br/>listen() on passfd_listener_port

    Shim->>FIFO: open() to acquire Fd<br/>(for stdin / stdout / stderr)

    Shim->>VSOCK: connect() + send("passfd\n")<br/>+ send_with_fd(Fd, PortA)

    Note over VSOCK,Agent: FD Transfer via Hybrid Vsock<br/>(repeat for stdin-port, stdout-port, stderr-port)

    VSOCK->>Agent: forward connection + Fd + PortA

    Agent->>Agent: accept() → get conn_fd + host-port<br/>save: map[host-port] = conn_fd<br/>(3 entries: stdin-port, stdout-port, stderr-port)

    Shim->>Agent: create_container RPC<br/>(includes stdin-port, stdout-port, stderr-port)

    Agent->>Agent: lookup map[stdin-port] → bind to container stdin<br/>lookup map[stdout-port] → bind to container stdout<br/>lookup map[stderr-port] → bind to container stderr

    Agent-->>Shim: create_container RPC response (OK)
```

1. Agent Initialization: The kata-agent starts a server listening on the port specified by passfd_listener_port.
2. FD Transfer: During the container creation phase, the kata-shim sends the FDs for stdin, stdout, and stderr to the Dragonball hybrid-vsock module using the sendfd mechanism.
3. Connection Establishment: Through hybrid-vsock, these FDs connect to the server started by the agent in Step 1.
4. Identification: The agent's server calls accept() to obtain the connection FD and a corresponding host-port. It saves the connection using the host-port as a unique identifier. At this stage, the agent has three established connections (identified by stdin-port, stdout-port, and stderr-port).
5. RPC Mapping: When kata-shim invokes the create_container RPC, it includes these three port identifiers in the request.
6. Final Binding: Upon receiving the RPC, the agent retrieves the saved connections using the provided ports and binds them directly to the container's standard IO streams.


## CLH: UDS Pull Mode (no recv-fd)

Cloud Hypervisor (CLH) does not implement the recv-fd extension of Dragonball's hybrid-vsock module, so host FDs cannot be transferred into the guest. The same `use_passfd_io` configuration instead enables a **UDS pull** mode on CLH:

```mermaid
sequenceDiagram
    autonumber

    box rgb(220,235,255) Guest (VM)
        participant Agent as kata-agent<br/>(Server)
        participant VSOCK as AF_VSOCK socket<br/>(Hybrid Vsock)
    end

    box rgb(255,240,220) Host
        participant Shim as kata-shim<br/>(Client)
        participant FIFO as File or FIFO<br/>(stdin/stdout/stderr)
    end

    Note over Agent: Agent Initialization:<br/>listen() on passfd_listener_port

    Shim->>VSOCK: connect() + send("connect port\n")
    Note over VSOCK,Agent: Connection Establishment over Hybrid Vsock<br/>(repeat for stdin-port, stdout-port, stderr-port)
    VSOCK->>Agent: forward connection
    Agent->>Agent: accept() → get host-port<br/>save: map[host-port] = stream
    Agent-->>Shim: echo host-port back over the connection

    Shim->>Agent: create_container RPC<br/>(includes stdin-port, stdout-port, stderr-port)
    Agent->>Agent: lookup map[stdin-port] → bind to container stdin<br/>lookup map[stdout-port] → bind to container stdout<br/>lookup map[stderr-port] → bind to container stderr
    Agent-->>Shim: create_container RPC response (OK)

    Note over Shim,FIFO: runtime-rs keeps the UDS streams<br/>and actively copies:<br/>stdout/stderr ← stream → FIFO<br/>stdin ← FIFO → stream
```

The differences with the Dragonball path are:

- No fd is transferred. runtime-rs simply connects to the agent listener over the host-side hybrid-vsock UDS (`<sandbox>/vsock.sock`, the same endpoint used for the agent ttrpc channel) using the standard `connect <port>` handshake.
- The agent echoes the host port back over the established connection, so runtime-rs learns the port to embed in the `CreateContainer`/`ExecProcess` RPC.
- runtime-rs keeps the three UDS streams and runs the usual `start_io_and_wait` copy loop between them and the containerd FIFOs. **stdout/stderr no longer go through the agent's `read_stdout`/`read_stderr` ttrpc calls**; only the control plane (create/start/exec/wait/signal) still uses ttrpc.


## How to enable PassthroughFD IO within Configuration?

The Passthrough-FD feature is controlled by two main parameters in the Kata configuration file:

- use_passfd_io: A boolean flag to enable or disable the Passthrough-FD IO feature.
- passfd_listener_port: Specifies the port on which the kata-agent listens for FD connections. The default value is 1027.
To enable Passthrough-FD IO, set use_passfd_io to true in the configuration file:

```toml
...
# If enabled, the runtime will attempt to use fd passthrough feature for process io.
# Note: this feature is supported by the Dragonball (fd passthrough) and
# Cloud Hypervisor (UDS pull) hypervisors.
use_passfd_io = true

# If fd passthrough io is enabled, the runtime will attempt to use the specified port instead of the default port.
passfd_listener_port = 1027
```
