# Substrate microVM tests

The normal `ci.yaml` pipeline calls `run-substrate-tests.yaml` through the
existing PR and nightly entry points, with the same gate as other integration
tests. The `substrate-microvm-e2e` job checks out upstream `main` and runs the
Kind microVM e2e suites with upstream's default guest assets. It verifies test
execution and uploads the results.

For local setup and testing, see [Substrate's microVM guide](https://github.com/agent-substrate/substrate/blob/main/docs/dev/microvm-local.md).
The standalone guest boot test is provided by [Substrate PR #1967](https://github.com/agent-substrate/substrate/pull/1967).
