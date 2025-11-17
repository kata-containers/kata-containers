#!/usr/bin/env bash
# Copyright (c) 2019 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#
# External dependencies (not present in bare minimum busybox image):
#   - nsenter
#   - tomlq
#

# If we fail for any reason a message will be displayed
die() {
        msg="$*"
        echo "ERROR: $msg" >&2
        exit 1
}

warn() {
        msg="$*"
        echo "WARN: $msg" >&2
}

info() {
	msg="$*"
	echo "INFO: $msg" >&2
}

# Check if a value exists within a specific field in the config file
# * field_contains_value "${config}" "kernel_params" "agent.log=debug"
field_contains_value() {
	local config_file="$1"
	local field="$2"
	local value="$3"
	# Use word boundaries (\b) to match complete parameters, not substrings
	# This handles space-separated values like kernel_params = "param1 param2 param3"
	grep -qE "^${field}[^=]*=.*[[:space:]\"](${value})([[:space:]\"]|$)" "${config_file}"
}

# Get existing values from a TOML array field and return them as a comma-separated string
# * get_field_array_values "${config}" "enable_annotations"
get_field_array_values() {
	local config_file="$1"
	local field="$2"
	# Extract values from field = ["val1", "val2", ...] format
	grep "^${field} = " "${config_file}" | sed "s/^${field} = \[\(.*\)\]/\1/" | sed 's/"//g' | sed 's/, /,/g'
}

# Check if a boolean config is already set to true
config_is_true() {
	local config_file="$1"
	local key="$2"
	grep -qE "^${key}\s*=\s*true" "${config_file}"
}

# Check if a string value already exists anywhere in the file (literal match)
string_exists_in_file() {
	local file_path="$1"
	local string="$2"
	grep -qF "${string}" "${file_path}"
}

function host_systemctl() {
	nsenter --target 1 --mount systemctl "${@}"
}

function host_exec() {
	nsenter --target 1 --mount bash -c "$*"
}

function get_kata_containers_config_path() {
	local shim="$1"

	# Directory holding pristine configuration files for the current default golang runtime.
	local golang_config_path="${dest_dir}/share/defaults/kata-containers"

	# Directory holding pristine configuration files for the new rust runtime.
	#
	# These are put into a separate directory since:
	#
	# - In some cases, the rust runtime configuration syntax is
	#   slightly different to the golang runtime configuration files
	#   so some hypervisors need two different configuration files,
	#   one for reach runtime type (for example Cloud Hypervisor which
	#   uses 'clh' for the golang runtime and 'cloud-hypervisor' for
	#   the rust runtime.
	#
	# - Some hypervisors only currently work with the golang runtime.
	#
	# - Some hypervisors only work with the rust runtime (dragonball).
	#
	# See: https://github.com/kata-containers/kata-containers/issues/6020
	local rust_config_path="${golang_config_path}/runtime-rs"

	local config_path

	# Map the runtime shim name to the appropriate configuration
	# file directory.
	case "$shim" in
		cloud-hypervisor | dragonball | qemu-runtime-rs | qemu-coco-dev-runtime-rs | qemu-se-runtime-rs) config_path="$rust_config_path" ;;
		*) config_path="$golang_config_path" ;;
	esac

	echo "$config_path"
}

function get_kata_containers_runtime_path() {
	local shim="$1"

	local runtime_path
	case "$shim" in
		cloud-hypervisor | dragonball | qemu-runtime-rs | qemu-coco-dev-runtime-rs | qemu-se-runtime-rs)
			runtime_path="${dest_dir}/runtime-rs/bin/containerd-shim-kata-v2"
			;;
		*)
			runtime_path="${dest_dir}/bin/containerd-shim-kata-v2"
			;;
	esac

	echo "$runtime_path"
}

function tdx_not_supported() {
	distro="${1}"
	version="${2}"

	warn "Distro ${distro} ${version} does not support TDX and the TDX related runtime classes will not work in your cluster!"
}

function tdx_supported() {
	distro="${1}"
	version="${2}"
	config="${3}"

	sed -i -e "s|PLACEHOLDER_FOR_DISTRO_QEMU_WITH_TDX_SUPPORT|$(get_tdx_qemu_path_from_distro ${distro})|g" ${config}
	sed -i -e "s|PLACEHOLDER_FOR_DISTRO_OVMF_WITH_TDX_SUPPORT|$(get_tdx_ovmf_path_from_distro ${distro})|g" ${config}

	info "In order to use the tdx related runtime classes, ensure TDX is properly configured for ${distro} ${version} by following the instructions provided at: $(get_tdx_distro_instructions ${distro})"
}

function get_tdx_distro_instructions() {
	distro="${1}"

	case ${distro} in
		ubuntu)
			echo "https://github.com/canonical/tdx/tree/3.3"
			;;
		centos)
			echo "https://sigs.centos.org/virt/tdx"
			;;
	esac
}

function get_tdx_qemu_path_from_distro() {
	distro="${1}"

	case ${distro} in
		ubuntu)
			echo "/usr/bin/qemu-system-x86_64"
			;;
		centos)
			echo "/usr/libexec/qemu-kvm"
			;;
	esac
}

function get_tdx_ovmf_path_from_distro() {
	distro="${1}"

	case ${distro} in
		ubuntu)
			echo "/usr/share/ovmf/OVMF.fd"
			;;
		centos)
			echo "/usr/share/edk2/ovmf/OVMF.inteltdx.fd"
			;;
	esac
}

function adjust_qemu_cmdline() {
	shim="${1}"
	config_path="${2}"
	qemu_share="${shim}"

	# The paths on the kata-containers tarball side look like:
	# ${dest_dir}/opt/kata/share/kata-qemu/qemu
	# ${dest_dir}/opt/kata/share/kata-qemu-snp-experimnental/qemu
	# ${dest_dir}/opt/kata/share/kata-qemu-cca-experimental/qemu
	[[ "${shim}" =~ ^(qemu-nvidia-gpu-snp|qemu-nvidia-gpu-tdx|qemu-cca)$ ]] && qemu_share=${shim}-experimental

	# Both qemu and qemu-coco-dev use exactly the same QEMU, so we can adjust
	# the shim on the qemu-coco-dev case to qemu
	[[ "${shim}" =~ ^(qemu|qemu-coco-dev)$ ]] && qemu_share="qemu"

	qemu_binary=$(tomlq '.hypervisor.qemu.path' ${config_path} | tr -d \")
	qemu_binary_script="${qemu_binary}-installation-prefix"
	qemu_binary_script_host_path="/host/${qemu_binary_script}"

	if [[ ! -f ${qemu_binary_script_host_path} ]]; then
		# From the QEMU man page:
		# ```
		# -L  path
		# 	Set the directory for the BIOS, VGA BIOS and keymaps.
		# 	To list all the data directories, use -L help.
		# ```
		#
		# The reason we have to do this here, is because otherwise QEMU
		# will only look for those files in specific paths, which are
		# tied to the location of the PREFIX used during build time
		# (/opt/kata, in our case).
		cat <<EOF >${qemu_binary_script_host_path}
#!/usr/bin/env bash

exec ${qemu_binary} "\$@" -L ${dest_dir}/share/kata-${qemu_share}/qemu/
EOF
		chmod +x ${qemu_binary_script_host_path}
	fi

	if ! string_exists_in_file "${config_path}" "${qemu_binary_script}"; then
		sed -i -e "s|${qemu_binary}|${qemu_binary_script}|" ${config_path}
	fi
}

