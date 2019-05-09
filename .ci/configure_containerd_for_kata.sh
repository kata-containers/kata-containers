#!/bin/bash
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

set -o errexit
set -o nounset
set -o pipefail

readonly kata_runtime_path=$(command -v kata-runtime)
readonly runc_path=$(command -v runc)

sudo mkdir -p /etc/containerd/

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
