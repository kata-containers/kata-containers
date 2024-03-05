#!/usr/bin/env bash
#
# Copyright (c) 2024 Microsoft Corporation
#
# SPDX-License-Identifier: Apache-2.0

set -o errexit
set -o pipefail
set -o errtrace

[[ -n "${DEBUG:-}" ]] && set -x

CONF_PODS=${CONF_PODS:-no}
PREFIX=${PREFIX:-}

script_dir="$(dirname "$(readlink -f "$0")")"
repo_dir="${script_dir}/../../../../"

common_file="${script_dir}/common.sh"
# shellcheck source=tools/osbuilder/node-builder/azure-linux/common.sh
source "${common_file}"

pushd "${repo_dir}" || exit

echo "Creating target directories"
mkdir -p "${PREFIX}/${UVM_TOOLS_PATH_OSB}/scripts"
mkdir -p "${PREFIX}/${UVM_TOOLS_PATH_OSB}/rootfs-builder/cbl-mariner"
mkdir -p "${PREFIX}/${UVM_TOOLS_PATH_OSB}/image-builder"
mkdir -p "${PREFIX}/${UVM_TOOLS_PATH_OSB}/node-builder/azure-linux/agent-install/usr/bin"
mkdir -p "${PREFIX}/${UVM_TOOLS_PATH_OSB}/node-builder/azure-linux/agent-install/usr/lib/systemd/system"

if [[ "${CONF_PODS}" == "yes" ]]; then
	mkdir -p "${PREFIX}/${UVM_TOOLS_PATH_SRC}/kata-opa"
	mkdir -p "${PREFIX}/${UVM_TOOLS_PATH_SRC}/tarfs"
	mkdir -p "${PREFIX}/${UVM_TOOLS_PATH_OSB}/igvm-builder/azure-linux"
fi

echo "Installing UVM build scripting"
cp -a --backup=numbered VERSION "${PREFIX}/${UVM_TOOLS_PATH_OSB}/"
cp -a --backup=numbered tools/osbuilder/Makefile "${PREFIX}/${UVM_TOOLS_PATH_OSB}/"
cp -a --backup=numbered tools/osbuilder/scripts/lib.sh "${PREFIX}/${UVM_TOOLS_PATH_OSB}/scripts/"
cp -a --backup=numbered tools/osbuilder/rootfs-builder/rootfs.sh "${PREFIX}/${UVM_TOOLS_PATH_OSB}/rootfs-builder/"
cp -a --backup=numbered tools/osbuilder/rootfs-builder/cbl-mariner/config.sh "${PREFIX}/${UVM_TOOLS_PATH_OSB}/rootfs-builder/cbl-mariner/"
cp -a --backup=numbered tools/osbuilder/rootfs-builder/cbl-mariner/rootfs_lib.sh "${PREFIX}/${UVM_TOOLS_PATH_OSB}/rootfs-builder/cbl-mariner/"
cp -a --backup=numbered tools/osbuilder/image-builder/image_builder.sh "${PREFIX}/${UVM_TOOLS_PATH_OSB}/image-builder/"
cp -a --backup=numbered tools/osbuilder/image-builder/nsdax.gpl.c "${PREFIX}/${UVM_TOOLS_PATH_OSB}/image-builder/"
cp -a --backup=numbered tools/osbuilder/node-builder/azure-linux/Makefile "${PREFIX}/${UVM_TOOLS_PATH_OSB}/node-builder/azure-linux/"
cp -a --backup=numbered tools/osbuilder/node-builder/azure-linux/clean.sh "${PREFIX}/${UVM_TOOLS_PATH_OSB}/node-builder/azure-linux/"
cp -a --backup=numbered tools/osbuilder/node-builder/azure-linux/common.sh "${PREFIX}/${UVM_TOOLS_PATH_OSB}/node-builder/azure-linux/"
cp -a --backup=numbered tools/osbuilder/node-builder/azure-linux/uvm_build.sh "${PREFIX}/${UVM_TOOLS_PATH_OSB}/node-builder/azure-linux/"
cp -a --backup=numbered tools/osbuilder/node-builder/azure-linux/uvm_install.sh "${PREFIX}/${UVM_TOOLS_PATH_OSB}/node-builder/azure-linux/"

echo "Installing agent binary and service files"
cp -a --backup=numbered tools/osbuilder/node-builder/azure-linux/agent-install/usr/bin/kata-agent "${PREFIX}/${UVM_TOOLS_PATH_OSB}/node-builder/azure-linux/agent-install/usr/bin/"
cp -a --backup=numbered tools/osbuilder/node-builder/azure-linux/agent-install/usr/lib/systemd/system/kata-containers.target "${PREFIX}/${UVM_TOOLS_PATH_OSB}/node-builder/azure-linux/agent-install/usr/lib/systemd/system/"
cp -a --backup=numbered tools/osbuilder/node-builder/azure-linux/agent-install/usr/lib/systemd/system/kata-agent.service "${PREFIX}/${UVM_TOOLS_PATH_OSB}/node-builder/azure-linux/agent-install/usr/lib/systemd/system/"

if [[ "${CONF_PODS}" == "yes" ]]; then
	cp -a --backup=numbered src/kata-opa/allow-all.rego "${PREFIX}/${UVM_TOOLS_PATH_SRC}/kata-opa/"
	cp -a --backup=numbered src/kata-opa/allow-set-policy.rego "${PREFIX}/${UVM_TOOLS_PATH_SRC}/kata-opa/"
	cp -a --backup=numbered src/tarfs/Makefile "${PREFIX}/${UVM_TOOLS_PATH_SRC}/tarfs/"
	cp -a --backup=numbered src/tarfs/tarfs.c "${PREFIX}/${UVM_TOOLS_PATH_SRC}/tarfs/"
	cp -a --backup=numbered tools/osbuilder/igvm-builder/igvm_builder.sh "${PREFIX}/${UVM_TOOLS_PATH_OSB}/igvm-builder/"
	cp -a --backup=numbered tools/osbuilder/igvm-builder/azure-linux/config.sh "${PREFIX}/${UVM_TOOLS_PATH_OSB}/igvm-builder/azure-linux/"
	cp -a --backup=numbered tools/osbuilder/igvm-builder/azure-linux/igvm_lib.sh "${PREFIX}/${UVM_TOOLS_PATH_OSB}/igvm-builder/azure-linux/"
fi

popd || exit
