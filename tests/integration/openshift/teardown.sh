#!/bin/bash
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

set -e

SCRIPT_PATH=$(dirname "$(readlink -f "$0")")
source "${SCRIPT_PATH}/openshiftrc"
source "${SCRIPT_PATH}/../../lib/common.bash"

echo "Terminate Openshift master and node processes"
pgrep openshift | xargs sudo kill -9

echo "Stop cri-o service"
sudo systemctl stop crio

# Check no processes are left behind
check_processes
