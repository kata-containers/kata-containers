#!/usr/bin/env bash
# Copyright (c) 2019 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#
# External dependencies (not present in bare minimum busybox image):
#   - kubectl
#   - yq
#

function adjust_shim_for_nfd() {
	local shim="${1}"
	local expand_runtime_classes_for_nfd="${2}"

	if [[ "${expand_runtime_classes_for_nfd}" == "true" ]]; then
		file="/opt/kata-artifacts/runtimeclasses/kata-${shim}.yaml"
		case "${shim}" in
			*tdx*)
				yq -yi --arg k "tdx.intel.com/keys" '.overhead.podFixed[$k] = 1' "${file}"
				;;
			*snp*)
				yq -yi --arg k "sev-snp.amd.com/esids" '.overhead.podFixed[$k] = 1' "${file}"
				;;
			*)
				;;
		esac
	fi
}

function create_runtimeclasses() {
	echo "Creating the runtime classes"

	local expand_runtime_classes_for_nfd="${1:-false}"

	for shim in "${shims[@]}"; do
		echo "Creating the kata-${shim} runtime class"
		if [ -n "${MULTI_INSTALL_SUFFIX}" ]; then
			sed -i -e "s|kata-${shim}|kata-${shim}-${MULTI_INSTALL_SUFFIX}|g" /opt/kata-artifacts/runtimeclasses/kata-${shim}.yaml
		fi

		adjust_shim_for_nfd "${shim}" "${expand_runtime_classes_for_nfd}"

		kubectl apply -f /opt/kata-artifacts/runtimeclasses/kata-${shim}.yaml

		if [ -n "${MULTI_INSTALL_SUFFIX}" ]; then
			# Move the file back to its original state, as the deletion is done
			# differently in the helm and in the kata-deploy daemonset case, meaning
			# that we should assume those files are always as they were during the
			# time the image was built
			sed -i -e "s|kata-${shim}-${MULTI_INSTALL_SUFFIX}|kata-${shim}|g" /opt/kata-artifacts/runtimeclasses/kata-${shim}.yaml
		fi

	done

	if [[ "${CREATE_DEFAULT_RUNTIMECLASS}" == "true" ]]; then
		if [ -n "${MULTI_INSTALL_SUFFIX}" ]; then
			warn "CREATE_DEFAULT_RUNTIMECLASS is being ignored!"
			warn "multi installation does not support creating a default runtime class"

			return
		fi

		echo "Creating the kata runtime class for the default shim (an alias for kata-${default_shim})"
		cp /opt/kata-artifacts/runtimeclasses/kata-${default_shim}.yaml /tmp/kata.yaml
		sed -i -e 's/name: kata-'${default_shim}'/name: kata/g' /tmp/kata.yaml
		kubectl apply -f /tmp/kata.yaml
		rm -f /tmp/kata.yaml
	fi
}

function delete_runtimeclasses() {
	echo "Deleting the runtime classes"

	for shim in "${shims[@]}"; do
		echo "Deleting the kata-${shim} runtime class"
		canonical_shim_name="kata-${shim}"
		shim_name="${canonical_shim_name}"
		if [ -n "${MULTI_INSTALL_SUFFIX}" ]; then
			shim_name+="-${MULTI_INSTALL_SUFFIX}"
			sed -i -e "s|${canonical_shim_name}|${shim_name}|g" /opt/kata-artifacts/runtimeclasses/${canonical_shim_name}.yaml
		fi

		kubectl delete --ignore-not-found -f /opt/kata-artifacts/runtimeclasses/${canonical_shim_name}.yaml
	done


	if [[ "${CREATE_DEFAULT_RUNTIMECLASS}" == "true" ]]; then
		if [ -n "${MULTI_INSTALL_SUFFIX}" ]; then
			# There's nothing to be done here, as a default runtime class is never created
			# for multi installations
			return
		fi

		echo "Deleting the kata runtime class for the default shim (an alias for kata-${default_shim})"
		cp /opt/kata-artifacts/runtimeclasses/kata-${default_shim}.yaml /tmp/kata.yaml
		sed -i -e 's/name: kata-'${default_shim}'/name: kata/g' /tmp/kata.yaml
		kubectl delete --ignore-not-found -f /tmp/kata.yaml
		rm -f /tmp/kata.yaml
	fi
}

