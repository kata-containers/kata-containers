# Kata Containers iperf3 and nuttcp network metrics

Kata Containers provides a series of network performance tests. Running these provides
a basic reference for measuring  network essentials like bandwidth, jitter,
packet per second throughput, and latency.

## Performance tools

- iperf3 measures bandwidth and the quality of a network link.

- nuttcp determines the raw UDP layer throughput.

## Networking tests

- `network-metrics-iperf3.sh` measures bandwidth, jitter,
and packet-per-second throughput using iperf3 on single threaded connections. The
bandwidth test shows the speed of the data transfer. The jitter test measures the
variation in the delay of received packets. The packet-per-second tests show the
maximum number of (smallest sized) packets allowed through the transports.

- `network-metrics-nuttcp.sh` measures the UDP bandwidth using nuttcp. This tool
shows the speed of the data transfer for the UDP protocol.

- `network-metrics-iperf.sh` measures bidirectional bandwidth. Bidirectional tests
are used to test both servers for the maximum amount of throughput.
 
- `network-metrics-memory-pss.sh` measures the Proportional Set Size (PSS) of the hypervisor footprint on the host using smem
while running a transfer of one GB with nuttcp.

- `network-metrics-nginx-ab-benchmark.sh` uses an nginx container and runs the apache
benchmarking tool on the host to calculate the requests per second.

## Running the tests

Individual tests can be run by hand, for example:

```
$ cd metrics
$ bash network/network-metrics-nuttcp.sh
```

## Expected results

In order to obtain repeatable and stable results it is necessary to run the
tests multiple times (at least 15 times to have standard deviation < 3%).

> **NOTE** Networking tests results can vary between platforms and OS
> distributions.
