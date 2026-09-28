#!/bin/bash
#
# Copyright (c) 2026 Red Hat, Inc.
#
# SPDX-License-Identifier: Apache-2.0
#
# Shared SELinux helpers for the OCP CI scripts.
#
# SELinux context is currently not handled by kata-deploy, so we ship a custom
# CIL policy and relabel /opt/kata and /var/opt/kata via a daemonset. These
# helpers are sourced by both the bare-metal/VM installer (install_kata.sh) and
# the peer-pods installer (peer-pods-azure.sh) to avoid duplicating the logic.

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
	[[ -z "${pod_count}" ]] && pod_count=1
	[[ -z "${timeout}" ]] && timeout=60
	[[ -n "${namespace}" ]] && namespace=("-n" "${namespace}")
	local pod
	local pods
	SECONDS=0
	while :; do
		mapfile -t pods < <(oc get pods -l app="${app}" --no-headers=true "${namespace[@]}" | awk '{print $1}')
		[[ "${#pods}" -ge "${pod_count}" ]] && break
		if [[ "${SECONDS}" -gt "${timeout}" ]]; then
			printf "Unable to find ${pod_count} pods for '-l app=\"${app}\"' in ${SECONDS}s (%s)" "${pods[@]}"
			return 1
		fi
	done
	local log
	for pod in "${pods[@]}"; do
		while :; do
			log=$(oc logs "${namespace[@]}" "${pod}")
			echo "${log}" | grep "${message}" -q && echo "Found $(echo "${log}" | grep "${message}") in ${pod}'s log (${SECONDS})" && break;
			if [[ "${SECONDS}" -gt "${timeout}" ]]; then
				echo -n "Message '${message}' not present in '${pod}' pod of the '-l app=\"${app}\"' "
				printf "pods after ${SECONDS}s :(%s)\n" "${pods[@]}"
				echo "Pod ${pod}'s output so far:"
				echo "${log}"
				return 1
			fi
			sleep 1;
		done
	done
}

# Install the custom SELinux policy and relabel the kata paths on all nodes.
#
# Must be run *before* kata-deploy so the custom policy is in place while
# kata-deploy (and, for peer-pods, helm) manipulate the kata files.
#
# Params:
#   $1 - directory containing relabel_selinux.yaml
#   $2 - expected number of worker nodes
apply_relabel_selinux() {
	local deployments_dir="$1"
	local num_nodes="$2"

	# Kata and selinux handling requires privileged pods
	oc label --overwrite ns kube-system pod-security.kubernetes.io/enforce=privileged pod-security.kubernetes.io/warn=baseline pod-security.kubernetes.io/audit=baseline

	# Selinux context is currently not handled by kata-deploy
	oc apply -f "${deployments_dir}/relabel_selinux.yaml"
	wait_for_app_pods_message restorecon "${num_nodes}" "NSENTER_FINISHED_WITH:" 120 "kube-system" || echo "Failed to configure selinux, proceeding anyway..."
}

# Re-run the relabel daemonset after kata-deploy has laid down the binaries.
#
# kata-deploy runs without an SELinux context, so /opt and /var need to be
# re-labeled once the files are in place.
#
# Params:
#   $1 - expected number of worker nodes
rerun_relabel_selinux() {
	local num_nodes="$1"

	oc delete -n kube-system -l app=restorecon pods --wait
	wait_for_app_pods_message restorecon "${num_nodes}" "NSENTER_FINISHED_WITH:" 120 "kube-system" || echo "Failed to relable selinux after deployment, proceeding anyway..."
}
