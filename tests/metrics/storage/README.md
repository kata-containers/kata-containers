# Kata Containers storage I/O tests

The metrics tests in this directory are designed to be used to assess storage IO.

## `Blogbench` test

The `blogbench` script is based on the `blogbench` program which is designed to emulate a busy blog server with a number of concurrent 
threads performing a mixture of reads, writes and rewrites.

### Running the `blogbench` test

The `blogbench` test can be run by hand, for example:
```
$ cd metrics
$ bash storage/blogbench.sh
```
## `fio` test

The `fio` test utilizes the [fio tool](https://github.com/axboe/fio), configured to perform measurements upon a single test file.

The test spawns 8 jobs that exercise the I/O types `sequential read`, `random read`, `sequential write` and `random write`, while collecting
data using a block size of 4 Kb, an I/O depth of 2, and uses the `libaio` engine on a workload with a size of 10 gigabytes for a period of
10 seconds on each I/O type.

The results show the average bandwidth and average number of IOPS per I/O type in JSON format.

The `fio` test can be run by hand, for example:
```
$ cd metrics
$ bash storage/fio_test.sh
```
