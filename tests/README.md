# Kata Containers Tests

This directory contains various types of tests for testing the Kata Containers
repository.

## Test Content

We provide several tests to ensure Kata-Containers run on different scenarios
and with different container managers.

1. Integration tests to ensure compatibility with:
   - [Kubernetes](https://github.com/kata-containers/kata-containers/tree/main/tests/integration/kubernetes)
   - [`Cri-Containerd`](https://github.com/kata-containers/kata-containers/tree/main/tests/integration/cri-containerd)
   - [Docker](https://github.com/kata-containers/kata-containers/tree/main/tests/integration/docker)
   - [`Nerdctl`](https://github.com/kata-containers/kata-containers/tree/main/tests/integration/nerdctl)
   - [`Nydus`](https://github.com/kata-containers/kata-containers/tree/main/tests/integration/nydus)
   - [`Runk`](https://github.com/kata-containers/kata-containers/tree/main/tests/integration/runk)
2. [Stability tests](https://github.com/kata-containers/kata-containers/tree/main/tests/stability)
3. [Metrics](https://github.com/kata-containers/kata-containers/tree/main/tests/metrics)
4. [Functional](https://github.com/kata-containers/kata-containers/tree/main/tests/functional)

## GitHub Actions

Kata Containers uses GitHub Actions in the [Kata Containers](https://github.com/kata-containers/kata-containers/tree/main/.github/workflows) repository.
