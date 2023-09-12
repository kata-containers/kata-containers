# Kata Containers Cassandra Metrics

Kata Containers provides a series of read and write performance tests using 
Cassandra Stress tool. The Cassandra Stress tool is a Java-based stress testing 
utility for basic benchmarking and load testing a cluster. This tool helps us 
to populate a cluster and stress test CQL tables and queries. This test is 
based in two operations, the first one is writing against the cluster or 
populating the database and the second one is reading the cluster that was 
populated by the writing test.

## Running the test

Individual tests can be run by hand, for example: 

``` 
$ cd metrics/disk/cassandra_kubernetes 
$ ./cassandra.sh 
```
