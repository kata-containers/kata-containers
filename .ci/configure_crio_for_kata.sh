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

# `use_runtime_class` should be set to:
# - true if we will test using k8s RuntimeClass feature or
# - false (default) if we will test using the old trusted/untrusted annotations.
use_runtime_class=${use_runtime_class:-false}

if [ "${use_runtime_class}"  == true ]; then
	echo "Configure runtimes map for RuntimeClass feature"
	echo "- Set runc as default runtime"
	sudo sed -i 's!runtime_path =.*!runtime_path = "/usr/local/bin/crio-runc"!' "$crio_config_file"
	echo "- Add kata-runtime to the runtimes map"
	sudo sed -i '/crio-runc/a[crio.runtime.runtimes.kata]' "$crio_config_file"
	sudo sed -i '/kata/aruntime_path = "/usr/local/bin/kata-runtime"' "$crio_config_file"
else
	echo "Configure runtimes for trusted/untrusted annotations"
	sudo sed -i 's!^#* *runtime =.*!runtime = "/usr/local/bin/crio-runc"!' "$crio_config_file"
	sudo sed -i 's!^default_runtime!# default_runtime!' "$crio_config_file"
	sudo sed -i 's!^#*runtime_untrusted_workload = ""!runtime_untrusted_workload = "/usr/local/bin/kata-runtime"!' "$crio_config_file"
	sudo sed -i 's!#*default_workload_trust = ""!default_workload_trust = "trusted"!' "$crio_config_file"
fi
