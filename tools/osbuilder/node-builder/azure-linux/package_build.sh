#!/usr/bin/env bash
#
# Copyright (c) 2024 Microsoft Corporation
#
# SPDX-License-Identifier: Apache-2.0

set -o errexit
set -o pipefail
set -o errtrace

[[ -n "${DEBUG:-}" ]] && set -x

AGENT_BUILD_TYPE=${AGENT_BUILD_TYPE:-release}
CONF_PODS=${CONF_PODS:-no}

script_dir="$(dirname "$(readlink -f "$0")")"
repo_dir="${script_dir}/../../../../"

common_file="${script_dir}/common.sh"
# shellcheck source=tools/osbuilder/node-builder/azure-linux/common.sh
source "${common_file}"

runtime_go_make_flags=(
	"SKIP_GO_VERSION_CHECK=1"
	"QEMUCMD="
	"FCCMD="
	"ACRNCMD="
	"STRATOVIRTCMD="
	"DEFAULT_HYPERVISOR=cloud-hypervisor"
	"DEFVIRTIOFSDAEMON=${VIRTIOFSD_BINARY_LOCATION}"
	"PREFIX=${INSTALL_PATH_PREFIX}"
)

runtime_rs_make_flags=(
	"BUILD_TYPE=release"
	"LIBC=gnu"
	"HYPERVISOR=cloud-hypervisor"
	"OPENSSL_NO_VENDOR=Y"
	"USE_BUILDIN_DB=false"
	"QEMUCMD="
	"FCCMD="
	"DEFVIRTIOFSDAEMON=${VIRTIOFSD_BINARY_LOCATION}"
	"PREFIX=${INSTALL_PATH_PREFIX}"
)

# - for vanilla Kata we use the kernel binary. For ConfPods we use IGVM, so no need to provide kernel path.
# - for vanilla Kata we explicitly set DEFSTATICRESOURCEMGMT_CLH. For ConfPods,
#   the variable DEFSTATICRESOURCEMGMT_TEE is used which defaults to false
# - for ConfPods we explicitly set the cloud-hypervisor path. The path is independent of the PREFIX variable
#   as we have a single CLH binary for both vanilla Kata and ConfPods
if [[ "${CONF_PODS}" == "no" ]]; then
	runtime_go_make_flags+=("DEFSTATICRESOURCEMGMT_CLH=true" "KERNELPATH_CLH=${KERNEL_BINARY_LOCATION}")
	runtime_rs_make_flags+=("DEFSTATICRESOURCEMGMT_CLH=true" "KERNELPATH_CLH=${KERNEL_BINARY_LOCATION}")
else
	runtime_go_make_flags+=("CLHPATH=${CLOUD_HYPERVISOR_LOCATION}")
	runtime_rs_make_flags+=("CLHPATH=${CLOUD_HYPERVISOR_LOCATION}")
fi

# On Mariner 3.0 we use cgroupsv2 with a single sandbox cgroup
if [[ "${OS_VERSION}" == "3.0" ]]; then
	runtime_go_make_flags+=("DEFSANDBOXCGROUPONLY=true")
	runtime_rs_make_flags+=("DEFSANDBOXCGROUPONLY_CLH=true")
fi

agent_make_flags=(
	"LIBC=gnu"
	"OPENSSL_NO_VENDOR=Y"
	"DESTDIR=${AGENT_INSTALL_DIR}"
	"BUILD_TYPE=${AGENT_BUILD_TYPE}"
)

if [[ "${CONF_PODS}" == "yes" ]]; then
	agent_make_flags+=("AGENT_POLICY=yes")
fi

pushd "${repo_dir}" || exit

if [[ "${CONF_PODS}" == "yes" ]]; then

	echo "Building utarfs binary"
	pushd src/utarfs/ || exit
	make all
	popd || exit

	echo "Building kata-overlay binary"
	pushd src/overlay/ || exit
	make all
	popd || exit

	echo "Building tardev-snapshotter service binary"
	pushd src/tardev-snapshotter/ || exit
	make all
	popd || exit
fi

echo "Building runtime-go shim binary"
pushd src/runtime/ || exit
if [[ "${CONF_PODS}" == "yes" ]] || [[ "${OS_VERSION}" == "3.0" ]]; then
	make "${runtime_go_make_flags[@]}"
else
	# Mariner 2 pod sandboxing uses cgroupsv1 - note: cannot add the kernelparams in above assignments,
	# leads to quotation issue. Hence, implementing the conditional check right here at the time of the make command
	make "${runtime_go_make_flags[@]}" "KERNELPARAMS=systemd.legacy_systemd_cgroup_controller=yes systemd.unified_cgroup_hierarchy=0"
fi
popd || exit

echo "Building runtime-rs shim binary"
pushd src/runtime-rs/ || exit
make "${runtime_rs_make_flags[@]}"
popd || exit

echo "Building kata-ctl binary"
pushd src/tools/kata-ctl/ || exit
make "${runtime_rs_make_flags[@]}"
popd || exit

create_debug_shim_config() {
	local config_dir="$1"
	local release_cfg="$2"
	local debug_cfg="$3"

	pushd "${config_dir}" || exit
	echo "Creating shim debug configuration: ${debug_cfg}"
	cp "${release_cfg}" "${debug_cfg}"
	# Ensure debug is enabled in the shim config, regardless of whether the
	# template uses commented or uncommented keys.
	sed -i -E 's|^#?[[:space:]]*enable_debug[[:space:]]*=.*$|enable_debug = true|' "${debug_cfg}"
	sed -i -E 's|^#?[[:space:]]*debug_console_enabled[[:space:]]*=.*$|debug_console_enabled = true|' "${debug_cfg}"

	if [[ "${CONF_PODS}" == "yes" ]]; then
		echo "Adding debug igvm to SNP shim debug configuration"
		sed -i "s|${IGVM_FILE_NAME}|${IGVM_DBG_FILE_NAME}|g" "${debug_cfg}"
	fi
	popd || exit
}

create_debug_shim_config  "${CONFIG_DIR_RUNTIME_GO}" "${SHIM_CONFIG_FILE_NAME_RUNTIME_GO}" "${SHIM_DBG_CONFIG_FILE_NAME_RUNTIME_GO}"
create_debug_shim_config "${CONFIG_DIR_RUNTIME_RS}" "${SHIM_CONFIG_FILE_NAME_RUNTIME_RS}" "${SHIM_DBG_CONFIG_FILE_NAME_RUNTIME_RS}"

echo "Building agent binary and generating service files"
pushd src/agent/ || exit
make "${agent_make_flags[@]}"
make install "${agent_make_flags[@]}"
popd || exit

popd || exit
