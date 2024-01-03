#!/bin/bash
#
# Copyright (c) 2021-2023 Intel Corporation
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

set -o pipefail

SCRIPT_PATH=$(dirname "$(readlink -f "$0")")

source "${SCRIPT_PATH}/../../lib/common.bash"
iperf_file=$(mktemp iperfresults.XXXXXXXXXX)
TEST_NAME="${TEST_NAME:-network-iperf3}"
COLLECT_ALL="${COLLECT_ALL:-false}"
IPERF_DEPLOYMENT="${SCRIPT_PATH}/runtimeclass_workloads/iperf3-deployment.yaml"
IPERF_DAEMONSET="${SCRIPT_PATH}/runtimeclass_workloads/iperf3-daemonset.yaml"

function iperf3_all_collect_results() {

	if [ -z "${bandwidth_result}" ] || [ -z "${jitter_result}" ] || [ -z "${cpu_result}" ] || [ -z "${parallel_result}" ]; then
		die "iperf couldn't find any results to save."
	fi

	metrics_json_init
	metrics_json_start_array
	local json="$(cat << EOF
	{
		"bandwidth": {
			"Result" : "${bandwidth_result}",
			"Units" : "${bandwidth_units}"
		},
		"jitter": {
			"Result" : "${jitter_result}",
			"Units" : "${jitter_units}"
		},
		"cpu": {
			"Result" : "${cpu_result}",
			"Units"  : "${cpu_units}"
		},
		"parallel": {
			"Result" : "${parallel_result}",
			"Units" : "${parallel_units}"
		}
	}
EOF
)"
	metrics_json_add_array_element "${json}"
	metrics_json_end_array "Results"
}

function iperf3_bandwidth() {
	# Start server
	local transmit_timeout="30"

	kubectl exec -i "$client_pod_name" -- sh -c "iperf3 -J -c ${server_ip_add} -t ${transmit_timeout}" | jq '.end.sum_received.bits_per_second' > "${iperf_file}"
	bandwidth_result=$(cat "${iperf_file}")
	export bandwidth_result=$(printf "%.3f\n" ${bandwidth_result})
	export bandwidth_units="bits per second"

	[ -z "${bandwidth_result}" ] && die "iperf3 was unable to collect Bandwidth workload results."
	[ "$COLLECT_ALL" == "true" ] && return

	metrics_json_init
	metrics_json_start_array

	local json="$(cat << EOF
	{
		"bandwidth": {
			"Result" : "${bandwidth_result}",
			"Units" : "${bandwidth_units}"
		}
	}
EOF
)"
	metrics_json_add_array_element "${json}"
	metrics_json_end_array "Results"
}

function iperf3_jitter() {
	# Start server
	local transmit_timeout="30"

	kubectl exec -i "$client_pod_name" -- sh -c "iperf3 -J -c ${server_ip_add} -u -t ${transmit_timeout}" | jq '.end.sum.jitter_ms' > "${iperf_file}"
	result=$(cat "${iperf_file}")
	export jitter_result=$(printf "%0.3f\n" $result)
	export jitter_units="ms"

	[ -z "${jitter_result}" ] && die "Iperf3 was unable to collect Jitter results."
	[ "$COLLECT_ALL" == "true" ] && return

	metrics_json_init
	metrics_json_start_array

	local json="$(cat << EOF
	{
		"jitter": {
			"Result" : "${jitter_result}",
			"Units" : "${jitter_units}"
		}
	}
EOF
)"
	metrics_json_add_array_element "${json}"
	metrics_json_end_array "Results"
}

function iperf3_parallel() {
	# This will measure four parallel connections with iperf3
	kubectl exec -i "$client_pod_name" -- sh -c "iperf3 -J -c ${server_ip_add} -P 4" | jq '.end.sum_received.bits_per_second' > "${iperf_file}"
	parallel_result=$(cat "${iperf_file}")
	export parallel_result=$(printf "%0.3f\n" $parallel_result)
	export parallel_units="bits per second"

	[ -z "${parallel_result}" ] && die "Iperf3 was unable to collect Parallel workload results."
	[ "$COLLECT_ALL" == "true" ] && return

	metrics_json_init
	metrics_json_start_array

	local json="$(cat << EOF
	{
		"parallel": {
			"Result" : "${parallel_result}",
			"Units" : "${parallel_units}"
		}
	}
EOF
)"
	metrics_json_add_array_element "${json}"
	metrics_json_end_array "Results"
}

function iperf3_cpu() {
	local transmit_timeout="80"

	kubectl exec -i "$client_pod_name" -- sh -c "iperf3 -J -c ${server_ip_add} -t ${transmit_timeout}" | jq '.end.cpu_utilization_percent.host_total' > "${iperf_file}"
	cpu_result=$(cat "${iperf_file}")

        export cpu_result=$(printf "%.3f\n" ${cpu_result})
	export cpu_units="percent"

	[ -z "${cpu_result}" ] && die "Iperf3 was unable to collect CPU workload results."
	[ "$COLLECT_ALL" == "true" ] && return

	metrics_json_init
	metrics_json_start_array

	local json="$(cat << EOF
	{
		"cpu": {
			"Result" : "${cpu_result}",
			"Units"  : "${cpu_units}"
		}
	}
EOF
)"
	metrics_json_add_array_element "${json}"
	metrics_json_end_array "Results"
}

function iperf3_start_deployment() {
	cmds=("bc" "jq")
	check_cmds "${cmds[@]}"

	# Check no processes are left behind
	check_processes

	wait_time=20
	sleep_time=2

	# Create deployment
	kubectl create -f "${IPERF_DEPLOYMENT}"

	# Check deployment creation
	local cmd="kubectl wait --for=condition=Available deployment/iperf3-server-deployment"
	waitForProcess "${wait_time}" "${sleep_time}" "${cmd}"

	# Create DaemonSet
	kubectl create -f "${IPERF_DAEMONSET}"

	# Get the names of the server pod
	export server_pod_name=$(kubectl get pods -o name | grep server | cut -d '/' -f2)

	# Verify the server pod is working
	local cmd="kubectl get pod ${server_pod_name} -o yaml | grep 'phase: Running'"
	waitForProcess "${wait_time}" "${sleep_time}" "${cmd}"

	# Get the names of client pod
	export client_pod_name=$(kubectl get pods -o name | grep client | cut -d '/' -f2)

	# Verify the client pod is working
	local cmd="kubectl get pod ${client_pod_name} -o yaml | grep 'phase: Running'"
	waitForProcess "${wait_time}" "${sleep_time}" "${cmd}"

	# Get the ip address of the server pod
	export server_ip_add=$(kubectl get pod "${server_pod_name}" -o jsonpath='{.status.podIP}')
}

function iperf3_deployment_cleanup() {
	info "Iperf: deleting deployments and services"
	rm -rf "${iperf_file}"
	kubectl delete deployment iperf3-server-deployment
	kubectl delete service iperf3-server
	kubectl delete daemonset iperf3-clients
	kubectl delete pods "${client_pod_name}"
	kill_kata_components && sleep 1
	kill_kata_components
	check_processes
	info "End of Iperf3 test"
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
	local OPTIND
	while getopts ":abcjph:" opt
	do
		case "${opt}" in
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

	init_env
	# The deployment must be removed in
	# any case the script terminates.
	trap iperf3_deployment_cleanup EXIT
	iperf3_start_deployment

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
		export COLLECT_ALL=true
		iperf3_bandwidth
		iperf3_jitter
		iperf3_cpu
		iperf3_parallel
		iperf3_all_collect_results
	fi

	info "Iperf3: saving test results"
	metrics_json_save
}

main "$@"
