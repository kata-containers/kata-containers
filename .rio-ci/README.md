## Kata Containers Internal CI

### Unit Test

Current unit test leverages [simcloud](https://simcloud.apple.com).

To run the CI pipeline for unit test locally:
1. Go to https://simcloud.apple.com/tutorials/cli/ and install the simcloud cli
2. Run `make -f .rio-ci/Makefile sc-unit-test` from the root of the repo

Logs will be streamed back to the terminal as the job is running on simcloud

### Static Check

Current check flow also leverages [simcloud](https://simcloud.apple.com).

To run the CI pipeline for static check locally:
1. Go to https://simcloud.apple.com/tutorials/cli/ and install the simcloud cli
2. Run `make -f .rio-ci/Makefile sc-check` from the root of the repo

Logs will be streamed back to the terminal as the job is running on simcloud

### K8s Integration Test

Current K8s integration test leverages the ephemeral cluster provided by [konfidence](https://at.apple.com/konfidence).

The base image with (ksmith)(https://github.pie.apple.com/konfidence/ksmith), `kubectl`, `pcl`
is located at https://github.pie.apple.com/compute-x/dockerfiles/tree/master/ksmith-kata

To run the integration test locally:

1. Ensure Docker is installed
2. Run `make -f .rio-ci/Makefile kubernetes-integration-test-local-spawn`

NOTE: since `konfidence` requires an actual namespace with quota in a prod cluster, ensure
you have enough quota in the appropriate priority class.

You can override the default priority class (p1) with
```bash
PRIORITY_CLASS=p2 make -f .rio-ci/Makefile kubernetes-integration-test-local-spawn
```
If the quota is under `p2` priority class

By default, the flow will use the Kubernetes config located at `$HOME/.kube/config`.
To override this, use
```bash
KUBECONFIG=$HOME/.kube/sa-config make -f .rio-ci/Makefile kubernetes-integration-test-local-spawn
```
Where `$HOME/.kube/sa-config` is the path to a custom config for a service account for example.

To take the cluster down, run
```bash
KUBECONFIG=$HOME/.kube/sa-config make -f .rio-ci/Makefile kubernetes-integration-test-local-teardown
```
Where `$HOME/.kube/sa-config` is the path to a custom config for a service account for example.
