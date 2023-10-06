# Kata Containers network metrics

Kata Containers provides a series of network performance tests. Running these provides a basic reference for measuring network essentials like 
bandwidth, jitter, latency and parallel bandwidth.

## Performance tools

- `iperf3` measures bandwidth, jitter, CPU usage and the quality of a network link.

## Networking tests

- `k8s-network-metrics-iperf3.sh` measures bandwidth which is the speed of the data transfer.
- `latency-network.sh` measures network latency.

## Running the tests

Individual tests can be run by hand, for example:

``` 
$ cd metrics
$ bash network/iperf3_kubernetes/k8s-network-metrics-iperf3.sh -b
```
