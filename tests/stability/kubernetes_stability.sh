#!/bin/bash
#
# Copyright (c) 2024 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

set -o pipefail
set -x

SCRIPT_PATH=$(dirname "$(readlink -f "$0")")
source "${SCRIPT_PATH}/../metrics/lib/common.bash"

# Timeout is the duration of this test (seconds)
timeout=3600
start_time=$(date +%s)
end_time=$((start_time+timeout))


function main() {
	# Check no processes are left behind
	check_processes

	# Create pod
	kubectl create -f "${SCRIPT_PATH}/runtimeclass_workloads/stability-test.yaml"
	# Verify pod is running
	pod_name="stability-test"
	kubectl wait --for=condition=Ready --timeout=30s pod "${pod_name}"

	echo "Running kubernetes stability test"
	count=0
	while [[ "${end_time}" > $(date +%s) ]]; do
		echo "This is the number of iterations $count"
		count=$((count+1))

		cmd1="echo 'hello world' > file"
		kubectl exec "${pod_name}" -- /bin/bash -c "${cmd1}"

		cmd2="rm -rf /file"
		kubectl exec "${pod_name}" -- /bin/bash -c "${cmd2}"

		cmd3="touch /tmp/execWorks"
		kubectl exec "${pod_name}" -- /bin/bash -c "${cmd3}"

		cmd4="ls /tmp | grep execWorks"
		kubectl exec "${pod_name}" -- /bin/bash -c "${cmd4}"

		cmd5="rm -rf /tmp/execWorks"
		kubectl exec "${pod_name}" -- /bin/bash -c "${cmd5}"
	done

	kubectl delete -f "${SCRIPT_PATH}/runtimeclass_workloads/stability-test.yaml"
}

main "$@"
