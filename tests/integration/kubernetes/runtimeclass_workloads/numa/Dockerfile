# Copyright (c) 2026 NVIDIA Corporation
#
# SPDX-License-Identifier: Apache-2.0
#
# Minimal image that reads guest NUMA topology from sysfs.
# Multi-arch: linux/amd64, linux/arm64
#
# Build & push:
#   docker buildx build --platform linux/amd64,linux/arm64 \
#       -t quay.io/kata-containers/numa:$(date +%Y-%m-%d) --push .

FROM alpine:3.23

COPY entrypoint.sh /entrypoint.sh
RUN chmod +x /entrypoint.sh

ENTRYPOINT ["/entrypoint.sh"]
