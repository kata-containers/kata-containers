# Copyright (c) 2022 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
from ubuntu:20.04

ARG DEBIAN_FRONTEND=noninteractive
ENV TZ=UTC
RUN apt-get update &&\
    apt-get --no-install-recommends install -y software-properties-common &&\
    add-apt-repository ppa:git-core/ppa -y &&\
    apt-get update && apt-get upgrade -y && \
    apt-get --no-install-recommends install -y \
	    apt-utils \
	    asciidoctor \
	    autoconf \
	    autopoint \
	    automake \
	    busybox-static \
	    ca-certificates \
	    curl \
	    gcc \
	    gettext \
	    git \
	    libaio-dev \
	    libblkid-dev \
	    libselinux1-dev \
	    libtool \
	    libpopt-dev \
	    libjson-c-dev \
	    libssl-dev \
	    make \
	    ninja-build \
	    pkg-config \
	    uuid-dev \
	    libseccomp-dev \
	    libseccomp2 \
	    zlib1g-dev &&\
    apt-get clean && rm -rf /var/lib/apt/lists/
