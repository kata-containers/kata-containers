#!/bin/bash
#
# Copyright (c) 2019 ARM Limited
#
# SPDX-License-Identifier: Apache-2.0

set -o errexit
set -o nounset
set -o pipefail

SCRIPT_PATH=$(dirname "$(readlink -f "$0")")
iptables_cache="${SCRIPT_PATH}/iptables_cache"

# The kubeadm reset process does not reset or clean up iptables rules
# you must do it manually
# Here, we restore the iptables based on the previously cached file.
sudo iptables-restore < "$iptables_cache"
sudo rm -rf "$iptables_cache"

# The kubeadm reset process does not clean your kubeconfig files.
# you must remove them manually.
sudo -E rm -rf "$HOME/.kube"

# Remove existing CNI configurations and binaries.
sudo rm -rf /var/lib/cni/networks/*
sudo rm -rf /opt/cni/bin/*
