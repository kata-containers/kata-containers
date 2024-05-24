# Documentation

The [Kata Containers](https://github.com/kata-containers)
documentation repository hosts overall system documentation, with information
common to multiple components.

For details of the other Kata Containers repositories, see the
[repository summary](https://github.com/kata-containers/kata-containers).

## Getting Started

* [Installation guides](./install/README.md): Install and run Kata Containers with Docker or Kubernetes

## Tracing

See the [tracing documentation](tracing.md).

## More User Guides

* [Upgrading](Upgrading.md): how to upgrade from [Clear Containers](https://github.com/clearcontainers) and [runV](https://github.com/hyperhq/runv) to [Kata Containers](https://github.com/kata-containers) and how to upgrade an existing Kata Containers system to the latest version.
* [Limitations](Limitations.md): differences and limitations compared with the default [Docker](https://www.docker.com/) runtime,
[`runc`](https://github.com/opencontainers/runc).

### How-to guides

See the [how-to documentation](how-to).

## Kata Use-Cases

* [GPU Passthrough with Kata](./use-cases/GPU-passthrough-and-Kata.md)
* [SR-IOV with Kata](./use-cases/using-SRIOV-and-kata.md)
* [Intel QAT with Kata](./use-cases/using-Intel-QAT-and-kata.md)
* [SPDK vhost-user with Kata](./use-cases/using-SPDK-vhostuser-and-kata.md)
* [Intel SGX with Kata](./use-cases/using-Intel-SGX-and-kata.md)

## Developer Guide

Documents that help to understand and contribute to Kata Containers.

### Design and Implementations

* [Kata Containers Architecture](design/architecture): Architectural overview of Kata Containers
* [Kata Containers CI](../ci/README.md): Kata Containers CI document
* [Kata Containers E2E Flow](design/end-to-end-flow.md): The entire end-to-end flow of Kata Containers
* [Kata Containers design](./design/README.md): More Kata Containers design documents
* [Kata Containers threat model](./threat-model/threat-model.md): Kata Containers threat model

### How to Contribute

* [Developer Guide](Developer-Guide.md): Setup the Kata Containers developing environments
* [How to contribute to Kata Containers](https://github.com/kata-containers/community/blob/main/CONTRIBUTING.md)
* [Code of Conduct](../CODE_OF_CONDUCT.md)
* [How to submit a blog post](Blog-Post-Submission-Guide.md)

## Help Writing a Code PR

* [Code PR advice](code-pr-advice.md).

## Help Writing Unit Tests

* [Unit Test Advice](Unit-Test-Advice.md)
* [Unit testing presentation](presentations/unit-testing/kata-containers-unit-testing.md)

## Help Improving the Documents

* [Documentation Requirements](Documentation-Requirements.md)

### Code Licensing

* [Licensing](Licensing-strategy.md): About the licensing strategy of Kata Containers.

### The Release Process

* [Release Process](Release-Process.md)

## Presentations

* [Presentations](presentations)

## Website Changes

If you have a suggestion for how we can improve the
[website](https://katacontainers.io), please raise an issue (or a PR) on
[the repository that holds the source for the website](https://github.com/OpenStackweb/kata-netlify-refresh).
