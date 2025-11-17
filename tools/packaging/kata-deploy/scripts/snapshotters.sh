#!/usr/bin/env bash
# Copyright (c) 2019 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#
# External dependencies (not present in bare minimum busybox image):
#   - kubectl
#   - tomlq
#   - nsenter (via host_systemctl function from utils.sh)
#

function containerd_erofs_snapshotter_version_check() {
	local container_runtime_version=$(kubectl get node $NODE_NAME -o jsonpath='{.status.nodeInfo.containerRuntimeVersion}')
	local containerd_prefix="containerd://"
	local containerd_version=${container_runtime_version#$containerd_prefix}
	local min_version_major="2"
	local min_version_minor="2"

	# Extract major.minor (strip patch and prerelease stuff)
	local major=${containerd_version%%.*}
	local rest=${containerd_version#*.}
	local minor=${rest%%[^0-9]*}

	if [ "${min_version_major}" -gt "${major}" ] || { [ "${min_version_major}" -eq "${major}" ] && [ "${min_version_minor}" -gt "${minor}" ]; }; then
		die "In order to use erofs-snapshotter containerd must be 2.2.0 or newer"
	fi
}

function snapshotter_handler_mapping_validation_check() {
	echo "Validating the snapshotter-handler mapping: \"${SNAPSHOTTER_HANDLER_MAPPING_FOR_ARCH}\""
	if [[ -z "${SNAPSHOTTER_HANDLER_MAPPING_FOR_ARCH}" ]]; then
		echo "No snapshotter has been requested, using the default value from containerd"
		return
	fi

	for m in "${snapshotters[@]}"; do
		shim="${m%$snapshotters_delimiter*}"
		snapshotter="${m#*$snapshotters_delimiter}"

		if [ -z "${shim}" ]; then
			die "The snapshotter must follow the \"shim:snapshotter,shim:snapshotter,...\" format, but at least one shim is empty"
		fi

		if [ -z "${snapshotter}" ]; then
			die "The snapshotter must follow the \"shim:snapshotter,shim:snapshotter,...\" format, but at least one snapshotter is empty"
		fi

		if ! grep -q " ${shim} " <<< " ${SHIMS_FOR_ARCH} "; then
			die "\"${shim}\" is not part of \"${SHIMS_FOR_ARCH}\""
		fi

		matches=$(grep -o "${shim}${snapshotters_delimiter}" <<< "${SNAPSHOTTER_HANDLER_MAPPING_FOR_ARCH}" | wc -l)
		if [[ ${matches} -ne 1 ]]; then
			die "One, and only one, entry per shim is required"
		fi
	done
}

function configure_erofs_snapshotter() {
	info "Configuring erofs-snapshotter"

	# As it's only supported with containerd 2.2.0 or newer
	# we don't even care about the config file format, as
	# it'll always be 3 (at least till version 4 is out).
	#
	# Also, drop-in is always supported on containerd 2.x
	configuration_file="${1}"

	tomlq -i -t $(printf '.plugins."io.containerd.cri.v1.images".discard_unpacked_layers=false') ${configuration_file}

	tomlq -i -t $(printf '.plugins."io.containerd.service.v1.diff-service".default=["erofs","walking"]') ${configuration_file}

	tomlq -i -t $(printf '.plugins."io.containerd.snapshotter.v1.erofs".enable_fsverity=true') ${configuration_file}
	tomlq -i -t $(printf '.plugins."io.containerd.snapshotter.v1.erofs".set_immutable=true') ${configuration_file}
}

function configure_nydus_snapshotter() {
	info "Configuring nydus-snapshotter"

	local nydus="nydus"
	local containerd_nydus="nydus-snapshotter"
	if [[ -n "${MULTI_INSTALL_SUFFIX}" ]]; then
		nydus="${nydus}-${MULTI_INSTALL_SUFFIX}"
		containerd_nydus="${containerd_nydus}-${MULTI_INSTALL_SUFFIX}"
	fi

	configuration_file="${1}"
	pluginid="${2}"

	tomlq -i -t $(printf '.plugins.%s.disable_snapshot_annotations=false' ${pluginid}) ${configuration_file}

	tomlq -i -t $(printf '.proxy_plugins."%s".type="snapshot"' ${nydus} ) ${configuration_file}
	tomlq -i -t $(printf '.proxy_plugins."%s".address="/run/%s/containerd-nydus-grpc.sock"' ${nydus} ${containerd_nydus}) ${configuration_file}
}

function configure_snapshotter() {
	snapshotter="${1}"

	local runtime="$(get_container_runtime)"
	local pluginid="\"io.containerd.grpc.v1.cri\".containerd" # version = 2
	local configuration_file="${containerd_conf_file}"

	# Properly set the configuration file in case drop-in files are supported
	if [[ ${use_containerd_drop_in_conf_file} == "true" ]]; then
		configuration_file="/host${containerd_drop_in_conf_file}"
	fi

	local containerd_root_conf_file="${containerd_conf_file}"
	if [[ "${runtime}" =~ ^(k0s-worker|k0s-controller)$ ]]; then
		containerd_root_conf_file="/etc/containerd/containerd.toml"
	fi

	if grep -q "version = 3\>" ${containerd_root_conf_file}; then
		pluginid=\"io.containerd.cri.v1.images\"
	fi

	case "${snapshotter}" in
		nydus)
			configure_nydus_snapshotter "${configuration_file}" "${pluginid}"

			nydus_snapshotter="nydus-snapshotter"
			if [[ -n "${MULTI_INSTALL_SUFFIX}" ]]; then
				nydus_snapshotter="${nydus_snapshotter}-${MULTI_INSTALL_SUFFIX}"
			fi
			host_systemctl restart "${nydus_snapshotter}"
			;;
		erofs)
			configure_erofs_snapshotter "${configuration_file}"
			;;
	esac
}

