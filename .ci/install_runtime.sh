#!/bin/bash
#
# Copyright (c) 2017-2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

set -e

cidir=$(dirname "$0")

source "${cidir}/lib.sh"
source /etc/os-release || source /usr/lib/os-release

# Modify the runtimes build-time defaults

# enable verbose build
export V=1

# tell the runtime build to use sane defaults
export SYSTEM_BUILD_TYPE=kata

# The runtimes config file should live here
export SYSCONFDIR=/etc

# Artifacts (kernel + image) live below here
export SHAREDIR=/usr/share

USE_VSOCK="${USE_VSOCK:-no}"

runtime_config_path="${SYSCONFDIR}/kata-containers/configuration.toml"

PKGDEFAULTSDIR="${SHAREDIR}/defaults/kata-containers"
NEW_RUNTIME_CONFIG="${PKGDEFAULTSDIR}/configuration.toml"
# Note: This will also install the config file.
build_and_install "github.com/kata-containers/runtime"

# Check system supports running Kata Containers
kata_runtime_path=$(command -v kata-runtime)
sudo -E PATH=$PATH "$kata_runtime_path" kata-check

if [ -e "${NEW_RUNTIME_CONFIG}" ]; then
	# Remove the legacy config file
	sudo rm -f "${runtime_config_path}"

	# Use the new path
	runtime_config_path="${NEW_RUNTIME_CONFIG}"
fi

if [ -z "${METRICS_CI}" ]; then
	echo "Enabling all debug options in file ${runtime_config_path}"
	sudo sed -i -e 's/^#\(enable_debug\).*=.*$/\1 = true/g' "${runtime_config_path}"
	sudo sed -i -e 's/^kernel_params = "\(.*\)"/kernel_params = "\1 agent.log=debug"/g' "${runtime_config_path}"
else
	echo "Metrics run - do not enable all debug options in file ${runtime_config_path}"
fi

if [ x"${TEST_INITRD}" == x"yes" ]; then
	echo "Set to test initrd image"
	sudo sed -i -e '/^image =/d' ${runtime_config_path}
else
	echo "Set to test rootfs image"
	sudo sed -i -e '/^initrd =/d' ${runtime_config_path}
fi

if [ "$USE_VSOCK" == "yes" ]; then
	echo "Configure use of VSOCK in ${runtime_config_path}"
	sudo sed -i -e 's/^#use_vsock.*/use_vsock = true/' "${runtime_config_path}"

	vsock_module="vhost_vsock"
	echo "Check if ${vsock_module} is loaded"
	if lsmod | grep -q "$vsock_module" &> /dev/null ; then
		echo "Module ${vsock_module} is already loaded"
	else
		echo "Load ${vsock_module} module"
		sudo modprobe "${vsock_module}"
	fi
fi

echo "Add runtime as a new/default Docker runtime. Docker version \"$(docker --version)\" could change according to updates."
docker_options="-D --add-runtime kata-runtime=/usr/local/bin/kata-runtime"

echo "Add kata-runtime as a new/default Docker runtime."
"${cidir}/../cmd/container-manager/manage_ctr_mgr.sh" docker configure -r kata-runtime -f
