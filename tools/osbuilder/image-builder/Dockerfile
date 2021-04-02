#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
ARG IMAGE_REGISTRY=registry.fedoraproject.org
FROM ${IMAGE_REGISTRY}/fedora:latest

RUN [ -n "$http_proxy" ] && sed -i '$ a proxy='$http_proxy /etc/dnf/dnf.conf ; true

RUN dnf install -y qemu-img parted gdisk e2fsprogs gcc xfsprogs findutils
