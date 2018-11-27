#!/bin/bash
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

set -o errexit
set -o nounset
set -o pipefail

readonly runtime_path=$(which ${RUNTIME:-kata-runtime})

sudo mkdir -p /etc/containerd/
cat << EOT | sudo tee /etc/containerd/config.toml
[plugins]
    [plugins.cri.containerd]
          [plugins.cri.containerd.untrusted_workload_runtime]
	          runtime_type = "io.containerd.runtime.v1.linux"
		          runtime_engine = "${runtime_path}"
EOT
