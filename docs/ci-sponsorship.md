# CI infrastructure sponsorship

Kata Containers welcomes cloud credits and access to physical hardware to run
continuous integration (CI). Sponsors can support ordinary Kubernetes testing
or provide access to architectures and features that are harder to obtain,
including confidential computing and GPUs.

Anyone helping with sponsorship can use this guide to collect resource usage
and capacity estimates and present them to the Architecture Committee. The
committee can then review the proposal using the supplied evidence, without
its members having to collect the numbers for every sponsorship discussion.

!!! note "Coordinate sponsorship and resource usage"
    The OpenInfra Foundation and the Architecture Committee
    must both be involved in sponsorship discussions and kept informed about
    resource usage and operational issues. Share the available capacity,
    requirements, usage estimates, and any limitations with both groups so
    they can understand what the project can take advantage of and where
    capacity, access, or maintenance bottlenecks occur.

We are flexible about geographic region: there is no preferred region for
sponsored resources. Reliable access, suitable hardware, and someone who can
help maintain it matter more than location.

## Physical hardware

The following is a proposed planning baseline for a dedicated runner executing
one ordinary Kubernetes test job at a time. It is not a measured minimum for
every test suite or a statement of Kata's runtime requirements. Confirm sizing
with the maintainers using the intended test matrix before committing resources.

| Resource | Proposed minimum for sponsorship planning |
| --- | --- |
| CPU | 8 dedicated CPU cores, with the architecture and CPU features needed by the intended jobs. Record physical cores and hardware threads separately. |
| RAM | 32 GiB available to the runner and its test workloads. |
| Storage | 200 GiB of usable SSD storage for images, artifacts, logs, and temporary files; allow cleanup between jobs. |
| Virtualization | Working Linux KVM support, enabled in firmware, with access to `/dev/kvm` for the test processes. CPU virtualization flags alone are insufficient. |
| Network | Reliable Internet access to GitHub, container registries, and dependency downloads. Agree on any proxy, firewall, or traffic restrictions before setup. |
| Administration | A dedicated Linux test environment where maintainers can install dependencies, change the kernel and networking, and reboot or reinstall the machine. |

Start with one concurrent job per machine. More concurrency, scale tests,
local builds, GPU workloads, and confidential computing tests can need more
CPU, memory, storage, or specific hardware. Confirm those requirements separately.
For virtual machines offered instead of physical hosts, verify that nested
virtualization works with the intended hypervisor. See the
[installation guide](installation.md) for virtualization prerequisites.

### Availability, reliability, and maintenance

- Provide continuous availability for jobs triggered throughout the week,
  ideally 24 hours a day, seven days a week. A limited schedule can still be
  useful if agreed in advance and reflected in runner scheduling.
- Agree on the duration of the sponsorship, expected uptime, maintenance
  windows, and notice before access is withdrawn. Avoid suspending a machine
  during a running job.
- Name at least one person responsible for maintaining the sponsored
  infrastructure. This person must be reachable when a host becomes unavailable
  or tests leave it in a broken state, and must be able to resolve problems or
  involve someone who can.
- Agree on a contact channel and expected response time. A backup contact is
  desirable for holidays and other absences.
- Provide a recovery path for a machine that cannot be reached over the network,
  such as remote console and power control or an administrator who can reboot
  and reinstall it. Clarify who handles hardware failures, firmware and kernel
  updates, disk cleanup, runner updates, and failed provisioning.

Sponsored runners execute test code and can require privileged operations.
Keep them separate from production workloads and sensitive data. Agree with
maintainers on runner access, cleanup, and recovery before enabling jobs.

## Cloud sponsorship

Cloud credits should cover the complete environment, including worker nodes,
control plane charges where applicable, disks, network traffic, and resources
that remain allocated between jobs. Confirm that the offered instance types
support KVM or nested virtualization and any required device access.

Agree on a budget, sponsorship period, resource quotas, a billing contact,
spending alerts, and who can delete resources left behind by failed jobs.
Region is flexible for cloud resources too; select a region with suitable
capacity and features.

## Measure CI demand with GitHub CLI

