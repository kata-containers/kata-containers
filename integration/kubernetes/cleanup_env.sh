#!/bin/bash
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

export KUBECONFIG=/etc/kubernetes/admin.conf
sudo -E kubeadm reset --cri-socket=/var/run/crio/crio.sock

sudo systemctl stop crio

sudo ip link set dev cni0 down
sudo ip link set dev flannel.1 down
sudo ip link del cni0
sudo ip link del flannel.1
