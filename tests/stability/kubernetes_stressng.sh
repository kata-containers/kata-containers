#!/bin/bash
#
# Copyright (c) 2024 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

set -o pipefail

SCRIPT_PATH=$(dirname "$(readlink -f "$0")")
source "${SCRIPT_PATH}/../metrics/lib/common.bash"

function main() {
        # Check no processes are left behind
        check_processes
        # Create pod
        kubectl create -f "${SCRIPT_PATH}/runtimeclass_workloads/stress-test.yaml"
        # Verify pod is running
        pod_name="stressng-test"
        kubectl wait --for=condition=Ready --timeout=30s pod "${pod_name}"

	echo "Running stress matrix test"
        cmd1="stress-ng --matrix 0 -t 90m"
        kubectl exec "${pod_name}" -- /bin/bash -c "${cmd1}"

	echo "Running stress cpu test"
        cmd2="stress-ng --cpu 0 --vm 2 -t 90m"
        kubectl exec "${pod_name}" -- /bin/bash -c "${cmd2}"

	echo "Running stress io test"
        cmd3="stress-ng --io 2 -t 90m"
        kubectl exec "${pod_name}" -- /bin/bash -c "${cmd3}"

        kubectl delete -f "${SCRIPT_PATH}/runtimeclass_workloads/stress-test.yaml"
        kubectl delete pod "${pod_name}"
        check_processes
}

main "$@"
