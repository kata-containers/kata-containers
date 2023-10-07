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

The `fio` test utilises the [fio tool](https://github.com/axboe/fio), configured
to perform measurements upon a single test file.

The test configuration used by the script can be modified by setting a number of
environment variables to change or over-ride the test defaults.
