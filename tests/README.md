# Kata Containers Tests

This directory contains various types of tests for testing the Kata Containers
repository.

## Test Content

We provide several tests to ensure Kata-Containers run on different scenarios
and with different container managers.

1. Integration tests to ensure compatibility with:
   - [Kubernetes](https://github.com/kata-containers/kata-containers/tree/main/tests/integration/kubernetes)
   - [Cri-Containerd](https://github.com/kata-containers/kata-containers/tree/main/tests/integration/cri-containerd)
   - [Docker](https://github.com/kata-containers/kata-containers/tree/main/tests/integration/docker)
   - [Nerdctl](https://github.com/kata-containers/kata-containers/tree/main/tests/integration/nerdctl)
   - [Nydus](https://github.com/kata-containers/kata-containers/tree/main/tests/integration/nydus)
   - [Runk](https://github.com/kata-containers/kata-containers/tree/main/tests/integration/runk)
2. [Stability tests](https://github.com/kata-containers/kata-containers/tree/main/tests/stability)
3. [Metrics](https://github.com/kata-containers/kata-containers/tree/main/tests/metrics)
4. [Functional](https://github.com/kata-containers/kata-containers/tree/main/tests/functional)

## GitHub Actions

Kata Containers uses GitHub Actions in the [Kata Containers](https://github.com/kata-containers/kata-containers) repository.

## Breaking Compatibility

In case the patch you submit breaks the CI because it needs to be tested
together with a patch from another `kata-containers` repository, you have to
specify which repository and which pull request it depends on.

Using a simple tag `Depends-on:` in your commit message will allow the CI to
run properly. Notice that this tag is parsed from the latest commit of the
pull request.

For example:

```
	Subsystem: Change summary

	Detailed explanation of your changes.

	Fixes: #nnn

	Depends-on:github.com/kata-containers/kata-containers#999

	Signed-off-by: <contributor@foo.com>

```

In this example, we tell the CI to fetch the pull request 999 from the `kata-containers`
repository and use that rather than the `main` branch when testing the changes
contained in this pull request.
