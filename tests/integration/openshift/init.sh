#!/bin/bash
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

set -e

SCRIPT_PATH=$(dirname "$(readlink -f "$0")")
source "${SCRIPT_PATH}/openshiftrc"
source "${SCRIPT_PATH}/../../.ci/lib.sh"
source "${SCRIPT_PATH}/../../lib/common.bash"

# Check no processes are left behind
check_processes

echo "Start crio service"
sudo systemctl start crio

echo "Create configuration files"
openshift start --write-config="$openshift_config_path"

cp "$node_config" "$node_crio_config"

cat << EOF >> "$node_crio_config"
kubeletArguments:
  node-labels:
  - region=infra
  image-service-endpoint:
  - "unix:///var/run/crio/crio.sock"
  container-runtime-endpoint:
  - "unix:///var/run/crio/crio.sock"
  container-runtime:
  - "remote"
  runtime-request-timeout:
  - "15m"
  cgroup-driver:
  - "cgroupfs"
EOF

echo "Start Master"
sudo -E openshift start master --config "$master_config" &> master.log &

# Wait for the master to get ready.
wait_time=10
sleep_time=1
cmd="sudo -E oc status"
waitForProcess "$wait_time" "$sleep_time" "$cmd"

echo "Start Node"
sudo -E openshift start node --config "$node_crio_config" &> node.log &

sudo -E oc get all
echo "Openshift started successfully"
