#!/bin/bash
# Copyright (c) 2021, 2022 IBM Corporation
# Copyright (c) 2022, 2023 Red Hat
#
# SPDX-License-Identifier: Apache-2.0
#
# This provides generic functions to use in the tests.
#
set -e

wait_time=60
sleep_time=3

# Delete all pods if any exist, otherwise just return
#
k8s_delete_all_pods_if_any_exists() {
	[ -z "$(kubectl get --no-headers pods)" ] || \
		kubectl delete --all pods
}

FIXTURES_DIR="${BATS_TEST_DIRNAME}/runtimeclass_workloads"

# Wait until the pod is not 'Ready'. Fail if it hits the timeout.
#
# Parameters:
#	$1 - the sandbox ID
#	$2 - wait time in seconds. Defaults to 120. (optional)
#
k8s_wait_pod_be_ready() {
	local pod_name="$1"
	local wait_time="${2:-120}"

	kubectl wait --timeout="${wait_time}s" --for=condition=ready "pods/$pod_name"
}

# Create a pod with a given number of retries if an output includes a timeout.
#
# Parameters:
#	$1 - the pod configuration file.
#
retry_kubectl_apply() {
    local file_path=$1
    local retries=5
    local delay=5
    local attempt=1
    local func_name="${FUNCNAME[0]}"

    while true; do
        output=$(kubectl apply -f "$file_path" 2>&1) || true
        echo ""
        echo "$func_name: Attempt $attempt/$retries"
        echo "$output"

        # Check for timeout and retry if needed
        if echo "$output" | grep -iq "timed out"; then
            if [ $attempt -ge $retries ]; then
                echo "$func_name: Max ${retries} retries reached. Failed due to timeout."
                return 1
            fi
            echo "$func_name: Timeout encountered, retrying in $delay seconds..."
            sleep $delay
            attempt=$((attempt + 1))
            continue
        fi

        # Check for any other kind of error
        if echo "$output" | grep -iq "error"; then
            echo "$func_name: Error detected in kubectl output. Aborting."
            return 1
        fi

        echo "$func_name: Resource created successfully."
        return 0
    done
}

# Create a pod and wait it be ready, otherwise fail.
#
# Parameters:
#	$1 - the pod configuration file.
#	$2 - wait time in seconds. Defaults to 120. (optional)
#
k8s_create_pod() {
	local config_file="$1"
	local wait_time="${2:-120}"
	local pod_name=""

	if [ ! -f "${config_file}" ]; then
		echo "Pod config file '${config_file}' does not exist"
		return 1
	fi

	retry_kubectl_apply "${config_file}"
	if ! pod_name=$(kubectl get pods -o jsonpath='{.items..metadata.name}'); then
		echo "Failed to create the pod"
		return 1
	fi

	if ! k8s_wait_pod_be_ready "${pod_name}" "${wait_time}"; then
		# TODO: run this command for debugging. Maybe it should be
		#       guarded by DEBUG=true?
		kubectl get pods "${pod_name}"
		kubectl describe pod "${pod_name}"
		return 1
	fi
}

