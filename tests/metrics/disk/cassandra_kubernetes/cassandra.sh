#!/bin/bash
#
# Copyright (c) 2022 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

set -e
set -x

SCRIPT_PATH=$(dirname "$(readlink -f "$0")")

source "${SCRIPT_PATH}/../../../.ci/lib.sh"
source "${SCRIPT_PATH}/../../lib/common.bash"
test_repo="${test_repo:-github.com/kata-containers/tests}"
TEST_NAME="${TEST_NAME:-cassandra}"
cassandra_file=$(mktemp cassandraresults.XXXXXXXXXX)
cassandra_read_file=$(mktemp cassandrareadresults.XXXXXXXXXX)

function remove_tmp_file() {
	rm -rf "${cassandra_file}" "${cassandra_read_file}"
}

trap remove_tmp_file EXIT

function cassandra_write_test() {
	cassandra_start
	export pod_name="cassandra-0"
	export write_cmd="/usr/local/apache-cassandra-3.11.2/tools/bin/cassandra-stress write n=1000000 cl=one -mode native cql3 -schema keyspace="keyspace1" -pop seq=1..1000000 -node cassandra"
 	number_of_retries="50"
	for _ in $(seq 1 "$number_of_retries"); do
		if kubectl exec -i cassandra-0 -- sh -c 'nodetool status' | grep Up; then
			ok="1"
 			break;
		fi
 		sleep 1
	done
	# This is needed to wait that cassandra is up
	sleep 30
	kubectl exec -i cassandra-0 -- sh -c "$write_cmd" > "${cassandra_file}"
	write_op_rate=$(cat "${cassandra_file}" | grep -e "Op rate" | cut -d':' -f2  | sed -e 's/^[ \t]*//' | cut -d ' ' -f1)
	write_latency_mean=$(cat "${cassandra_file}" | grep -e "Latency mean" | cut -d':' -f2  | sed -e 's/^[ \t]*//' | cut -d ' ' -f1)
	write_latency_95th=$(cat "${cassandra_file}" | grep -e "Latency 95th percentile" | cut -d':' -f2  | sed -e 's/^[ \t]*//' | cut -d ' ' -f1)
	write_latency_99th=$(cat "${cassandra_file}" | grep -e "Latency 99th percentile" | cut -d':' -f2  | sed -e 's/^[ \t]*//' | cut -d ' ' -f1)
	write_latency_median=$(cat "${cassandra_file}" | grep -e "Latency median" | cut -d':' -f2  | sed -e 's/^[ \t]*//' | cut -d ' ' -f1)

	export read_cmd="/usr/local/apache-cassandra-3.11.2/tools/bin/cassandra-stress read n=200000 -rate threads=50"
	kubectl exec -i cassandra-0 -- sh -c "$read_cmd" > "${cassandra_read_file}"
	read_op_rate=$(cat "${cassandra_read_file}" | grep -e "Op rate" | cut -d':' -f2  | sed -e 's/^[ \t]*//' | cut -d ' ' -f1)
	read_latency_mean=$(cat "${cassandra_read_file}" | grep -e "Latency mean" | cut -d':' -f2  | sed -e 's/^[ \t]*//' | cut -d ' ' -f1)
	read_latency_95th=$(cat "${cassandra_read_file}" | grep -e "Latency 95th percentile" | cut -d':' -f2  | sed -e 's/^[ \t]*//' | cut -d ' ' -f1)
	read_latency_99th=$(cat "${cassandra_read_file}" | grep -e "Latency 99th percentile" | cut -d':' -f2  | sed -e 's/^[ \t]*//' | cut -d ' ' -f1)
	read_latency_median=$(cat "${cassandra_read_file}" | grep -e "Latency median" | cut -d':' -f2  | sed -e 's/^[ \t]*//' | cut -d ' ' -f1)

	metrics_json_init
	# Save configuration
	metrics_json_start_array

	local json="$(cat << EOF
	{
		"Write Op rate": {
			"Result" : "$write_op_rate",
			"Units" : "op/s"
		},
		"Write Latency Mean": {
			"Result" : "$write_latency_mean",
			"Units" : "ms"
		},
		"Write Latency 95th percentile": {
			"Result" : "$write_latency_95th",
			"Units" : "ms"
		},
		"Write Latency 99th percentile": {
			"Result" : "$write_latency_99th",
			"Units" : "ms"
		},
		"Write Latency Median" : {
			"Result" : "$write_latency_median",
			"Units" : "ms"
		},
		"Read Op rate": {
			"Result" : "$read_op_rate",
			"Units" : "op/s"
		},
		"Read Latency Mean": {
			"Result" : "$read_latency_mean",
			"Units" : "ms"
		},
		"Read Latency 95th percentile": {
			"Result" : "$read_latency_95th",
			"Units" : "ms"
		},
		"Read Latency 99th percentile": {
			"Result" : "$read_latency_99th",
			"Units" : "ms"
		},
		"Read Latency Median" : {
			"Result" : "$read_latency_median",
			"Units" : "ms"
		}
	}
EOF
)"

	metrics_json_add_array_element "$json"
	metrics_json_end_array "Results"

	metrics_json_save
	cassandra_cleanup
}

