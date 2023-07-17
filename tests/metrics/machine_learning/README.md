# Kata Containers Tensorflow Metrics

Kata Containers provides a series of performance tests using the
TensorFlow reference benchmarks (tf_cnn_benchmarks).
The tf_cnn_benchmarks containers TensorFlow implementations of several
popular convolutional models https://github.com/tensorflow/benchmarks/tree/master/scripts/tf_cnn_benchmarks.

Currently the TensorFlow benchmark on Kata Containers includes test for
the `AxelNet` and `ResNet50` models.

## Running the test

Individual tests can be run by hand, for example:

```
$ cd metrics/machine_learning
$ ./tensorflow.sh 25 60
```
# Kata Containers Pytorch Metrics

Based on a suite of Python high performance computing benchmarks that
uses various popular Python HPC libraries using Python
 https://github.com/dionhaefner/pyhpc-benchmarks.

## Running the Pytorch test

Individual tests can be run by hand, for example:

```
$ cd metrics/machine_learning
$ ./tensorflow.sh 40 100
```