Use individual GitHub Actions jobs as the accounting unit. A workflow run can
contain many matrix jobs, and reruns consume resources again. Count individual
test cases separately from jobs and test suite files.

The commands below download metadata and logs for analysis; they do not run
tests or provision infrastructure. They require Bash, GitHub CLI (`gh`)
version `2.104.0` or newer, `jq`, and GitHub access that permits reading Actions
metadata, logs, and artifacts. The walkthrough was validated with `gh` version
`2.104.0`; older versions can lack required options, including
`gh api --allow-escape-sequences`. Check your version before starting:

```bash
gh --version
```

### Download a week of runs

Choose a completed, representative week in UTC. Set `from_date` to its first
day and `to_date` to the day after its last day, using `YYYY-MM-DD` format.
The end date is excluded. Run these commands in a fresh directory for each
sample so that files from different samples cannot mix. Check that each
command succeeds before proceeding; if a command fails, resolve the error
before using its output or running the next command.

```bash
gh auth status
repo=kata-containers/kata-containers
: "${from_date:?Set from_date in YYYY-MM-DD format}"
: "${to_date:?Set to_date in YYYY-MM-DD format (exclusive)}"
start="${from_date}T00:00:00Z"
end="${to_date}T00:00:00Z"
mkdir ci-usage
cd ci-usage

gh api --method GET --paginate --slurp \
  "repos/$repo/actions/runs" \
  -f created="$start..$end" \
  -f per_page=100 > run-pages.json

jq --arg start "$start" --arg end "$end" '
  [.[].workflow_runs[]
   | select(.created_at >= $start and .created_at < $end)]
  | unique_by(.id)
' run-pages.json > runs.json
```

The interval includes the start and excludes the end. GitHub limits filtered
run searches to 1,000 results. Check the returned total before proceeding:

```bash
jq '.[0].total_count' run-pages.json
```

