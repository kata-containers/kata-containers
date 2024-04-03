#!/bin/bash
#
# Copyright (c) 2020 Red Hat, Inc.
#
# SPDX-License-Identifier: Apache-2.0
#
# This script installs the built kata-containers in the test cluster,
# and configure a runtime.

scripts_dir=$(dirname $0)
deployments_dir=${scripts_dir}/deployments
configs_dir=${scripts_dir}/configs

source ${scripts_dir}/../lib.sh

# Set to 'yes' if you want to configure SELinux to permissive on the cluster
# workers.
#
SELINUX_PERMISSIVE=${SELINUX_PERMISSIVE:-no}

# Set to 'yes' if you want to configure Kata Containers to use the system's
# QEMU (from the RHCOS extension).
#
KATA_WITH_SYSTEM_QEMU=${KATA_WITH_SYSTEM_QEMU:-no}

# Set to 'yes' if you want to configure Kata Containers to use the host kernel.
#
KATA_WITH_HOST_KERNEL=${KATA_WITH_HOST_KERNEL:-no}

# kata-deploy image to be used to deploy the kata (by default use CI image
# that is built for each pull request)
#
KATA_DEPLOY_IMAGE=${KATA_DEPLOY_IMAGE:-quay.io/kata-containers/kata-deploy-ci:kata-containers-latest}

# Enable workaround for OCP 4.13 https://github.com/kata-containers/kata-containers/pull/9206
#
WORKAROUND_9206_CRIO=${WORKAROUND_9206_CRIO:-no}

# Leverage kata-deploy to install Kata Containers in the cluster.
#
apply_kata_deploy() {
	local deploy_file="tools/packaging/kata-deploy/kata-deploy/base/kata-deploy.yaml"
	pushd "$katacontainers_repo_dir"
	sed -ri "s#(\s+image:) .*#\1 ${KATA_DEPLOY_IMAGE}#" "$deploy_file"

	info "Applying kata-deploy"
	oc apply -f tools/packaging/kata-deploy/kata-rbac/base/kata-rbac.yaml
	oc label --overwrite ns kube-system pod-security.kubernetes.io/enforce=privileged pod-security.kubernetes.io/warn=baseline pod-security.kubernetes.io/audit=baseline
	oc apply -f "$deploy_file"
	oc -n kube-system wait --timeout=10m --for=condition=Ready -l name=kata-deploy pod

	info "Adding the kata runtime classes"
	oc apply -f tools/packaging/kata-deploy/runtimeclasses/kata-runtimeClasses.yaml
	popd
}


