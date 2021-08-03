#!/bin/bash
#
# Copyright (c) 2021 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

set -o errexit
set -o nounset
set -o pipefail
set -o errtrace

script_dir=$(dirname "$(readlink -f "$0")")
install_guide="docs/install/container-manager/containerd/containerd-install.md"
guide_local="${script_dir}/../../${install_guide}"
guide_url="https://raw.githubusercontent.com/kata-containers/kata-containers/main/${install_guide}"


if [ -f "$guide_local" ];then
	guide_md="$guide_local"
	echo "Using $guide_md"
else
	guide_md="containerd-install.md"
	echo "Not found ${guide_local}"
	echo "Using ${guide_url}"
	curl -o "${guide_md}" "${guide_url}"
fi

installer_script="containerd-installer-generated.sh"
kata_to_doc_url="https://raw.githubusercontent.com/kata-containers/tests/master/.ci/kata-doc-to-script.sh"
bash -c "$(curl -fsSL ${kata_to_doc_url}) ${guide_md} ${installer_script}"
cat "${installer_script}"
bash -x "${installer_script}"
