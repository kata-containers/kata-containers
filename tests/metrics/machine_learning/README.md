# Kata Containers Tensorflow Metrics

Kata Containers provides a series of performance tests using the
TensorFlow reference benchmarks (tf_cnn_benchmarks).
The tf_cnn_benchmarks containers TensorFlow implementations of several
popular convolutional models.

## Running the test

Individual tests can be run by hand, for example:

```
$ cd metrics/machine_learning
$ ./tensorflow.sh 25 60
```
# Kata Containers Pytorch Metrics

Kata Containers provides a series of performance tests using Pytorch
benchmarks based on a suite of Python high performance computing
benchmarks.

## Running the Pytorch test

Individual tests can be run by hand, for example:

```
$ cd metrics/machine_learning
$ ./tensorflow.sh 40 100
```
# Kata Containers Tensorflow `MobileNet` Metrics

`MobileNets` are small, low-latency, low-power models parameterized to meet the resource 
constraints of a variety of use cases. They can be built upon for classification, detection, 
embeddings and segmentation similar to how other popular large scale models, such as Inception, are used. 
`MobileNets` can be run efficiently on mobile devices with `Tensorflow` Lite.

Kata Containers provides a test for running `MobileNet V1` inference using Intel-Optimized `Tensorflow`.

## Running the `Tensorflow` `MobileNet` test
Individual test can be run by hand, for example:

```
$ cd metrics/machine_learning
$ ./tensorflow_mobilenet_benchmark.sh 25 60
```

