#!/bin/sh
# Copyright (c) 2019 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#
# External dependencies (not present in bare minimum busybox image):
#   - kubectl
#   - tomlq
#

configure_containerd_runtime() {
	local shim="$2"
	local adjusted_shim_to_multi_install="${shim}"
	local runtime
	local configuration
	local pluginid=cri
	local configuration_file="${containerd_conf_file}"
	local containerd_root_conf_file="$containerd_conf_file"
	local runtime_table
	local runtime_options_table
	local runtime_type
	local runtime_config_path
	local runtime_path
	local key
	local value

	if [ -n "${MULTI_INSTALL_SUFFIX}" ]; then
		adjusted_shim_to_multi_install="${shim}-${MULTI_INSTALL_SUFFIX}"
	fi
	runtime="kata-${adjusted_shim_to_multi_install}"
	configuration="configuration-${shim}"

	# Properly set the configuration file in case drop-in files are supported
	if [ "$use_containerd_drop_in_conf_file" = "true" ]; then
		configuration_file="/host${containerd_drop_in_conf_file}"
	fi

	case "$1" in
		k0s-worker|k0s-controller)
			containerd_root_conf_file="/etc/containerd/containerd.toml"
			;;
	esac

	if grep -q "version = 2\>" $containerd_root_conf_file; then
		pluginid=\"io.containerd.grpc.v1.cri\"
	fi

	if grep -q "version = 3\>" $containerd_root_conf_file; then
		pluginid=\"io.containerd.cri.v1.runtime\"
	fi

	runtime_table=".plugins.${pluginid}.containerd.runtimes.\"${runtime}\""
	runtime_options_table="${runtime_table}.options"
	runtime_type=\"io.containerd."${runtime}".v2\"
	runtime_config_path=\"$(get_kata_containers_config_path "${shim}")/${configuration}.toml\"
	runtime_path=\"$(get_kata_containers_runtime_path "${shim}")\"

	tomlq -i -t $(printf '%s.runtime_type=%s' ${runtime_table} ${runtime_type}) ${configuration_file}
	tomlq -i -t $(printf '%s.runtime_path=%s' ${runtime_table} ${runtime_path}) ${configuration_file}
	tomlq -i -t $(printf '%s.privileged_without_host_devices=true' ${runtime_table}) ${configuration_file}
	case "${shim}" in
		*nvidia-gpu-*)
			tomlq -i -t $(printf '%s.pod_annotations=["io.katacontainers.*","cdi.k8s.io/*"]' ${runtime_table}) ${configuration_file}
			;;
		*)
			tomlq -i -t $(printf '%s.pod_annotations=["io.katacontainers.*"]' ${runtime_table}) ${configuration_file}
			;;
	esac

	tomlq -i -t $(printf '%s.ConfigPath=%s' ${runtime_options_table} ${runtime_config_path}) ${configuration_file}

	if [ "${DEBUG}" = "true" ]; then
		tomlq -i -t '.debug.level = "debug"' ${configuration_file}
	fi

	if [ -n "${SNAPSHOTTER_HANDLER_MAPPING_FOR_ARCH}" ]; then
		for m in ${snapshotters}; do
			key="${m%$snapshotters_delimiter*}"

			if [ "${key}" != "${shim}" ]; then
				continue
			fi

			value="${m#*$snapshotters_delimiter}"
			if [ "${value}" = "nydus" ] && [ -n "${MULTI_INSTALL_SUFFIX}" ]; then
				value="${value}-${MULTI_INSTALL_SUFFIX}"
			fi

			tomlq -i -t $(printf '%s.snapshotter="%s"' ${runtime_table} ${value}) ${configuration_file}
			break
		done
	fi
}

configure_containerd() {
	# Configure containerd to use Kata:
	echo "Add Kata Containers as a supported runtime for containerd"

	mkdir -p /etc/containerd/

	if [ "$use_containerd_drop_in_conf_file" = "false" ] && [ -f "$containerd_conf_file" ]; then
		# only backup in case drop-in files are not supported, and when doing the backup
		# only do it if a backup doesn't already exist (don't override original)
		cp -n "$containerd_conf_file" "$containerd_conf_file_backup"
	fi

	if [ "$use_containerd_drop_in_conf_file" = "true" ]; then
		if ! grep -q "${containerd_drop_in_conf_file}" ${containerd_conf_file}; then
			tomlq -i -t $(printf '.imports|=.+["%s"]' ${containerd_drop_in_conf_file}) ${containerd_conf_file}
		fi
	fi

	for shim in ${shims}; do
		configure_containerd_runtime "$1" $shim
	done
}

cleanup_containerd() {
	if [ "$use_containerd_drop_in_conf_file" = "true" ]; then
		# There's no need to remove the drop-in file, as it'll be removed as
		# part of the artefacts removal.  Thus, simply remove the file from
		# the imports line of the containerd configuration and return.
		tomlq -i -t $(printf '.imports|=.-["%s"]' ${containerd_drop_in_conf_file}) ${containerd_conf_file}
		return
	fi

	rm -f $containerd_conf_file
	if [ -f "$containerd_conf_file_backup" ]; then
		mv "$containerd_conf_file_backup" "$containerd_conf_file"
	fi
}

containerd_snapshotter_version_check() {
	local container_runtime_version
	local containerd_prefix="containerd://"
	local containerd_version_to_avoid="1.6"
	local containerd_version

	container_runtime_version=$(kubectl get node $NODE_NAME -o jsonpath='{.status.nodeInfo.containerRuntimeVersion}')
	containerd_version=${container_runtime_version#$containerd_prefix}

	if echo "${containerd_version}" | grep -q "^${containerd_version_to_avoid}"; then
		if [ -n "${SNAPSHOTTER_HANDLER_MAPPING_FOR_ARCH}" ]; then
			die "kata-deploy only supports snapshotter configuration with containerd 1.7 or newer"
		fi
	fi
}

