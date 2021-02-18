#!/bin/bash
#
# Copyright (c) 2019 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

set -o errexit
set -o pipefail
set -o nounset

supported_artifacts=(
  "install_clh"
  "install_experimental_kernel"
  "install_firecracker"
  "install_image"
  "install_kata_components"
  "install_kernel"
  "install_qemu"
)

for c in ${supported_artifacts[@]}; do echo $c; done
