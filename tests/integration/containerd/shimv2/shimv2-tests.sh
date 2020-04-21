#/bin/bash
#
# Copyright (c) 2018 HyperHQ Inc.
#
# SPDX-License-Identifier: Apache-2.0
#
# This test will perform several tests to validate kata containers with
# shimv2 + containerd + cri

source /etc/os-release || source /usr/lib/os-release
SCRIPT_PATH=$(dirname "$(readlink -f "$0")")

if [ "$ID" == "centos" ] || [ "$ID" == sles ] || [[ "$ID" =~ ^opensuse.*$ ]]; then
	issue="https://github.com/kata-containers/tests/issues/1251"
	echo "Skip shimv2 on $ID, see: $issue"
	exit
fi

${SCRIPT_PATH}/../../../.ci/install_cri_containerd.sh

cni_bin_path="/opt/cni"

# Check if cni plugin binary is already installed, if so skip installation and 
# simply configure cni.
if [ -f "${cni_bin_path}/bridge" ]; then
	${SCRIPT_PATH}/../../../.ci/configure_cni.sh
else
	${SCRIPT_PATH}/../../../.ci/install_cni_plugins.sh
fi

export SHIMV2_TEST=true

echo "========================================"
echo "         start shimv2 testing"
echo "========================================"

${SCRIPT_PATH}/../cri/integration-tests.sh