# Runs a command in the host filesystem.
#
# Parameters:
#	$1 - the node name
#
exec_host() {
	local node="$1"
	# Validate the node
	if ! kubectl get node "${node}" > /dev/null 2>&1; then
		die "A given node ${node} is not valid"
	fi
	# `kubectl debug` always returns 0, so we hack it to return the right exit code.
	local command="${@:2}"
	# Make 7 character hash from the node name
	local pod_name="custom-node-debugger-$(echo -n "$node" | sha1sum | cut -c1-7)"

	# Run a debug pod
	# Check if there is an existing node debugger pod and reuse it
	# Otherwise, create a new one
	if ! kubectl get pod -n kube-system "${pod_name}" > /dev/null 2>&1; then
		POD_NAME="${pod_name}" NODE_NAME="${node}" envsubst < runtimeclass_workloads/custom-node-debugger.yaml | \
			kubectl apply -n kube-system -f - > /dev/null
		# Wait for the newly created pod to be ready
		kubectl wait pod -n kube-system --timeout="30s" --for=condition=ready "${pod_name}" > /dev/null
		# Manually check the exit status of the previous command to handle errors explicitly
		# since `set -e` is not enabled, allowing subsequent commands to run if needed.
		if [ $? -ne 0 ]; then
			return $?
		fi
	fi

	# Execute the command and capture the output
	# We're trailing the `\r` here due to: https://github.com/kata-containers/kata-containers/issues/8051
	# tl;dr: When testing with CRI-O we're facing the following error:
	# ```
	# (from function `exec_host' in file tests_common.sh, line 51,
	# in test file k8s-file-volume.bats, line 25)
	# `exec_host "echo "$file_body" > $tmp_file"' failed with status 127
	# [bats-exec-test:38] INFO: k8s configured to use runtimeclass
	# bash: line 1: $'\r': command not found
	# ```
	kubectl exec -qi -n kube-system "${pod_name}" -- chroot /host bash -c "${command}" | tr -d '\r'
}

# Check the logged messages on host have a given message.
#
# Parameters:
#	$1 - the k8s worker node name
#	$2 - the syslog identifier as in journalctl's -t option
#	$3 - only logs since date/time (%Y-%m-%d %H:%M:%S)
#	$4 - the message
#
assert_logs_contain() {
	local node="$1"
	local log_id="$2"
	local datetime="$3"
	local message="$4"

	# Note: with image-rs we get more than the default 1000 lines of logs
	exec_host "${node}" journalctl -x -t $log_id --since '"'$datetime'"' | grep "$message"
}

# Create a pod then assert it fails to run. Use in tests that you expect the
# pod creation to fail.
#
# Note: a good testing practice is to afterwards check that the pod creation
# failed because of the expected reason.
#
# Parameters:
#	$1 - the pod configuration file.
# 	$2 - the duration to wait for the container to fail. Defaults to 120. (optional)
#
assert_pod_fail() {
	local container_config="$1"
	local duration="${2:-120}"

	echo "In assert_pod_fail: $container_config"
	echo "Attempt to create the container but it should fail"

	retry_kubectl_apply "${container_config}"
	if ! pod_name=$(kubectl get pods -o jsonpath='{.items..metadata.name}'); then
		echo "Failed to create the pod"
		return 1
	fi

	local elapsed_time=0
	local sleep_time=5
	while true; do
		echo "Waiting for a container to fail"
		sleep ${sleep_time}
		elapsed_time=$((elapsed_time+sleep_time))
		if [[ $(kubectl get pod "${pod_name}" \
			-o jsonpath='{.status.containerStatuses[0].state.waiting.reason}') = *BackOff* ]]; then
			return 0
		fi
		if [ $elapsed_time -gt $duration ]; then
			echo "The container does not get into a failing state" >&2
			break
		fi
	done
	return 1

}

# Check the pulled rootfs on host for given node and sandbox_id
#
# Parameters:
#	$1 - the k8s worker node name
#	$2 - the sandbox id for kata container
#	$3 - the expected count of pulled rootfs
#
assert_rootfs_count() {
	local node="$1"
	local sandbox_id="$2"
	local expect_count="$3"
	local allrootfs=""

	# verify that the sandbox_id is not empty;
	# otherwise, the command $(exec_host $node "find /run/kata-containers/shared/sandboxes/${sandbox_id} -name rootfs -type d")
	# may yield an unexpected count of rootfs.
	if [ -z "$sandbox_id" ]; then
		return 1
	fi

	# Max loop 3 times to get all pulled rootfs for given sandbox_id
	for _ in {1..3}
	do
		allrootfs=$(exec_host $node "find /run/kata-containers/shared/sandboxes/${sandbox_id} -name rootfs -type d")
		if [ -n "$allrootfs" ]; then
			break
		else
			sleep 1
		fi
	done
	echo "allrootfs is: $allrootfs"
	count=$(echo $allrootfs | grep -o "rootfs" | wc -l)
	echo "count of container rootfs in host is: $count, expect count is less than, or equal to: $expect_count"
	[ $expect_count -ge $count ]
}

