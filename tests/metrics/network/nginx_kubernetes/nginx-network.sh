#!/bin/bash
#
# Copyright (c) 2023 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

set -o pipefail

SCRIPT_PATH=$(dirname "$(readlink -f "$0")")

source "${SCRIPT_PATH}/../../lib/common.bash"
nginx_file=$(mktemp nginxresults.XXXXXXXXXX)
TEST_NAME="${TEST_NAME:-nginx}"

function remove_tmp_file() {
	rm -rf "${nginx_file}"
}

trap remove_tmp_file EXIT

function main() {
	init_env
	cmds=("bc" "jq" "ab")
	check_cmds "${cmds[@]}"

	# Check no processes are left behind
	check_processes

	wait_time=20
	sleep_time=2
	timeout="20s"

	deployment="nginx-deployment"
	kubectl create -f "${SCRIPT_PATH}/runtimeclass_workloads/nginx-networking.yaml"
	kubectl wait --for=condition=Available --timeout="${timeout}" deployment/"${deployment}"
	kubectl expose deployment/"${deployment}"
	ip=$(kubectl get service/nginx-deployment -o jsonpath='{.spec.clusterIP}')

	ab -n 100000 -c 100 http://"${ip}":80/ > "${nginx_file}"
	metrics_json_init
 	rps=$(cat "${nginx_file}" | grep "Requests" | awk '{print $4}')
	echo "Requests per second: ${rps}"

	metrics_json_start_array

	local json="$(cat << EOF
	{
		"requests": {
			"Result" : ${rps},
			"Units": "rps"
		}
	}
EOF
)"

	metrics_json_add_array_element "$json"
	metrics_json_end_array "Results"
	metrics_json_save

	nginx_cleanup
}

function nginx_cleanup() {
	kubectl delete deployment "${deployment}"
	kubectl delete service "${deployment}"
	check_processes
}

main "$@"
