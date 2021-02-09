use sys::*;

bitflags! {
    /// Signals that can be waited upon.
    ///
    /// See
    /// [Objects and signals](https://fuchsia.googlesource.com/zircon/+/master/docs/concepts.md#Objects-and-Signals)
    /// in the Zircon kernel documentation. Note: the names of signals are still in flux.
    #[repr(C)]
    pub struct Signals: zx_signals_t {
        const NONE          = ZX_SIGNAL_NONE;
        const OBJECT_ALL    = ZX_OBJECT_SIGNAL_ALL;
        const USER_ALL      = ZX_USER_SIGNAL_ALL;
        const OBJECT_0      = ZX_OBJECT_SIGNAL_0;
        const OBJECT_1      = ZX_OBJECT_SIGNAL_1;
        const OBJECT_2      = ZX_OBJECT_SIGNAL_2;
        const OBJECT_3      = ZX_OBJECT_SIGNAL_3;
        const OBJECT_4      = ZX_OBJECT_SIGNAL_4;
        const OBJECT_5      = ZX_OBJECT_SIGNAL_5;
        const OBJECT_6      = ZX_OBJECT_SIGNAL_6;
        const OBJECT_7      = ZX_OBJECT_SIGNAL_7;
        const OBJECT_8      = ZX_OBJECT_SIGNAL_8;
        const OBJECT_9      = ZX_OBJECT_SIGNAL_9;
        const OBJECT_10     = ZX_OBJECT_SIGNAL_10;
        const OBJECT_11     = ZX_OBJECT_SIGNAL_11;
        const OBJECT_12     = ZX_OBJECT_SIGNAL_12;
        const OBJECT_13     = ZX_OBJECT_SIGNAL_13;
        const OBJECT_14     = ZX_OBJECT_SIGNAL_14;
        const OBJECT_15     = ZX_OBJECT_SIGNAL_15;
        const OBJECT_16     = ZX_OBJECT_SIGNAL_16;
        const OBJECT_17     = ZX_OBJECT_SIGNAL_17;
        const OBJECT_18     = ZX_OBJECT_SIGNAL_18;
        const OBJECT_19     = ZX_OBJECT_SIGNAL_19;
        const OBJECT_20     = ZX_OBJECT_SIGNAL_20;
        const OBJECT_21     = ZX_OBJECT_SIGNAL_21;
        const OBJECT_22     = ZX_OBJECT_SIGNAL_22;
        const OBJECT_HANDLE_CLOSED = ZX_OBJECT_HANDLE_CLOSED;
        const USER_0        = ZX_USER_SIGNAL_0;
        const USER_1        = ZX_USER_SIGNAL_1;
        const USER_2        = ZX_USER_SIGNAL_2;
        const USER_3        = ZX_USER_SIGNAL_3;
        const USER_4        = ZX_USER_SIGNAL_4;
        const USER_5        = ZX_USER_SIGNAL_5;
        const USER_6        = ZX_USER_SIGNAL_6;
        const USER_7        = ZX_USER_SIGNAL_7;

        const OBJECT_READABLE    = ZX_OBJECT_READABLE;
        const OBJECT_WRITABLE    = ZX_OBJECT_WRITABLE;
        const OBJECT_PEER_CLOSED = ZX_OBJECT_PEER_CLOSED;

        // Cancelation (handle was closed while waiting with it)
        const HANDLE_CLOSED = ZX_SIGNAL_HANDLE_CLOSED;

        // Event
        const EVENT_SIGNALED = ZX_EVENT_SIGNALED;

        // EventPair
        const EVENT_PAIR_SIGNALED = ZX_EPAIR_SIGNALED;
        const EVENT_PAIR_CLOSED   = ZX_EPAIR_CLOSED;

        // Task signals (process, thread, job)
        const TASK_TERMINATED = ZX_TASK_TERMINATED;

        // Channel
        const CHANNEL_READABLE    = ZX_CHANNEL_READABLE;
        const CHANNEL_WRITABLE    = ZX_CHANNEL_WRITABLE;
        const CHANNEL_PEER_CLOSED = ZX_CHANNEL_PEER_CLOSED;

        // Socket
        const SOCKET_READABLE    = ZX_SOCKET_READABLE;
        const SOCKET_WRITABLE    = ZX_SOCKET_WRITABLE;
        const SOCKET_PEER_CLOSED = ZX_SOCKET_PEER_CLOSED;

        // Port
        const PORT_READABLE = ZX_PORT_READABLE;

        // Resource
        const RESOURCE_DESTROYED   = ZX_RESOURCE_DESTROYED;
        const RESOURCE_READABLE    = ZX_RESOURCE_READABLE;
        const RESOURCE_WRITABLE    = ZX_RESOURCE_WRITABLE;
        const RESOURCE_CHILD_ADDED = ZX_RESOURCE_CHILD_ADDED;

        // Fifo
        const FIFO_READABLE    = ZX_FIFO_READABLE;
        const FIFO_WRITABLE    = ZX_FIFO_WRITABLE;
        const FIFO_PEER_CLOSED = ZX_FIFO_PEER_CLOSED;

        // Job
        const JOB_NO_PROCESSES = ZX_JOB_NO_PROCESSES;
        const JOB_NO_JOBS      = ZX_JOB_NO_JOBS;

        // Process
        const PROCESS_TERMINATED = ZX_PROCESS_TERMINATED;

        // Thread
        const THREAD_TERMINATED = ZX_THREAD_TERMINATED;

        // Log
        const LOG_READABLE = ZX_LOG_READABLE;
        const LOG_WRITABLE = ZX_LOG_WRITABLE;

        // Timer
        const TIMER_SIGNALED = ZX_TIMER_SIGNALED;
    }
}