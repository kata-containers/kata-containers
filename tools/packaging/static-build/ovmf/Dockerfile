# Copyright (c) 2022 IBM
#
# SPDX-License-Identifier: Apache-2.0

FROM ubuntu:22.04
ENV DEBIAN_FRONTEND=noninteractive

RUN apt-get update && \
    apt-get install -y --no-install-recommends \
        ca-certificates \
        curl \
        g++ \
        gcc \
        git \
        iasl  \
        make \
        nasm \
        python3 \
        python3-distutils \
        python-is-python3 \
        uuid-dev && \
    apt-get clean && rm -rf /var/lib/lists/
