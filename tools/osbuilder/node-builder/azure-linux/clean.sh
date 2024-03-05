#!/usr/bin/env bash
#
# Copyright (c) 2024 Microsoft Corporation
#
# SPDX-License-Identifier: Apache-2.0

set -o errexit
set -o pipefail
set -o errtrace

[[ -n "${DEBUG:-}" ]] && set -x

script_dir="$(dirname "$(readlink -f "$0")")"
repo_dir="${script_dir}/../../../../"

common_file="${script_dir}/common.sh"
# shellcheck source=tools/osbuilder/node-builder/azure-linux/common.sh
source "${common_file}"

pushd "${repo_dir}" || exit

echo "Clean debug shim config"
pushd src/runtime/config/ || exit
rm -f "${SHIM_DBG_CONFIG_FILE_NAME}"
popd || exit

echo "Clean runtime build"
pushd src/runtime/ || exit
make clean SKIP_GO_VERSION_CHECK=1
popd || exit

echo "Clean agent build"
pushd src/agent/ || exit
make clean
popd || exit

rm -rf "${AGENT_INSTALL_DIR}"

echo "Clean UVM build"
pushd tools/osbuilder/ || exit
sudo -E PATH="${PATH}" make DISTRO=cbl-mariner clean
popd || exit

echo "Clean IGVM tool installation"


if [[ "${CONF_PODS}" == "yes" ]]; then

	echo "Clean tardev-snapshotter tarfs driver build"
	pushd src/tarfs || exit
	set_uvm_kernel_vars
	if [[ -n "${UVM_KERNEL_HEADER_DIR}" ]]; then
		make clean KDIR="${UVM_KERNEL_HEADER_DIR}"
	fi
	popd || exit

	echo "Clean utarfs binary build"
	pushd src/utarfs/ || exit
	make clean
	popd || exit

	echo "Clean tardev-snapshotter overlay binary build"
	pushd src/overlay/ || exit
	make clean
	popd || exit

	echo "Clean tardev-snapshotter service build"
	pushd src/tardev-snapshotter/ || exit
	make clean
	popd || exit
fi

popd || exit
