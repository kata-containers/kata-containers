#!/bin/bash
#
# Copyright (c) 2021 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#
# This test measures the following network essentials:
# - bandwith simplex
# - jitter
#
# These metrics/results will be got from the interconnection between
# a client and a server using iperf3 tool.
# The following cases are covered:
#
# case 1:
#  container-server <----> container-client
#
# case 2"
#  container-server <----> host-client

set -e

SCRIPT_PATH=$(dirname "$(readlink -f "$0")")

source "${SCRIPT_PATH}/../../../.ci/lib.sh"
source "${SCRIPT_PATH}/../../lib/common.bash"
iperf_file=$(mktemp iperfresults.XXXXXXXXXX)
TEST_NAME="${TEST_NAME:-network-iperf3}"
COLLECT_ALL="${COLLECT_ALL:-false}"
CI_JOB="${CI_JOB:-}"

function remove_tmp_file() {
	rm -rf "${iperf_file}"
}

trap remove_tmp_file EXIT

function iperf3_all_collect_results() {
	metrics_json_init
	metrics_json_start_array
	local json="$(cat << EOF
	{
		"bandwidth": {
			"Result" : $bandwidth_result,
			"Units" : "$bandwidth_units"
		},
		"jitter": {
			"Result" : $jitter_result,
			"Units" : "$jitter_units"
		},
		"cpu": {
			"Result" : $cpu_result,
			"Units"  : "$cpu_units"
		},
		"parallel": {
			"Result" : $parallel_result,
			"Units" : "$parallel_units"
		}
	}
EOF
)"
	metrics_json_add_array_element "$json"
	metrics_json_end_array "Results"
}

function iperf3_bandwidth() {
	# Start server
	local transmit_timeout="30"

	kubectl exec -i "$client_pod_name" -- sh -c "iperf3 -J -c ${server_ip_add} -t ${transmit_timeout}" | jq '.end.sum_received.bits_per_second' > "${iperf_file}"
	export bandwidth_result=$(cat "${iperf_file}")
	export bandwidth_units="bits per second"

	if [ "$COLLECT_ALL" == "true" ]; then
		iperf3_all_collect_results
	else
		metrics_json_init
		metrics_json_start_array

		local json="$(cat << EOF
		{
			"bandwidth": {
				"Result" : $bandwidth_result,
				"Units" : "$bandwidth_units"
			}
		}
EOF
)"
		metrics_json_add_array_element "$json"
		metrics_json_end_array "Results"
	fi
}

function iperf3_jitter() {
	# Start server
	local transmit_timeout="30"

	kubectl exec -i "$client_pod_name" -- sh -c "iperf3 -J -c ${server_ip_add} -u -t ${transmit_timeout}" | jq '.end.sum.jitter_ms' > "${iperf_file}"
	result=$(cat "${iperf_file}")
	export jitter_result=$(printf "%0.3f\n" $result)
	export jitter_units="ms"

	if [ "$COLLECT_ALL" == "true" ]; then
		iperf3_all_collect_results
	else
		metrics_json_init
		metrics_json_start_array

		local json="$(cat << EOF
		{
			"jitter": {
				"Result" : $jitter_result,
				"Units" : "ms"
			}
		}
EOF
)"
		metrics_json_add_array_element "$json"
		metrics_json_end_array "Results"
	fi
}

function iperf3_parallel() {
	# This will measure four parallel connections with iperf3
	kubectl exec -i "$client_pod_name" -- sh -c "iperf3 -J -c ${server_ip_add} -P 4" | jq '.end.sum_received.bits_per_second' > "${iperf_file}"
	export parallel_result=$(cat "${iperf_file}")
	export parallel_units="bits per second"

	if [ "$COLLECT_ALL" == "true" ]; then
		iperf3_all_collect_results
	else
		metrics_json_init
		metrics_json_start_array

		local json="$(cat << EOF
		{
			"parallel": {
				"Result" : $parallel_result,
				"Units" : "$parallel_units"
			}
		}
EOF
)"
		metrics_json_add_array_element "$json"
		metrics_json_end_array "Results"
	fi
}

