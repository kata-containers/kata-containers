#!/bin/bash
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

set -o errexit
set -o nounset
set -o pipefail

source /etc/os-release

SNAP_CI="${SNAP_CI:-false}"

echo "Setup script for packaging"

export tests_repo="${tests_repo:-github.com/kata-containers/tests}"
export tests_repo_dir="$GOPATH/src/$tests_repo"

clone_tests_repo()
{
	# KATA_CI_NO_NETWORK is (has to be) ignored if there is
	# no existing clone.
	if [ -d "$tests_repo_dir" -a -n "${KATA_CI_NO_NETWORK:-}" ]
	then
		return
	fi

	go get -d -u "$tests_repo" || true
}

clone_tests_repo

if [ "$SNAP_CI" == "true" ] && [ "$ID" == "ubuntu" ]; then
	# Do not install kata since we want to build, install and test the snap
	export INSTALL_KATA="no"

	echo "Install snap dependencies"
	sudo apt-get --no-install-recommends install -y snapd snapcraft make

	echo "Building snap image"
	make snap

	echo "Install kata container snap"
	sudo snap install --dangerous --classic "$(basename kata-containers_*.snap)"

	etc_confile="/etc/kata-containers/configuration.toml"
	usr_confile="/usr/share/defaults/kata-containers/configuration.toml"
	snap_confile="/snap/kata-containers/current/usr/share/defaults/kata-containers/configuration.toml"
	snap_bin_dir="/snap/kata-containers/current/usr/bin"

	sudo rm -f /usr/local/bin/kata-runtime \
	   /usr/bin/kata-runtime \
	   /usr/local/bin/containerd-shim-kata-v2 \
	   /usr/bin/containerd-shim-kata-v2 \
	   "${etc_confile}" "${usr_confile}"

	sudo ln -sf ${snap_bin_dir}/kata-runtime /usr/bin/kata-runtime
	sudo ln -sf ${snap_bin_dir}/kata-runtime /usr/local/bin/kata-runtime

	sudo ln -sf ${snap_bin_dir}/containerd-shim-kata-v2 /usr/bin/containerd-shim-kata-v2
	sudo ln -sf ${snap_bin_dir}/containerd-shim-kata-v2 /usr/local/bin/containerd-shim-kata-v2

	# copy configuration file since some tests modify it.
	sudo mkdir -p "$(dirname "${etc_confile}")" "$(dirname "${usr_confile}")"
	sudo cp "${snap_confile}" "${etc_confile}"
	sudo cp "${snap_confile}" "${usr_confile}"

	# Use the same version of tests to test the snap
	git -C "${tests_repo_dir}" checkout "$(basename kata-containers_*.snap | cut -d_ -f2)"

	"${tests_repo_dir}/cmd/container-manager/manage_ctr_mgr.sh" docker configure -r kata-runtime -f
fi

pushd "${tests_repo_dir}"
.ci/setup.sh
popd
