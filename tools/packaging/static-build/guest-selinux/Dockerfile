# Copyright (c) 2026 NVIDIA Corporation
#
# SPDX-License-Identifier: Apache-2.0

ARG IMAGE_REGISTRY=registry.fedoraproject.org
FROM ${IMAGE_REGISTRY}/fedora:43

RUN ([ -n "$http_proxy" ] && \
    sed -i '$ a proxy='$http_proxy /etc/dnf/dnf.conf ; true) && \
    dnf install -y \
        container-selinux \
        policycoreutils \
        selinux-policy-targeted \
        setools-console && \
    dnf clean all
