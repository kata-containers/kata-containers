OpenShift CI
============

This directory contains scripts used by
[the OpenShift CI](https://github.com/openshift/release/tree/master/ci-operator/config/kata-containers/kata-containers)
pipelines to monitor selected functional tests on OpenShift.
There are 2 pipelines, history and logs can be accessed here:

* [main - currently supported OCP](https://prow.ci.openshift.org/job-history/gs/origin-ci-test/logs/periodic-ci-kata-containers-kata-containers-main-e2e-tests)
* [next - currently under development OCP](https://prow.ci.openshift.org/job-history/gs/origin-ci-test/logs/periodic-ci-kata-containers-kata-containers-main-next-e2e-tests)


Running openshift-tests on OCP with kata-containers manually
============================================================

To run openshift-tests (or other suites) with kata-containers one can use
the kata-webhook. To deploy everything you can mimic the CI pipeline by:

```bash
#!/bin/bash -e
# Setup your kubectl and check it's accessible by
kubectl nodes
# Deploy kata (set KATA_DEPLOY_IMAGE to override the default kata-deploy-ci:latest image)
./test.sh
# Deploy the webhook
KATA_RUNTIME=kata-qemu cluster/deploy_webhook.sh
```

This should ensure kata-containers as well as kata-webhook are installed and
working. Before running the openshift-tests it's (currently) recommended to
ignore some security features by:

```bash
#!/bin/bash -e
oc adm policy add-scc-to-group privileged system:authenticated system:serviceaccounts
oc adm policy add-scc-to-group anyuid system:authenticated system:serviceaccounts
oc label --overwrite ns default pod-security.kubernetes.io/enforce=privileged pod-security.kubernetes.io/warn=baseline pod-security.kubernetes.io/audit=baseline
```

Now you should be ready to run the openshift-tests. Our CI only uses a subset
of tests, to get the current ``TEST_SKIPS`` see
[the pipeline config](https://github.com/openshift/release/tree/master/ci-operator/config/kata-containers/kata-containers).
Following steps require the [openshift tests](https://github.com/openshift/origin)
being cloned and built in the current directory:

```bash
#!/bin/bash -e
# Define tests to be skipped (see the pipeline config for the current version)
TEST_SKIPS="\[sig-node\] Security Context should support seccomp runtime/default\|\[sig-node\] Variable Expansion should allow substituting values in a volume subpath\|\[k8s.io\] Probing container should be restarted with a docker exec liveness probe with timeout\|\[sig-node\] Pods Extended Pod Container lifecycle evicted pods should be terminal\|\[sig-node\] PodOSRejection \[NodeConformance\] Kubelet should reject pod when the node OS doesn't match pod's OS\|\[sig-network\].*for evicted pods\|\[sig-network\].*HAProxy router should override the route\|\[sig-network\].*HAProxy router should serve a route\|\[sig-network\].*HAProxy router should serve the correct\|\[sig-network\].*HAProxy router should run\|\[sig-network\].*when FIPS.*the HAProxy router\|\[sig-network\].*bond\|\[sig-network\].*all sysctl on whitelist\|\[sig-network\].*sysctls should not affect\|\[sig-network\] pods should successfully create sandboxes by adding pod to network"
# Get the list of tests to be executed
TESTS="$(./openshift-tests run --dry-run --provider "${TEST_PROVIDER}" "${TEST_SUITE}")"
# Store the list of tests in /tmp/tsts file
echo "${TESTS}" | grep -v "$TEST_SKIPS" > /tmp/tsts
# Remove previously-existing temporarily files as well as previous results
OUT=RESULTS/tmp
rm -Rf /tmp/*test* /tmp/e2e-*
rm -R $OUT
mkdir -p $OUT
# Run the tests ignoring the monitor health checks
./openshift-tests run --provider azure -o "$OUT/job.log" --junit-dir "$OUT" --file /tmp/tsts --max-parallel-tests 5 --cluster-stability Disruptive --run '^\[sig-node\].*|^\[sig-network\]'
```

[!NOTE]
Note we are ignoring the cluster stability checks because our public cloud is
not that stable and running with VMs instead of containers results in minor
stability issues. Some of the old monitor stability tests do not reflect
the ``--cluster-stability`` setting, one should simply ignore these. If you
get a message like ``invariant was violated`` or ``error: failed due to a
MonitorTest failure``, it's usually an indication that only those kind of
tests failed but the real tests passed. See
[wrapped-openshift-tests.sh](https://github.com/openshift/release/blob/master/ci-operator/config/kata-containers/kata-containers/wrapped-openshift-tests.sh)
for details how our pipeline deals with that.

[!TIP]
To compare multiple results locally one can use
[junit2html](https://github.com/inorton/junit2html) tool.


Best-effort kata-containers cleanup
===================================

If you need to cleanup the cluster after testing, you can use the
``cleanup.sh`` script from the current directory. It tries to delete all
resources created by ``test.sh`` as well as ``cluster/deploy_webhook.sh``
ignoring all failures. The primary purpose of this script is to allow
soft-cleanup after deployment to test different versions without
re-provisioning everything.

[!WARNING]
Do not rely on this script in production, return codes are not checked!**


Bisecting e2e tests failures
============================

Let's say the OCP pipeline passed running with
``quay.io/kata-containers/kata-deploy-ci:kata-containers-d7afd31fd40e37a675b25c53618904ab57e74ccd-amd64``
but failed running with
``quay.io/kata-containers/kata-deploy-ci:kata-containers-9f512c016e75599a4a921bd84ea47559fe610057-amd64``
and you'd like to know which PR caused the regression. You can either run with
all the 60 tags between or you can utilize the [bisecter](https://github.com/ldoktor/bisecter)
to optimize the number of steps in between.

Before running the bisection you need a reproducer script. Sample one called
``sample-test-reproducer.sh`` is provided in this directory but you might
want to copy and modify it, especially:

* ``OCP_DIR`` - directory where your openshift/release is located (can be exported)
* ``E2E_TEST`` - openshift-test(s) to be executed (can be exported)
* behaviour of SETUP (returning 125 skips the current image tag, returning
  >=128 interrupts the execution, everything else reports the tag as failure
* what should be executed (perhaps running the setup is enough for you or
  you might want to be looking for specific failures...)
* use ``timeout`` to interrupt execution in case you know things should be faster

Executing that script with the GOOD commit should pass
``./sample-test-reproducer.sh quay.io/kata-containers/kata-deploy-ci:kata-containers-d7afd31fd40e37a675b25c53618904ab57e74ccd-amd64``
and fail when executed with the BAD commit
``./sample-test-reproducer.sh quay.io/kata-containers/kata-deploy-ci:kata-containers-9f512c016e75599a4a921bd84ea47559fe610057-amd64``.

To get the list of all tags in between those two PRs you can use the
``bisect-range.sh`` script

```bash
./bisect-range.sh d7afd31fd40e37a675b25c53618904ab57e74ccd 9f512c016e75599a4a921bd84ea47559fe610057
```

[!NOTE]
The tagged images are only built per PR, not for individual commits. See
[kata-deploy-ci](https://quay.io/kata-containers/kata-deploy-ci) to see the
available images.

To find out which PR caused this regression, you can either manually try the
individual commits or you can simply execute:

```bash
bisecter start "$(./bisect-range.sh d7afd31fd40 9f512c016)"
OCP_DIR=/path/to/openshift/release bisecter run ./sample-test-reproducer.sh
```

[!NOTE]
If you use ``KATA_WITH_SYSTEM_QEMU=yes`` you might want to deploy once with
it and skip it for the cleanup. That way you might (in most cases) test
all images with a single MCP update instead of per-image MCP update.

[!TIP]
You can check the bisection progress during/after execution by running
``bisecter log`` from the current directory. Before starting a new
bisection you need to execute ``bisecter reset``.