# Create a pod configuration out of a template file.
#
# Parameters:
#	$1 - the container image.
#	$2 - the runtimeclass, is not optional.
#	$3 - the specific node name, optional.
#
# Return:
# 	the path to the configuration file. The caller should not care about
# 	its removal afterwards as it is created under the bats temporary
# 	directory.
#
new_pod_config() {
	local base_config="${FIXTURES_DIR}/pod-config.yaml.in"
	local image="$1"
	local runtimeclass="$2"
	local new_config

	# The runtimeclass is not optional.
	[ -n "$runtimeclass" ] || return 1

	new_config=$(mktemp "${BATS_FILE_TMPDIR}/$(basename "${base_config}").XXX")
	IMAGE="$image" RUNTIMECLASS="$runtimeclass" envsubst < "$base_config" > "$new_config"

	echo "$new_config"
}

# Set an annotation on configuration metadata.
#
# Usually you will pass a pod configuration file where the 'metadata'
# is relative to the 'root' path. Other configuration files like deployments,
# the annotation should be set on 'spec.template.metadata', so use the 4th
# parameter of this function to pass the base metadata path (for deployments
# cases, it will be 'spec.template' for example).
#
# Parameters:
#	$1 - the yaml file
#	$2 - the annotation key
#	$3 - the annotation value
#	$4 - (optional) base metadata path
set_metadata_annotation() {
	local yaml="${1}"
	local key="${2}"
	local value="${3}"
	local metadata_path="${4:-}"
	local annotation_key=""

	[ -n "$metadata_path" ] && annotation_key+="${metadata_path}."

	# yaml annotation key name.
	annotation_key+="metadata.annotations.\"${key}\""

	echo "$annotation_key"
	# yq set annotations in yaml. Quoting the key because it can have
	# dots.
	yq -i ".${annotation_key} = \"${value}\"" "${yaml}"

	if [[ "${key}" =~ kernel_params ]] && [[ "${KATA_HYPERVISOR}" == "qemu-se" ]]; then
		# A secure boot image for IBM SE should be rebuilt according to the KBS configuration.
		if [ -z "${IBM_SE_CREDS_DIR:-}" ]; then
			>&2 echo "ERROR: IBM_SE_CREDS_DIR is empty"
			return 1
		fi
		repack_secure_image "${value}" "${IBM_SE_CREDS_DIR}" "true"
	fi
}

# Set the command for container spec.
#
# Parameters:
#	$1 - the yaml file
#	$2 - the index of the container
#	$N - the command values
#
set_container_command() {
	local yaml="${1}"
	local container_idx="${2}"
	shift 2

    for command_value in "$@"; do
        yq -i \
          '.spec.containers['"${container_idx}"'].command += ["'"${command_value}"'"]' \
          "${yaml}"
    done
}

# Set the node name on configuration spec.
#
# Parameters:
#	$1 - the yaml file
#	$2 - the node name
#
set_node() {
	local yaml="$1"
	local node="$2"
	[ -n "$node" ] || return 1

  yq -i \
    ".spec.nodeName = \"$node\"" \
    "${yaml}"
}

# Get the sandbox id for kata container from a worker node
#
# Parameters:
#	$1 - the k8s worker node name
#
get_node_kata_sandbox_id() {
	local node="$1"
	local kata_sandbox_id=""
	local local_wait_time="${wait_time}"
	# Max loop 3 times to get kata_sandbox_id
	while [ "$local_wait_time" -gt 0 ];
	do
		kata_sandbox_id=$(exec_host $node "ps -ef |\
		  grep containerd-shim-kata-v2" |\
		  grep -oP '(?<=-id\s)[a-f0-9]+' |\
		  tail -1)
		if [ -n "$kata_sandbox_id" ]; then
			break
		else
			sleep "${sleep_time}"
			local_wait_time=$((local_wait_time-sleep_time))
		fi
	done
	echo $kata_sandbox_id
}