function iperf3_cpu() {
	# Start server
	local transmit_timeout="80"

	kubectl exec -i "$client_pod_name" -- sh -c "iperf3 -J -c ${server_ip_add} -t ${transmit_timeout}" | jq '.end.cpu_utilization_percent.host_total' > "${iperf_file}"
	export cpu_result=$(cat "${iperf_file}")
	export cpu_units="percent"

	if [ "$COLLECT_ALL" == "true" ]; then
		iperf3_all_collect_results
	else
		metrics_json_init
		metrics_json_start_array

		local json="$(cat << EOF
		{
			"cpu": {
				"Result" : $cpu_result,
				"Units"  : "$cpu_units"
			}
		}
EOF
)"

		metrics_json_add_array_element "$json"
		metrics_json_end_array "Results"
	fi
}

function iperf3_start_deployment() {
	cmds=("bc" "jq")
	check_cmds "${cmds[@]}"

	# Check no processes are left behind
	check_processes

	if [ -z "${CI_JOB}" ]; then
		# Start kubernetes
		start_kubernetes
	fi

	export KUBECONFIG="$HOME/.kube/config"
	export service="iperf3-server"
	export deployment="iperf3-server-deployment"

	wait_time=20
	sleep_time=2

	# Create deployment
	kubectl create -f "${SCRIPT_PATH}/runtimeclass_workloads/iperf3-deployment.yaml"

	# Check deployment creation
	local cmd="kubectl wait --for=condition=Available deployment/${deployment}"
	waitForProcess "$wait_time" "$sleep_time" "$cmd"

	# Create DaemonSet
	kubectl create -f "${SCRIPT_PATH}/runtimeclass_workloads/iperf3-daemonset.yaml"

	# Expose deployment
	kubectl expose deployment/"${deployment}"

	# Get the names of the server pod
	export server_pod_name=$(kubectl get pods -o name | grep server | cut -d '/' -f2)

	# Verify the server pod is working
	local cmd="kubectl get pod $server_pod_name -o yaml | grep 'phase: Running'"
	waitForProcess "$wait_time" "$sleep_time" "$cmd"

	# Get the names of client pod
	export client_pod_name=$(kubectl get pods -o name | grep client | cut -d '/' -f2)

	# Verify the client pod is working
	local cmd="kubectl get pod $client_pod_name -o yaml | grep 'phase: Running'"
	waitForProcess "$wait_time" "$sleep_time" "$cmd"

	# Get the ip address of the server pod
	export server_ip_add=$(kubectl get pod "$server_pod_name" -o jsonpath='{.status.podIP}')
}

function iperf3_deployment_cleanup() {
	kubectl delete pod "$server_pod_name" "$client_pod_name"
	kubectl delete ds iperf3-clients 
	kubectl delete deployment "$deployment"
	kubectl delete service "$deployment"
	if [ -z "${CI_JOB}" ]; then
		end_kubernetes
		check_processes
	fi
}

function help() {
echo "$(cat << EOF
Usage: $0 "[options]"
	Description:
		This script implements a number of network metrics
		using iperf3.

	Options:
		-a	Run all tests
		-b 	Run bandwidth tests
		-c	Run cpu metrics tests
		-h	Help
		-j	Run jitter tests
EOF
)"
}

function main() {
	init_env
	iperf3_start_deployment

	local OPTIND
	while getopts ":abcjph:" opt
	do
		case "$opt" in
		a)	# all tests
			test_all="1"
			;;
		b)	# bandwith test
			test_bandwith="1"
			;;
		c)
			# run cpu tests
			test_cpu="1"
			;;
		h)
			help
			exit 0;
			;;
		j)	# jitter tests
			test_jitter="1"
			;;
		p)
			# run parallel tests
			test_parallel="1"
			;;
		:)
			echo "Missing argument for -$OPTARG";
			help
			exit 1;
			;;
		esac
	done
	shift $((OPTIND-1))

	[[ -z "$test_bandwith" ]] && \
	[[ -z "$test_jitter" ]] && \
	[[ -z "$test_cpu" ]] && \
	[[ -z "$test_parallel" ]] && \
	[[ -z "$test_all" ]] && \
		help && die "Must choose at least one test"

	if [ "$test_bandwith" == "1" ]; then
		iperf3_bandwidth
	fi

	if [ "$test_jitter" == "1" ]; then
		iperf3_jitter
	fi

	if [ "$test_cpu" == "1" ]; then
		iperf3_cpu
	fi

	if [ "$test_parallel" == "1" ]; then
		iperf3_parallel
	fi

	if [ "$test_all" == "1" ]; then
		export COLLECT_ALL=true && iperf3_bandwidth && iperf3_jitter && iperf3_cpu && iperf3_parallel
	fi

	metrics_json_save
	iperf3_deployment_cleanup
}

main "$@"
