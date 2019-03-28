#!/bin/bash
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

set -o errexit
set -o nounset
set -o pipefail

# `use_runtime_class` should be set to:
# - true if we will test using k8s RuntimeClass feature or
# - false (default) if we will test using the old trusted/untrusted annotations.
use_runtime_class=${use_runtime_class:-false}

readonly kata_runtime_path=$(command -v kata-runtime)
readonly runc_path=$(command -v runc)

sudo mkdir -p /etc/containerd/

if [ "${use_runtime_class}"  == true ]; then
	cat << EOT | sudo tee /etc/containerd/config.toml
[plugins]
  [plugins.cri]
    [plugins.cri.containerd]
      [plugins.cri.containerd.runtimes.runc]
        runtime_type = "io.containerd.runtime.v1.linux"
        [plugins.cri.containerd.runtimes.runc.options]
          Runtime = "runc"
          RuntimeRoot = "${runc_path}"
      [plugins.cri.containerd.runtimes.kata]
        runtime_type = "io.containerd.runtime.v1.linux"
        [plugins.cri.containerd.runtimes.kata.options]
          Runtime = "kata-runtime"
          RuntimeRoot = "${kata_runtime_path}"
EOT

else
	cat << EOT | sudo tee /etc/containerd/config.toml
[plugins]
  [plugins.cri.containerd]
    [plugins.cri.containerd.untrusted_workload_runtime]
      runtime_type = "io.containerd.runtime.v1.linux"
      runtime_engine = "${kata_runtime_path}"
EOT

fi
