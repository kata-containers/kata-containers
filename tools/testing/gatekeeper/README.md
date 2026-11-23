# CI suite selection

[`required-tests.yaml`](required-tests.yaml) maps changed paths to integration
suites, suites to workflow jobs, and suites to required checks. Update this file
to change the scope of an existing suite or to classify another path. Gatekeeper
uses the same selection to choose its required checks. Static checks and artifact builds remain separate
from integration test selection.

The `paths` rules are ordered: the first matching rule wins for each file.
Patterns use Python `fnmatch` syntax, where `*` also matches directory separators.
The selected suites are the union of all changed files' requirements. Any path
without a rule selects every suite. Keep shared helpers, shims, agents, rootfs
builders and other shared binaries unmapped so their changes retain full coverage.

Each path rule has `paths` and `suites` arrays. An empty `suites` array explicitly
skips integration tests for that path. Builds still run unless the rule sets
`build` to `false`, as the documentation rule does. Only classify a component as
untested after checking that no integration suite consumes it.

Each integration suite has a `jobs` list under `mapping`, alongside its required
check `names`, `regexps`, and `required-labels`. Suite job identifiers are
`<workflow-file-without-extension>/<job-id>`.
Include both the parent reusable-workflow job and any selected child jobs.
Include conditional prerequisites, such as the confidential test image build.
New workflow jobs also need a selection condition in their workflow and an entry
in the suite's required checks if Gatekeeper should require them.

Selection operates on whole suites. Selected runners execute their complete
configured test lists with their existing architecture and hardware restrictions.
Selecting individual test files or cases within a suite is deferred to a separate
change. Kubernetes test and fixture changes select the Kubernetes, TEE, and GPU
suites; genpolicy changes select those suites as well.

The `dependencies` entries name keys under `externals` in `versions.yaml`.
Only changes confined to those entries can narrow dependency testing. For
example, changing only `coco-guest-components` or `coco-trustee` selects TEE tests,
including workflows covering `coco-dev` and confidential GPUs. These workflows
retain their full matrices, including any non-confidential entries. A simultaneous
QEMU, kernel, or other dependency change restores every suite.

PR selection compares committed changes against the merge base with the target
branch. Unrelated upstream changes and local working tree edits are excluded.
Renames select both the old and new paths.
Reusable workflows default to every job when no selection is supplied.

Run the regression tests locally with:

```sh
python3 -m unittest discover -s tools/testing/gatekeeper -p 'test_*.py'
```

The tests require `PyYAML` and Git. They check suite selection rules, real Git diffs,
workflow job coverage, and the mapping used by Gatekeeper. Skipper runs them
before publishing its selection outputs.
