#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
ARG IMAGE_REGISTRY=registry.fedoraproject.org
FROM ${IMAGE_REGISTRY}/fedora:44

RUN ([ -n "$http_proxy" ] && \
    sed -i '$ a proxy='$http_proxy /etc/dnf/dnf.conf ; true) && \
    dnf install -y \
        e2fsprogs \
        erofs-utils \
        findutils \
        gcc \
        gdisk \
        parted \
        qemu-img \
        veritysetup \
        xfsprogs && \
    dnf clean all
