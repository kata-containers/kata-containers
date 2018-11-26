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

echo "Configure runtimes for trusted/untrusted annotations"
sudo sed -i 's/^#* *runtime =.*/runtime = "\/usr\/local\/bin\/crio-runc"/' "$crio_config_file"
sudo sed -i 's/^default_runtime/# default_runtime/' "$crio_config_file"
sudo sed -i 's/^#*runtime_untrusted_workload = ""/runtime_untrusted_workload = "\/usr\/local\/bin\/kata-runtime"/' "$crio_config_file"
sudo sed -i 's/#*default_workload_trust = ""/default_workload_trust = "trusted"/' "$crio_config_file"


