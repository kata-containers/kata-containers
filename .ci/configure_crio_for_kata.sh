#!/bin/bash
#
# Copyright (c) 2017-2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

set -o errexit
set -o nounset
set -o pipefail

crio_config_file="/etc/crio/crio.conf"

minor_crio_version=$(crio --version | head -1 | cut -d '.' -f2)

if [ "$minor_crio_version" -ge "12" ]; then
	echo "Configure runtimes map for RuntimeClass feature"
	echo "- Set runc as default runtime"
	sudo sed -i 's!runtime_path =.*!runtime_path = "/usr/local/bin/crio-runc"!' "$crio_config_file"
	sudo sed -i 's!runtime_type =.*!runtime_type = "oci"!' "$crio_config_file"
	echo "- Add kata-runtime to the runtimes map"
	sudo sed -i '/crio-runc/a[crio.runtime.runtimes.kata]' "$crio_config_file"
	sudo sed -i '/kata/aruntime_path = "/usr/local/bin/kata-runtime"' "$crio_config_file"
	sudo sed -i 's!runtime_root =.*!runtime_root = "/run/vc"!' "$crio_config_file"
	sudo sed -i '/crio-runc/a runtime_root = "/run/runc"' "$crio_config_file"
	sudo sed -i '/crio-runc/a runtime_type = "oci"' "$crio_config_file"
else
	echo "Configure runtimes for trusted/untrusted annotations"
	sudo sed -i 's!^#* *runtime =.*!runtime = "/usr/local/bin/crio-runc"!' "$crio_config_file"
	sudo sed -i 's!^default_runtime!# default_runtime!' "$crio_config_file"
	sudo sed -i 's!^#*runtime_untrusted_workload = ""!runtime_untrusted_workload = "/usr/local/bin/kata-runtime"!' "$crio_config_file"
	sudo sed -i 's!#*default_workload_trust = ""!default_workload_trust = "trusted"!' "$crio_config_file"
fi