function install_nydus_snapshotter() {
	info "Deploying nydus-snapshotter"

	local nydus_snapshotter="nydus-snapshotter"
	if [[ -n "${MULTI_INSTALL_SUFFIX}" ]]; then
		nydus_snapshotter="${nydus_snapshotter}-${MULTI_INSTALL_SUFFIX}"
	fi

	local config_guest_pulling="/opt/kata-artifacts/nydus-snapshotter/config-guest-pulling.toml"
	local nydus_snapshotter_service="/opt/kata-artifacts/nydus-snapshotter/nydus-snapshotter.service"

	# Adjust the paths for the config-guest-pulling.toml and nydus-snapshotter.service
	sed -i -e "s|@SNAPSHOTTER_ROOT_DIR@|/var/lib/${nydus_snapshotter}|g" "${config_guest_pulling}"
	sed -i -e "s|@SNAPSHOTTER_GRPC_SOCKET_ADDRESS@|/run/${nydus_snapshotter}/containerd-nydus-grpc.sock|g" "${config_guest_pulling}"
	sed -i -e "s|@NYDUS_OVERLAYFS_PATH@|${host_install_dir#/host}/nydus-snapshotter/nydus-overlayfs|g" "${config_guest_pulling}"

	sed -i -e "s|@CONTAINERD_NYDUS_GRPC_BINARY@|${host_install_dir#/host}/nydus-snapshotter/containerd-nydus-grpc|g" "${nydus_snapshotter_service}"
	sed -i -e "s|@CONFIG_GUEST_PULLING@|${host_install_dir#/host}/nydus-snapshotter/config-guest-pulling.toml|g" "${nydus_snapshotter_service}"

	mkdir -p "${host_install_dir}/nydus-snapshotter"
	install -D -m 775 /opt/kata-artifacts/nydus-snapshotter/containerd-nydus-grpc "${host_install_dir}/nydus-snapshotter/containerd-nydus-grpc"
	install -D -m 775 /opt/kata-artifacts/nydus-snapshotter/nydus-overlayfs "${host_install_dir}/nydus-snapshotter/nydus-overlayfs"

	install -D -m 644 "${config_guest_pulling}" "${host_install_dir}/nydus-snapshotter/config-guest-pulling.toml"
	install -D -m 644 "${nydus_snapshotter_service}" "/host/etc/systemd/system/${nydus_snapshotter}.service"

	host_systemctl daemon-reload
	host_systemctl enable "${nydus_snapshotter}.service"
}

function uninstall_nydus_snapshotter() {
	info "Removing deployed nydus-snapshotter"

	local nydus_snapshotter="nydus-snapshotter"
	if [[ -n "${MULTI_INSTALL_SUFFIX}" ]]; then
		nydus_snapshotter="${nydus_snapshotter}-${MULTI_INSTALL_SUFFIX}"
	fi

	host_systemctl disable --now "${nydus_snapshotter}.service"

	rm -f "/host/etc/systemd/system/${nydus_snapshotter}.service"
	rm -rf "${host_install_dir}/nydus-snapshotter"

	host_systemctl daemon-reload
}

function install_snapshotter() {
	snapshotter="${1}"

	case "${snapshotter}" in
		erofs) ;; # it's a containerd's built-in snapshotter
		nydus) install_nydus_snapshotter ;;
	esac
}

function uninstall_snapshotter() {
	snapshotter="${1}"

	case "${snapshotter}" in
		nydus) uninstall_nydus_snapshotter ;;
	esac
}

