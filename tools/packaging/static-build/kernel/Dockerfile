# Copyright (c) 2021 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

FROM ubuntu:22.04
ENV DEBIAN_FRONTEND=noninteractive

ARG ARCH

# kernel deps
RUN apt-get update && \
    apt-get install -y --no-install-recommends \
	    bc \
	    bison \
	    build-essential \
	    ca-certificates \
	    curl \
	    debhelper \
	    flex \
	    git \
	    iptables \
	    kmod \
	    libelf-dev \
	    libssl-dev \
	    gettext \
	    rsync \
	    cpio \
	    patch && \
    if [ "${ARCH}" != "$(uname -m)" ]; then apt-get install --no-install-recommends -y gcc-"${ARCH}"-linux-gnu binutils-"${ARCH}"-linux-gnu; fi && \
    apt-get clean && apt-get autoclean && rm -rf /var/lib/apt/lists/*
