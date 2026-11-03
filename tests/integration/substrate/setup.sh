#!/usr/bin/env bash
# Copyright (c) 2026 NVIDIA Corporation
# SPDX-License-Identifier: Apache-2.0

set -euo pipefail

sudo apt-get update
sudo apt-get install -y iproute2 iptables jq kmod nfs-common util-linux zstd
