#!/bin/bash
#
# Copyright (c) 2017-2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

set -o errexit
set -o nounset
set -o pipefail

source /etc/os-release || source /usr/lib/os-release
issue="https://github.com/cri-o/cri-o/issues/3130"

if [ "$ID" == "centos" ]; then
	echo "Skip CRI-O installation on $ID, see: $issue"
	exit
fi

crio_config_file="/etc/crio/crio.conf"
runc_flag="\/usr\/local\/bin\/crio-runc"
kata_flag="\/usr\/local\/bin\/kata-runtime"

minor_crio_version=$(crio --version | head -1 | cut -d '.' -f2)

if [ "$minor_crio_version" -ge "12" ]; then
	echo "Configure runtimes map for RuntimeClass feature"
	echo "- Set runc as default runtime"
	runc_configured=$(grep -q $runc_flag $crio_config_file; echo "$?")
	if [[ $runc_configured -ne 0 ]]; then
		sudo sed -i 's!runtime_path =.*!runtime_path = "/usr/local/bin/crio-runc"!' "$crio_config_file"
	fi
	echo "- Add kata-runtime to the runtimes map"
	kata_configured=$(grep -q $kata_flag $crio_config_file; echo "$?")
	if [[ $kata_configured -ne 0 ]]; then
		sudo sed -i '/\/run\/runc/a [crio.runtime.runtimes.kata]' "$crio_config_file"
		sudo sed -i '/crio\.runtime\.runtimes\.kata\]/a runtime_path = "/usr/local/bin/kata-runtime"' "$crio_config_file"
		sudo sed -i '/kata-runtime"/a runtime_root = "/run/vc"' "$crio_config_file"
		sudo sed -i '/\/run\/vc/a runtime_type = "oci"' "$crio_config_file"
	fi
else
	echo "Configure runtimes for trusted/untrusted annotations"
	sudo sed -i 's!^#* *runtime =.*!runtime = "/usr/local/bin/crio-runc"!' "$crio_config_file"
	sudo sed -i 's!^default_runtime!# default_runtime!' "$crio_config_file"
	sudo sed -i 's!^#*runtime_untrusted_workload = ""!runtime_untrusted_workload = "/usr/local/bin/kata-runtime"!' "$crio_config_file"
	sudo sed -i 's!#*default_workload_trust = ""!default_workload_trust = "trusted"!' "$crio_config_file"
fi
