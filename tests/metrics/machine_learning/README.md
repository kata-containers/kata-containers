# Kata Containers TensorFlow Metrics

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
$ ./tensorflow_nhwc.sh 25 60
```
# Kata Containers Pytorch Metrics

Based on a suite of Python high performance computing benchmarks that
uses various popular Python HPC libraries using Python
 https://github.com/dionhaefner/pyhpc-benchmarks.

## Running the Pytorch test

Individual tests can be run by hand, for example:

```
$ cd metrics/machine_learning
$ ./pytorch.sh 40 100
```
# Kata Containers TensorFlow `MobileNet` Metrics

`MobileNets` are small, low-latency, low-power models parameterized to meet the resource 
constraints of a variety of use cases. They can be built upon for classification, detection, 
embeddings and segmentation similar to how other popular large scale models, such as Inception, are used. 
`MobileNets` can be run efficiently on mobile devices with `Tensorflow` Lite.

Kata Containers provides a test for running `MobileNet V1` inference using Intel-Optimized `TensorFlow`.

## Running the `TensorFlow` `MobileNet` test
Individual test can be run by hand, for example:

```
$ cd metrics/machine_learning
$ ./tensorflow_mobilenet_benchmark.sh 25 60
```

# Kata Containers TensorFlow `ResNet50` Metrics

`ResNet50` is an image classification model pre-trained on the `ImageNet` dataset.
Kata Containers provides a test for running `ResNet50` inference using Intel-Optimized
`TensorFlow`.

## Running the `TensorFlow` `ResNet50` test
Individual test can be run by hand, for example:

```
$ cd metrics/machine_learning
$ ./tensorflow_resnet50_int8.sh 25 60
```

# Kata Containers OpenVINO Benchmark

This is a toolkit around neural networks using its built-in benchmarking support
and analyzing the throughput and latency for various models.

## Running the `OpenVINO` test
Individual test can be run by hand, for example:

```
$ cd metrics/machine_learning
$ ./openvino.sh
```

# Kata Containers `oneDNN` Benchmark

This is a test of the Intel `oneDNN` as an Intel optimized library for Deep Neural Networks
and making use of its built-in `benchdnn` functionality.

## Running the `oneDNN` test
Individual test can be run by hand, for example:

```
$ cd metrics/machine_learning
$ ./onednn.sh
```
