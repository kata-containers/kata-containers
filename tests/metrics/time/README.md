# Kata Containers boot time metrics

The boot time metrics test takes a number of time measurements through the complete
launch/shutdown cycle of a single container.
From those measurements it derives a number of time measures, such as:
- time to payload execution
- time to get to VM kernel
- time in VM kernel boot
- time to quit
- total time (from launch to finished)

## Running the test

Boot time test can be run by hand, for example:

```
$ cd metrics
$ bash time/launch_times.sh -i public.ecr.aws/ubuntu/ubuntu:latest -n 1
```
