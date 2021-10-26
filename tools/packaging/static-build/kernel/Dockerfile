# Copyright (c) 2021 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

FROM ubuntu
ENV DEBIAN_FRONTEND=noninteractive

# kernel deps
RUN apt update
RUN apt install -y \
	    bc \
	    bison \
	    build-essential \
	    curl \
	    flex \
	    git \
	    iptables \
	    libelf-dev

RUN [ "$(uname -m)" = "s390x" ] && apt-get install -y libssl-dev || true
