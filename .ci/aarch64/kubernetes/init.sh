#!/bin/bash
#
# Copyright (c) 2019 ARM Limited
#
# SPDX-License-Identifier: Apache-2.0

set -o errexit
set -o nounset
set -o pipefail

k8s_version=$(kubectl version | base64 | tr -d '\n')
network_plugin_config="https://cloud.weave.works/k8s/net?k8s-version=${k8s_version}"
