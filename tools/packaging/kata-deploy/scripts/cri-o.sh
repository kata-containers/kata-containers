#!/bin/sh
# Copyright (c) 2019 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#
# External dependencies (not present in bare minimum busybox image):
#   - (none - only uses shell builtins and busybox commands)
#

configure_crio_runtime() {
	local shim="${1}"
	local adjusted_shim_to_multi_install="${shim}"
	local runtime
	local configuration
	local config_path
	local kata_path
	local kata_conf
	local kata_config_path
	local key
	local value

	if [ -n "${MULTI_INSTALL_SUFFIX}" ]; then
		adjusted_shim_to_multi_install="${shim}-${MULTI_INSTALL_SUFFIX}"
	fi
	runtime="kata-${adjusted_shim_to_multi_install}"
	configuration="configuration-${shim}"

	config_path=$(get_kata_containers_config_path "${shim}")

	kata_path=$(get_kata_containers_runtime_path "${shim}")
	kata_conf="crio.runtime.runtimes.${runtime}"
	kata_config_path="${config_path}/${configuration}.toml"

	cat <<EOF | tee -a "$crio_drop_in_conf_file"

[$kata_conf]
	runtime_path = "${kata_path}"
	runtime_type = "vm"
	runtime_root = "/run/vc"
	runtime_config_path = "${kata_config_path}"
	privileged_without_host_devices = true
EOF

	if [ -n "${PULL_TYPE_MAPPING_FOR_ARCH}" ]; then
		for m in ${pull_types}; do
			key="${m%"$snapshotters_delimiter"*}"
			value="${m#*"$snapshotters_delimiter"}"

			if [ "${value}" = "default" ] || [ "${key}" != "${shim}" ]; then
				continue
			fi

			if [ "${value}" = "guest-pull" ]; then
				echo -e "\truntime_pull_image = true" | \
					tee -a "${crio_drop_in_conf_file}"
			else
				die "Unsupported pull type '${value}' for ${shim}"
			fi
			break
		done
	fi
}

configure_crio() {
	# Configure crio to use Kata:
	echo "Add Kata Containers as a supported runtime for CRIO:"

	# As we don't touch the original configuration file in any way,
	# let's just ensure we remove any exist configuration from a
	# previous deployment.
	mkdir -p "$crio_drop_in_conf_dir"
	rm -f "$crio_drop_in_conf_file"
	touch "$crio_drop_in_conf_file"
	rm -f "$crio_drop_in_conf_file_debug"
	touch "$crio_drop_in_conf_file_debug"

	# configure storage option for crio
	cat <<EOF | tee -a "$crio_drop_in_conf_file"
[crio]
  storage_option = [
	"overlay.skip_mount_home=true",
  ]
EOF

	# configure runtimes for crio
	for shim in ${shims}; do
		configure_crio_runtime $shim
	done

	if [ "${DEBUG}" = "true" ]; then
		cat <<EOF | tee $crio_drop_in_conf_file_debug
[crio.runtime]
log_level = "debug"
EOF
	fi
}

cleanup_crio() {
	rm -f $crio_drop_in_conf_file
	if [ "${DEBUG}" = "true" ]; then
		rm -f $crio_drop_in_conf_file_debug
	fi
}

