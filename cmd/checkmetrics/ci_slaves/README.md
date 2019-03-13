# CI slaves reference files

This directory contains the reference `checkmetrics` configuration files
utilised by the CI system build machines to check CI metrics results.

These files are actively downloaded by the CI system on a per-build basis.
Thus, any changes to the files in this directory should apply automatically
to all future CI build/runs.

The files in this directory are named according to how the
[metrics CI script](../../../.ci/run_metrics_PR_ci.sh) invokes `checkmetrics`,
using the CI build machine hostname to locate the correct file:

```bash
local CM_BASE_FILE="${CHECKMETRICS_CONFIG_DIR}/checkmetrics-json-$(uname -n).toml"
```

Thus, each CI metrics slave machine should be uniquely named, to allow for file
differentiation in this directory.