If the total reaches 1,000, repeat the query for smaller intervals, combine
their run arrays, and deduplicate by run ID before downloading jobs. Pagination
alone does not bypass this limit. See the
[workflow runs API](https://docs.github.com/en/rest/actions/workflow-runs)
and the [GitHub CLI API manual](https://cli.github.com/manual/gh_api).

### Download jobs, including reruns

```bash
jq -r '.[].id' runs.json |
while read -r run_id; do
  gh api --method GET --paginate --slurp \
    "repos/$repo/actions/runs/$run_id/jobs" \
    -f filter=all \
    -f per_page=100 > "jobs-$run_id.json"
done

jq -s '[.[][] | .jobs[]] | unique_by(.id)' jobs-*.json > jobs.json
```

If there are no runs, report an empty sample instead of running the final
command. Check API errors before using downloaded files. The `filter=all`
parameter includes earlier attempts; the default only returns the latest
attempt. The [workflow jobs API](https://docs.github.com/en/rest/actions/workflow-jobs)
provides job and step timestamps, conclusions, and runner labels.

This sample contains jobs belonging to runs **created during the selected
week**. Reruns executed later remain part of that sample, while runs created
before the week are excluded. For exact calendar-week usage, collect runs
whose executions overlap the week, including older runs with reruns, and clip
each job interval to the week boundaries.

### Select Kubernetes jobs

Names can change, and reusable workflows can appear under a generic parent
workflow name. Do not use a fixed name filter as the authoritative selection.
Build an inventory of job IDs, step numbers, and names for the sample:

```bash
jq -r '.[] as $job | $job.steps[]? |
  [$job.id, $job.workflow_name, $job.name, .number, .name, .conclusion]
  | @tsv' jobs.json > step-inventory.tsv
```

Review the inventory against the workflow and runner scripts at the revisions
used by each run. Identify Kubernetes test steps by what they execute, such as
`tests/integration/kubernetes/gha-run.sh run-tests`, rather than their display
names alone. Check logs when necessary. Names and numbers can change between
runs, so review each distinct workflow revision and matrix configuration.

Create `test-steps.tsv` with one row per selected test step: job ID, a literal
tab, and step number, without a header. These IDs and numbers refer only to
this downloaded sample; do not reuse the selection for another period.
Convert that reviewed selection into machine-readable metadata:

```bash
jq -Rn '[inputs | select(length > 0) | split("\t") |
  {job_id: (.[0] | tonumber), step_number: (.[1] | tonumber)}]
' test-steps.tsv > test-steps.json

jq --slurpfile selection test-steps.json '
  [.[] | . as $job |
   [$selection[0][] | select(.job_id == $job.id) | .step_number]
   | unique as $numbers |
   select(($numbers | length) > 0) |
   $job + {test_step_numbers: $numbers}]
' jobs.json > k8s-jobs.json
```

Keep a record of excluded and unclassified jobs, including jobs with no steps.
Do not interpret jobs absent from the reviewed selection as having zero cost.
Account for provisioning, cleanup, and jobs cancelled before tests separately.
Save the selection and revision information with the estimate so someone else
can reproduce it even after names change.

### Estimate time per job

The following produces a tab-separated report with full job time and test
step time. Missing timestamps remain unknown instead of becoming zero.

```bash
jq -r '
  def epoch: sub("\\.[0-9]+Z$"; "Z") | fromdateiso8601;
  def seconds:
    if .conclusion == "skipped" then 0
    elif .started_at != null and .completed_at != null
    then ((.completed_at | epoch) - (.started_at | epoch))
    else null end;
  def minutes: if . == null then null else . / 60 end;

  (["job_id", "workflow", "job", "conclusion", "runner_labels",
    "job_minutes", "test_minutes"]),
  (.[] | . as $job |
    [.steps[]? | .number as $number |
     select($job.test_step_numbers | index($number))
     | seconds] as $tests |
    [.id, .workflow_name, .name, .conclusion,
     (.labels | join(",")), (seconds | minutes),
     (if ($tests | length) == 0 or any($tests[]; . == null)
      then null else ($tests | add | minutes) end)])
  | @tsv
' k8s-jobs.json > k8s-job-times.tsv
```

Use test minutes to understand test demand and full job minutes as the starting
point for resource budgeting. Include failed and cancelled executions. Inspect
unfinished jobs separately; blank durations are incomplete data. Do not use
workflow elapsed time as total compute time because jobs can run concurrently.

### Count test cases per job

Download one job's log using its run and job IDs from the metadata:

```bash
run_id=REPLACE_WITH_RUN_ID
job_id=REPLACE_WITH_JOB_ID
gh api --allow-escape-sequences "repos/$repo/actions/jobs/$job_id/logs" > "job-$job_id.log"
```

Use the raw job log and the selected step timestamps, avoiding dependence on
step display names or the step labels added by `gh run view`.
The `--allow-escape-sequences` option permits raw log downloads; keep the
downloaded log in a file. The `gh run view` command can report `UNKNOWN STEP`
or return incomplete logs. This extraction requires
Python 3.11 or newer in addition to the tools listed above. The timestamp
parser uses the ISO 8601 support added in Python 3.11, including the `Z`
suffix and fractional seconds in log timestamps. See
[`datetime.fromisoformat()`](https://docs.python.org/3.11/library/datetime.html#datetime.datetime.fromisoformat).
Check your version before running the extraction:

```bash
python3 --version
```

```bash
python3 - "$job_id" <<'PYTHON'
import datetime
import json
import pathlib
import re
import sys

job_id = int(sys.argv[1])
job = next(j for j in json.loads(pathlib.Path("k8s-jobs.json").read_text())
           if j["id"] == job_id)

def timestamp(value):
    return datetime.datetime.fromisoformat(value)

intervals = []
for step in job["steps"]:
    if step["number"] not in job["test_step_numbers"]:
        continue
    if step["conclusion"] == "skipped":
        continue
    if not step.get("started_at") or not step.get("completed_at"):
        raise SystemExit("Selected step has incomplete timestamps")
    intervals.append((timestamp(step["started_at"]),
                      timestamp(step["completed_at"])))

pattern = re.compile(r"^(\d{4}-\d{2}-\d{2}T\S+Z) (.*)$")
with pathlib.Path(f"job-{job_id}-tests.tap").open("w") as output:
    for line in pathlib.Path(f"job-{job_id}.log").read_text(
            encoding="utf-8-sig").splitlines():
        match = pattern.match(line)
        if match and any(start <= timestamp(match[1]) < end
                         for start, end in intervals):
            output.write(match[2] + "\n")
PYTHON

awk '
  /^(not )?ok [0-9]+([[:space:]]|$)/ {
    results++
    if ($0 ~ /#[[:space:]]*[Ss][Kk][Ii][Pp]/) skipped++
    else {
      executed++
      if ($0 ~ /^not ok /) failed++
      else passed++
    }
  }
  END {
    printf "results=%d executed=%d passed=%d failed=%d skipped=%d\n",
           results, executed, passed, failed, skipped
  }
' "job-$job_id-tests.tap"
```

Inspect the extracted TAP before trusting the count: log formats can change.
API step timestamps have second precision, so inspect boundary lines if a
test result is emitted during a step transition. Count test result records, not TAP plan lines or the
number of `.bats` files. A file can contain several cases. A count of zero
requires investigation; missing logs or interrupted suites are unknown or
partial results, not evidence that no tests were scheduled.

The [Kubernetes test runner](../tests/integration/kubernetes/k8s_bats_runner.sh)
also saves per-suite `.out` reports and can rerun failed cases for debug
triage. Log counts include these extra executions, which consume resources.
To count distinct cases, use original suite reports separately from
`triage-debug-*` files and preserve suite and case names. Bats timing can help
estimate individual case time; test step time also covers work between cases.
Check the runner at the sampled commit because older runs may differ.

If the workflow uploads reports, discover artifacts and download the relevant
one instead of parsing the combined log:

```bash
gh api --paginate "repos/$repo/actions/runs/$run_id/artifacts" \
  --jq '.artifacts[] | [.id, .name, .expired] | @tsv'
gh run download "$run_id" --repo "$repo" \
  --name REPLACE_WITH_ARTIFACT_NAME --dir "artifacts-$run_id"
```

Keep each report mapped to its matrix job and attempt. Not every workflow
uploads reports, and expired artifacts cannot supply missing counts.

### Count weekly jobs

```bash
jq '{
  job_records: length,
  skipped: ([.[] | select(.conclusion == "skipped")] | length),
  finished_executions: ([.[] | select(.conclusion != "skipped"
    and .started_at != null and .completed_at != null)] | length),
  unfinished: ([.[] | select(.status != "completed")] | length)
}' jobs.json
```

Use `k8s-jobs.json` in the same command for the Kubernetes subset. These are
job executions, including reruns and matrix entries, rather than workflow or
pull request counts. Use several representative weeks to show variation.

## Turn demand into a sponsorship estimate

Summarize the sample by machine type and test configuration:

| Field | Purpose |
| --- | --- |
| Week and selection rules | Make the sample reproducible and disclose incomplete data. |
| Architecture, CPU features, GPU, runner labels, and node count | Match jobs to suitable hardware or cloud instance types. |
| Job executions and outcomes | Include retries, failures, cancellations, and skipped jobs separately. |
| Executed, skipped, and repeated test cases | Describe useful coverage and extra diagnostic work. |
| Test hours and full job hours | Separate test demand from setup and cleanup overhead. |
| Node-hours and peak concurrent nodes | Estimate total consumption and required capacity. |

For cloud resources, estimate each machine type separately:

```text
weekly compute cost = sum(node-hours per machine type * hourly price)
weekly total cost = compute + storage + network + control plane + idle resources
```

Multiply duration by the number of nodes only when those nodes have the same
lifetime; otherwise sum their individual lifetimes. Include provisioning and
teardown outside job timestamps, resources left after failures, and idle time
on persistent runners. Use cloud resource lifetimes and billing records when
available to refine the estimate. Record the region, currency, price date,
and billing assumptions instead of treating GitHub job time as a cloud bill.

For physical hardware, use total host-hours and peak concurrency to estimate
the number of machines needed. Include time for cleanup, maintenance, and
recovery; weekly average utilization alone can hide long queues during busy
periods. Record electricity, hosting, network, hardware replacement, and
maintenance responsibilities alongside the donated capacity.
