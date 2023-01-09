# Core scheduling

Core scheduling is a Linux kernel feature that allows only trusted tasks to run concurrently on
CPUs sharing compute resources (for example, hyper-threads on a core).

Containerd versions >= 1.6.4 leverage this to treat all of the processes associated with a
given pod or container to be a single group of trusted tasks. To indicate this should be carried
out, containerd sets the `SCHED_CORE` environment variable for each shim it spawns. When this is
set, the Kata Containers shim implementation uses the `prctl` syscall to create a new core scheduling
domain for the shim process itself as well as future VMM processes it will start.

For more details on the core scheduling feature, see the [Linux documentation](https://www.kernel.org/doc/html/latest/admin-guide/hw-vuln/core-scheduling.html).
