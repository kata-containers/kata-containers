# Copyright (c) 2023 Microsoft
#
# SPDX-License-Identifier: Apache-2.0

FROM ubuntu:20.04
ENV DEBIAN_FRONTEND=noninteractive

RUN apt-get update && \
    apt-get install -y --no-install-recommends \
        ca-certificates \
        curl \
        git \
        golang \
        make && \
    apt-get clean && rm -rf /var/lib/lists/