# Wait all worker nodes reboot.
#
# Params:
#   $1 - timeout in seconds (default to 900).
#
wait_for_reboot() {
	local delta="${1:-900}"
	local sleep_time=60
	declare -A BOOTIDS
	local workers=($(oc get nodes | \
		awk '{if ($3 == "worker") { print $1 } }'))
	# Get the boot ID to compared it changed over time.
	for node in ${workers[@]}; do
		BOOTIDS[$node]=$(oc get -o jsonpath='{.status.nodeInfo.bootID}'\
			node/$node)
		echo "Wait $node reboot"
	done

	echo "Set timeout to $delta seconds"
	timer_start=$(date +%s)
	while [ ${#workers[@]} -gt 0 ]; do
		sleep $sleep_time
		now=$(date +%s)
		if [ $(($timer_start + $delta)) -lt $now ]; then
			echo "Timeout: not all workers rebooted"
			return 1
		fi
		echo "Checking after $(($now - $timer_start)) seconds"
		for i in ${!workers[@]}; do
			current_id=$(oc get \
				-o jsonpath='{.status.nodeInfo.bootID}' \
				node/${workers[i]})
			if [ "$current_id" != ${BOOTIDS[${workers[i]}]} ]; then
				echo "${workers[i]} rebooted"
				unset workers[i]
			fi
		done
	done
}

wait_mcp_update() {
	local delta="${1:-3600}"
	local sleep_time=30
	# The machineconfigpool is fine when all the workers updated and are ready,
	# and none are degraded.
	local ready_count=0
	local degraded_count=0
	local machine_count=$(oc get mcp worker -o jsonpath='{.status.machineCount}')

	if [[ -z "$machine_count" && "$machine_count" -lt 1 ]]; then
		warn "Unabled to obtain the machine count"
		return 1
	fi

	echo "Set timeout to $delta seconds"
	local deadline=$(($(date +%s) + $delta))
	# The ready count might not have changed yet, so wait a little.
	while [[ "$ready_count" != "$machine_count" && \
		"$degraded_count" == 0 ]]; do
		# Let's check it hit the timeout (or not).
		local now=$(date +%s)
		if [ $deadline -lt $now ]; then
			echo "Timeout: not all workers updated" >&2
			return 1
		fi
		sleep $sleep_time
		ready_count=$(oc get mcp worker \
			-o jsonpath='{.status.readyMachineCount}')
		degraded_count=$(oc get mcp worker \
			-o jsonpath='{.status.degradedMachineCount}')
		echo "check machineconfigpool - ready_count: $ready_count degraded_count: $degraded_count"
	done
	[ $degraded_count -eq 0 ]
}

# Enable the RHCOS extension for the Sandboxed Containers.
#
enable_sandboxedcontainers_extension() {
	info "Enabling the RHCOS extension for Sandboxed Containers"
	local deployment_file="${deployments_dir}/machineconfig_sandboxedcontainers_extension.yaml"
	oc apply -f ${deployment_file}
	oc get -f ${deployment_file} || \
		die "Sandboxed Containers extension machineconfig not found"
	wait_mcp_update || die "Failed to update the machineconfigpool"
}

# Print useful information for debugging.
#
# Params:
#   $1 - the pod name
debug_pod() {
	local pod="$1"
	info "Debug pod: ${pod}"
	oc describe pods "$pod"
        oc logs "$pod"
}

# Wait for all pods of the app label to contain expected message
#
# Params:
#   $1 - app labela
#   $2 - expected pods count (>=1)
#   $3 - message to be present in the logs
#   $4 - timeout (60)
#   $5 - namespace (the current one)
wait_for_app_pods_message() {
	local app="$1"
	local pod_count="$2"
	local message="$3"
	local timeout="$4"
	local namespace="$5"
	[ -z "$pod_count" ] && pod_count=1
	[ -z "$timeout" ] && timeout=60
	[ -n "$namespace" ] && namespace=" -n $namespace "
	local pod
	local pods
	local i
	SECONDS=0
	while :; do
		pods=($(oc get pods -l app="$app" --no-headers=true $namespace | awk '{print $1}'))
		[ "${#pods}" -ge "$pod_count" ] && break
		if [ "$SECONDS" -gt "$timeout" ]; then
			echo "Unable to find ${pod_count} pods for '-l app=\"$app\"' in ${SECONDS}s (${pods[@]})"
			return -1
		fi
	done
	for pod in "${pods[@]}"; do
		while :; do
			local log=$(oc logs $namespace "$pod")
			echo "$log" | grep "$message" -q && echo "Found $(echo "$log" | grep "$message") in $pod's log ($SECONDS)" && break;
			if [ "$SECONDS" -gt "$timeout" ]; then
				echo -n "Message '$message' not present in '${pod}' pod of the '-l app=\"$app\"' "
				echo "pods after ${SECONDS}s (${pods[@]})"
				echo "Pod $pod's output so far:"
				echo "$log"
				return -1
			fi
			sleep 1;
		done
	done
}

oc config set-context --current --namespace=default

worker_nodes=$(oc get nodes |  awk '{if ($3 == "worker") { print $1 } }')
num_nodes=$(echo $worker_nodes | wc -w)
[ $num_nodes -ne 0 ] || \
	die "No worker nodes detected. Something is wrong with the cluster"

if [ "${KATA_WITH_SYSTEM_QEMU}" == "yes" ]; then
	# QEMU is deployed on the workers via RCHOS extension.
	enable_sandboxedcontainers_extension
	oc apply -f ${deployments_dir}/configmap_installer_qemu.yaml
fi

if [ "${KATA_WITH_HOST_KERNEL}" == "yes" ]; then
	oc apply -f ${deployments_dir}/configmap_installer_kernel.yaml
fi

apply_kata_deploy

# Set SELinux to permissive mode
if [ ${SELINUX_PERMISSIVE} == "yes" ]; then
	info "Configuring SELinux"
	if [ -z "$SELINUX_CONF_BASE64" ]; then
		export SELINUX_CONF_BASE64=$(echo \
			$(cat $configs_dir/selinux.conf|base64) | \
			sed -e 's/\s//g')
	fi
	envsubst < ${deployments_dir}/machineconfig_selinux.yaml.in | \
		oc apply -f -
	oc get machineconfig/51-kata-selinux || \
		die "SELinux machineconfig not found"
	# The new SELinux configuration will trigger another reboot.
	wait_for_reboot
fi

if [[ "$WORKAROUND_9206_CRIO" == "yes" ]]; then
	info "Applying workaround to enable skip_mount_home in crio on OCP 4.13"
	oc apply -f "${deployments_dir}/workaround-9206-crio.yaml"
	oc apply -f "${deployments_dir}/workaround-9206-crio-ds.yaml"
	wait_for_app_pods_message workaround-9206-crio-ds "$num_nodes" "Config file present" 1200 || echo "Failed to apply the workaround, proceeding anyway..."
fi

# FIXME: Remove when https://github.com/kata-containers/kata-containers/pull/8417 is resolved
# Selinux context is currently not handled by kata-deploy
oc apply -f ${deployments_dir}/relabel_selinux.yaml
wait_for_app_pods_message restorecon "$num_nodes" "NSENTER_FINISHED_WITH:" 120 "kube-system" || echo "Failed to treat selinux, proceeding anyway..."
