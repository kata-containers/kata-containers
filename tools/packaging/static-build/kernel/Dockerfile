# Copyright (c) 2021 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

FROM ubuntu:22.04
ENV DEBIAN_FRONTEND=noninteractive

# kernel deps
RUN apt-get update && \
    apt-get install -y --no-install-recommends \
	    bc \
	    bison \
	    build-essential \
	    ca-certificates \
	    curl \
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
    apt-get clean && apt-get autoclean
