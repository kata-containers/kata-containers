# FIO in Kubernetes

This test run `fio` jobs to measure how Kata Containers work using virtio-fs
DAX. The test works using Kubernetes.  The test has to run in a single node
cluster, it is needed as the test modifies Kata configuration file.

The `virtio-fs` options that this test will use are:

* `cache mode`
Only `auto`, this is the most compatible mode for most of the Kata use cases. Today
this is default in Kata.

* `thread pool size`
Restrict the number of worker threads per request queue, zero means no thread pool.

* `DAX`
```
File contents can be mapped into a memory window on the host, allowing the
guest to directly access data from the host page cache. This has several
advantages:

The guest page cache is bypassed, reducing the memory footprint.  No
communication is necessary to access file contents, improving I/O performance.
Shared file access is coherent between virtual machines on the same host even
with mmap.
```

This test by default iterates over different `virtio-fs` configurations.

| test name                 | DAX | thread pool size | cache mode |
|---------------------------|-----|------------------|------------|
| pool_0_cache_auto_no_DAX  | no  | 0                | auto       |
| pool_0_cache_auto_DAX     | yes | 0                | auto       |


The `fio` options used are:

`ioengine`: How the IO requests are issued to the kernel.
* `libaio`: Supports async IO for both direct and buffered IO.
* `mmap`: File is memory mapped with mmap(2) and data copied to/from using memcpy(3).

`rw type`: Type of I/O pattern.
* `randread`: Random reads.
* `randrw`: Random mixed reads and writes.
* `randwrite`: Random writes.
* `read`: Sequential reads.
* `write`: Sequential writes.

Additional notes: 
Some jobs contain a `multi` prefix. This means that the same job runs more than
once at the same time using its own file.

### Static `fio` values:
Some `fio` values are not modified over all the jobs.

`runtime`: Tell `fio` to terminate processing after the specified period of
time(seconds).

`loops`: Run the specified number of iterations of this job. Used to repeat the
same workload a given number of times.

`iodepth`: Number of I/O units to keep in flight against the file. Note that
increasing `iodepth` beyond 1 will not affect synchronous `ioengine`.

`size`: The total size of file I/O for each thread of this job.

`direct`: If value is true, use non-buffered I/O. This is usually O_`DIRECT`.

`blocksize`: The block size in bytes used for I/O units