function cassandra_start() {
	cmds=("bc" "jq")
	check_cmds "${cmds[@]}"

	# Check no processes are left behind
	check_processes

	# Start kubernetes
	start_kubernetes

	export KUBECONFIG="$HOME/.kube/config"
	export service_name="cassandra"
	export app_name="cassandra"

	wait_time=20
 	sleep_time=2

	vol_capacity="3Gi"
	volume_name="block-loop-pv"
	volume_claim="block-loop-pvc"

	# Create Loop Device
	export tmp_disk_image=$(mktemp --tmpdir disk.XXXXXX.img)
	truncate "$tmp_disk_image" --size "3GB"
	export loop_dev=$(sudo losetup -f)
	sudo losetup "$loop_dev" "$tmp_disk_image"

	# Create Storage Class
	kubectl create -f "${SCRIPT_PATH}/volume/block-local-storage.yaml"

	# Create Persistent Volume
	export tmp_pv_yaml=$(mktemp --tmpdir block_persistent_vol.XXXXX.yaml)
	sed -e "s|LOOP_DEVICE|${loop_dev}|" "${SCRIPT_PATH}/volume/block-loop-pv.yaml" > "$tmp_pv_yaml"
	sed -i "s|HOSTNAME|$(hostname | awk '{print tolower($0)}')|" "$tmp_pv_yaml"
	sed -i "s|CAPACITY|${vol_capacity}|" "$tmp_pv_yaml"

	kubectl create -f "$tmp_pv_yaml"
	cmd="kubectl get pv/${volume_name} | grep Available"
	waitForProcess "$wait_time" "$sleep_time" "$cmd"

	# Create Persistent Volume Claim
	export tmp_pvc_yaml=$(mktemp --tmpdir block_persistent_vol.XXXXX.yaml)
	sed -e "s|CAPACITY|${vol_capacity}|" "${SCRIPT_PATH}/volume/block-loop-pvc.yaml" > "$tmp_pvc_yaml"
	kubectl create -f "$tmp_pvc_yaml"

	# Create service
	kubectl create -f "${SCRIPT_PATH}/runtimeclass_workloads/cassandra-service.yaml"

	# Check service
	kubectl get svc | grep "$service_name"

	# Create workload using volume
	ctr_dev_path="/dev/xda"
	export tmp_pod_yaml=$(mktemp --tmpdir pod-pv.XXXXX.yaml)
	sed -e "s|DEVICE_PATH|${ctr_dev_path}|" "${SCRIPT_PATH}/runtimeclass_workloads/cassandra-statefulset.yaml" > "$tmp_pod_yaml"
	kubectl create -f "$tmp_pod_yaml"
	cmd="kubectl rollout status --watch --timeout=120s statefulset/$app_name"
 	waitForProcess "$wait_time" "$sleep_time" "$cmd"

	# Verify persistent volume claim is bound
	kubectl get pvc | grep "Bound"

	# Check pods are running
	cmd="kubectl get pods -o jsonpath='{.items[*].status.phase}' | grep Running"
	waitForProcess "$wait_time" "$sleep_time" "$cmd"
}

function cassandra_cleanup() {
	kubectl patch pvc block-loop-pvc -p '{"metadata":{"finalizers":null}}'
	kubectl delete pvc block-loop-pvc --force
	kubectl patch pv block-loop-pv -p '{"metadata":{"finalizers":null}}'
	kubectl delete pv block-loop-pv --force
	kubectl delete svc "$service_name"
	kubectl delete pod -l app="$app_name"
	kubectl delete storageclass block-local-storage
	kubectl delete statefulsets "$app_name"

	# Delete temporary yaml files
	rm -f "$tmp_pv_yaml"
	rm -f "$tmp_pvc_yaml"
	rm -f "$tmp_pod_yaml"

	# Remove image and loop device
	sudo losetup -d "$loop_dev"
	rm -f "$tmp_disk_image"

	end_kubernetes
	check_processes
}

function start_kubernetes() {
	info "Start k8s"
	pushd "${GOPATH}/src/${test_repo}/integration/kubernetes"
	bash ./init.sh
	popd
}

function end_kubernetes() {
	info "End k8s"
	pushd "${GOPATH}/src/${test_repo}/integration/kubernetes"
	bash ./cleanup_env.sh
	popd
}

function main() {
	init_env
	cassandra_write_test
}

main "$@"
