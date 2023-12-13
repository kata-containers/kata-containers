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

source ${scripts_dir}/../../lib.sh

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

# Leverage kata-deploy to install Kata Containers in the cluster.
#
apply_kata_deploy() {
	local deploy_file="tools/packaging/kata-deploy/kata-deploy/base/kata-deploy.yaml"
	local old_img="quay.io/kata-containers/kata-deploy:latest"
	# Use the kata-deploy CI image which is built for each pull request merged
	local new_img="quay.io/kata-containers/kata-deploy-ci:kata-containers-latest"

	pushd "$katacontainers_repo_dir"
	sed -i "s#${old_img}#${new_img}#" "$deploy_file"

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
	local delta="${1:-900}"
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

# FIXME: Remove when https://github.com/kata-containers/kata-containers/pull/8417 is resolved
# Selinux context is currently not handled by kata-deploy
oc apply -f ${deployments_dir}/relabel_selinux.yaml
( for I in $(seq 30); do
	sleep 10
	oc logs -n kube-system ds/relabel-selinux-daemonset | grep "NSENTER_FINISHED_WITH:" && exit
done ) || { echo "Selinux relabel failed, check the logs"; exit -1; }
