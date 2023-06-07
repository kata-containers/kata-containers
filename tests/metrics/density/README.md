# Kata Containers density metrics tests

This directory contains a number of tests to help measure container
memory footprint. Some measures are based around the
[PSS](https://en.wikipedia.org/wiki/Proportional_set_size) of the runtime
components, and others look at the system level (`free` and `/proc/meminfo`
for instance) impact.

## `memory_usage`

This test measures the PSS footprint of the runtime components whilst
launching a number of small ([BusyBox](https://hub.docker.com/_/busybox/)) containers
using ctr.

## `fast_footprint`

This test takes system level resource measurements after launching a number of
containers in parallel and optionally waiting for KSM to settle its memory
compaction cycles.

The script is quite configurable via environment variables, including:

* Which container workload to run.
* How many containers to launch.
* How many containers are launched in parallel.
* How long to wait until taking the measures.

See the script itself for more details.

This test shares many config options with the `footprint_data` test. Thus, referring
to the [footprint test documentation](footprint_data.md) may be useful.

> *Note:* If this test finds KSM is enabled on the host, it will wait for KSM
> to "settle" before taking the final measurement. If your KSM is not configured
> to process all the allocated VM memory fast enough, the test will hit a timeout
> and proceed to take the final measurement anyway.

## `footprint_data`

Similar to the `fast_footprint` test, but this test launches the containers
sequentially and takes a system level measurement between each launch. Thus,
this test provides finer grained information on system scaling, but takes
significantly longer to run than the `fast_footprint` test. If you are only
interested in the final figure or the average impact, you may be better running
the `fast_footprint` test.

For more details see the [footprint test documentation](footprint_data.md).

## `memory_usage_inside_container`

Measures the memory statistics *inside* the container. This allows evaluation of
the overhead the VM kernel and rootfs are having on the memory that was requested
by the container co-ordination system, and thus supplied to the VM.

## `k8s-sysbench`

Sysbench is an open-source and multi-purpose benchmark utility that evaluates parameters features
tests for CPU, memory and I/O. Currently the `k8s-sysbench` test is measuring
the CPU performance.
