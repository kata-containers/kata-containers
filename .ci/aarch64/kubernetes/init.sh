#!/bin/bash
#
# Copyright (c) 2019 ARM Limited
#
# SPDX-License-Identifier: Apache-2.0

set -o errexit
set -o nounset
set -o pipefail

network_plugin_config_file="${SCRIPT_PATH}/../../.ci/${arch}/kubernetes/kube-flannel.yml"

curl -fsL $flannel_url -o $network_plugin_config_file

memory_resource="spec.template.spec.containers[*].resources.*.memory"
# install yq if not exist
${SCRIPT_PATH}/../../.ci/install_yq.sh
# original flannel has limitation and request for memory, it may cause OOM on AArch64
# so we delete related config on AArch64
sudo -E ${GOPATH}/bin/yq d -i -d5 $network_plugin_config_file $memory_resource  > /dev/null

network_plugin_config="$network_plugin_config_file"
