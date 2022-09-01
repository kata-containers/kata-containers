# Copyright (c) 2021 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

FROM ubuntu:20.04
ENV DEBIAN_FRONTEND=noninteractive
ENV INSTALL_IN_GOPATH=false

COPY install_yq.sh /usr/bin/install_yq.sh

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
	    libelf-dev \
	    patch && \
    if [ "$(uname -m)" = "s390x" ]; then apt-get install -y --no-install-recommends libssl-dev; fi && \
    apt-get clean && rm -rf /var/lib/lists/ && \
    install_yq.sh
