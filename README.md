# Kata Containers packaging

Kata Containers currently supports packages for many distributions. Tooling to aid in creating these
packages are contained within this repository.

In addition, Kata build artifacts are available within a container image, created by a
[Dockerfile](kata-deploy/Dockerfile).  Reference daemonsets are provided in [kata-deploy](kata-deploy),
which make installation of Kata Containers in a running Kubernetes Cluster very straightforward.
