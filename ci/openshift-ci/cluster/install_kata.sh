#!/bin/bash
#
# Copyright (c) 2020 Red Hat, Inc.
#
# SPDX-License-Identifier: Apache-2.0
#
# This script installs the built kata-containers in the test cluster,
# and configure a runtime.

scripts_dir=$(dirname "$0")
deployments_dir=${scripts_dir}/deployments
configs_dir=${scripts_dir}/configs

# shellcheck disable=SC1091 # import based on variable
source "${scripts_dir}/../lib.sh"
# shellcheck disable=SC1091 # import based on variable
source "${scripts_dir}/selinux_helpers.sh"

# Set your katacontainers repo dir location
[[ -z "${katacontainers_repo_dir}" ]] && echo "Please set katacontainers_repo_dir variable to your kata repo"

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
	if ! command -v helm &>/dev/null; then
		echo "Helm not installed, installing in current location..."
		PATH=".:${PATH}"
		curl -fsSL https://raw.githubusercontent.com/helm/helm/main/scripts/get-helm-3 | HELM_INSTALL_DIR='.' bash -s -- --no-sudo
	fi

	local version chart
	version='0.0.0-dev'
	chart="oci://ghcr.io/kata-containers/kata-deploy-charts/kata-deploy"

	# Ensure any potential leftover is cleaned up ... and this secret usually is not in case of previous failures
	oc delete secret sh.helm.release.v1.kata-deploy.v1 -n kube-system || true

	echo "Installing kata using helm ${chart} ${version} (sha printed in helm output)"
	helm install kata-deploy --wait --namespace kube-system --set "image.reference=${KATA_DEPLOY_IMAGE%%:*},image.tag=${KATA_DEPLOY_IMAGE##*:}" "${chart}" --version "${version}"
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
	local workers
	mapfile -t workers < <(oc get nodes | awk '{if ($3 == "worker") { print $1 } }')
	# Get the boot ID to compared it changed over time.
	for node in "${workers[@]}"; do
		BOOTIDS[${node}]=$(oc get -o jsonpath='{.status.nodeInfo.bootID}'\
			"node/${node}")
		echo "Wait ${node} reboot"
	done

	echo "Set timeout to ${delta} seconds"
	timer_start=$(date +%s)
	while [[ ${#workers[@]} -gt 0 ]]; do
		sleep "${sleep_time}"
		now=$(date +%s)
		if [[ $((timer_start + delta)) -lt ${now} ]]; then
			echo "Timeout: not all workers rebooted"
			return 1
		fi
		echo "Checking after $((now - timer_start)) seconds"
		for i in "${!workers[@]}"; do
			current_id=$(oc get \
				-o jsonpath='{.status.nodeInfo.bootID}' \
				"node/${workers[i]}")
			if [[ "${current_id}" != "${BOOTIDS[${workers[i]}]}" ]]; then
				echo "${workers[i]} rebooted"
				unset "workers[i]"
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
	local machine_count
	machine_count=$(oc get mcp worker -o jsonpath='{.status.machineCount}')

	if [[ -z "${machine_count}" && "${machine_count}" -lt 1 ]]; then
		warn "Unabled to obtain the machine count"
		return 1
	fi

	echo "Set timeout to ${delta} seconds"
	local deadline=$(($(date +%s) + delta))
	local now
	# The ready count might not have changed yet, so wait a little.
	while [[ "${ready_count}" != "${machine_count}" && \
		"${degraded_count}" == 0 ]]; do
		# Let's check it hit the timeout (or not).
		now=$(date +%s)
		if [[ ${deadline} -lt ${now} ]]; then
			echo "Timeout: not all workers updated" >&2
			return 1
		fi
		sleep "${sleep_time}"
		ready_count=$(oc get mcp worker \
			-o jsonpath='{.status.readyMachineCount}')
		degraded_count=$(oc get mcp worker \
			-o jsonpath='{.status.degradedMachineCount}')
		echo "check machineconfigpool - ready_count: ${ready_count} degraded_count: ${degraded_count}"
	done
	[[ ${degraded_count} -eq 0 ]]
}

# Enable the RHCOS extension for the Sandboxed Containers.
#
enable_sandboxedcontainers_extension() {
	info "Enabling the RHCOS extension for Sandboxed Containers"
	local deployment_file="${deployments_dir}/machineconfig_sandboxedcontainers_extension.yaml"
	oc apply -f "${deployment_file}"
	oc get -f "${deployment_file}" || \
		die "Sandboxed Containers extension machineconfig not found"
	wait_mcp_update 3600 || die "Failed to update the machineconfigpool"
}

# Print useful information for debugging.
#
# Params:
#   $1 - the pod name
debug_pod() {
	local pod="$1"
	info "Debug pod: ${pod}"
	oc describe pods "${pod}"
        oc logs "${pod}"
}

oc config set-context --current --namespace=default

worker_nodes=$(oc get nodes |  awk '{if ($3 == "worker") { print $1 } }')
num_nodes=$(echo "${worker_nodes}" | wc -w)
[[ ${num_nodes} -ne 0 ]] || \
	die "No worker nodes detected. Something is wrong with the cluster"

if [[ "${KATA_WITH_SYSTEM_QEMU}" == "yes" ]]; then
	# QEMU is deployed on the workers via RCHOS extension.
	enable_sandboxedcontainers_extension
	oc apply -f "${deployments_dir}/configmap_installer_qemu.yaml"
fi

if [[ "${KATA_WITH_HOST_KERNEL}" == "yes" ]]; then
	oc apply -f "${deployments_dir}/configmap_installer_kernel.yaml"
fi

# Selinux context is currently not handled by kata-deploy
apply_relabel_selinux "${deployments_dir}" "${num_nodes}"

apply_kata_deploy

# Kata-deploy runs without selinux, we need to re-lable the /opt and /var
rerun_relabel_selinux "${num_nodes}"

# Set SELinux to permissive mode
if [[ ${SELINUX_PERMISSIVE} == "yes" ]]; then
	info "Configuring SELinux"
	if [[ -z "${SELINUX_CONF_BASE64}" ]]; then
		SELINUX_CONF_BASE64=$(base64 -w0 < "${configs_dir}/selinux.conf")
		export SELINUX_CONF_BASE64
	fi
	envsubst < "${deployments_dir}"/machineconfig_selinux.yaml.in | \
		oc apply -f -
	oc get machineconfig/51-kata-selinux || \
		die "SELinux machineconfig not found"
	# The new SELinux configuration will trigger another reboot.
	wait_for_reboot 900
fi

if [[ "${WORKAROUND_9206_CRIO}" == "yes" ]]; then
	info "Applying workaround to enable skip_mount_home in crio on OCP 4.13"
	oc apply -f "${deployments_dir}/workaround-9206-crio.yaml"
	oc apply -f "${deployments_dir}/workaround-9206-crio-ds.yaml"
	wait_for_app_pods_message workaround-9206-crio-ds "${num_nodes}" "Config file present" 1200 || echo "Failed to apply the workaround, proceeding anyway..."
fi
