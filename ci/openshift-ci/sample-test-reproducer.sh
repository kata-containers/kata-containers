#!/bin/bash
# Copyright (c) 2024 Red Hat, Inc.
#
# SPDX-License-Identifier: Apache-2.0
#
# A sample script to deploy, configure, run E2E_TEST and soft-cleanup
# afterwards OCP cluster using kata-containers primarily created for use
# with https://github.com/ldoktor/bisecter

[ "$#" -ne 1 ] && echo "Provide image as the first and only argument" && exit 255
export KATA_DEPLOY_IMAGE="$1"
OCP_DIR="${OCP_DIR:-/path/to/your/openshift/release/}"
E2E_TEST="${E2E_TEST:-'"[sig-node] Container Runtime blackbox test on terminated container should report termination message as empty when pod succeeds and TerminationMessagePolicy FallbackToLogsOnError is set [NodeConformance] [Conformance] [Suite:openshift/conformance/parallel/minimal] [Suite:k8s]"'}"
KATA_CI_DIR="${KATA_CI_DIR:-$(pwd)}"
export KATA_RUNTIME="${KATA_RUNTIME:-kata-qemu}"

## SETUP
# Deploy kata
SETUP=0
pushd "$KATA_CI_DIR" || { echo "Failed to cd to '$KATA_CI_DIR'"; exit 255; }
./test.sh || SETUP=125
cluster/deploy_webhook.sh || SETUP=125
if [ $SETUP != 0 ]; then
    ./cleanup.sh
    exit "$SETUP"
fi
popd || true
# Disable security
oc adm policy add-scc-to-group privileged system:authenticated system:serviceaccounts
oc adm policy add-scc-to-group anyuid system:authenticated system:serviceaccounts
oc label --overwrite ns default pod-security.kubernetes.io/enforce=privileged pod-security.kubernetes.io/warn=baseline pod-security.kubernetes.io/audit=baseline

## TEST EXECUTION
# Run the testing
pushd "$OCP_DIR" || { echo "Failed to cd to '$OCP_DIR'"; exit 255; }
echo "$E2E_TEST" > /tmp/tsts
# Remove previously-existing temporarily files as well as previous results
OUT=RESULTS/tmp
rm -Rf /tmp/*test* /tmp/e2e-*
rm -R $OUT
mkdir -p $OUT
# Run the tests ignoring the monitor health checks
./openshift-tests run --provider azure -o "$OUT/job.log" --junit-dir "$OUT" --file /tmp/tsts --max-parallel-tests 5 --cluster-stability Disruptive
RET=$?
popd || true

## CLEANUP
./cleanup.sh
exit "$RET"

