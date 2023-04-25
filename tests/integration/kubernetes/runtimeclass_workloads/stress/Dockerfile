#
# Copyright (c) 2021 IBM Corp.
#
# SPDX-License-Identifier: Apache-2.0

# The image has only the 'latest' tag so it needs to ignore DL3007
#hadolint ignore=DL3007
FROM quay.io/libpod/ubuntu:latest
RUN apt-get -y update && \
    apt-get -y upgrade && \
    apt-get -y --no-install-recommends install stress && \
    apt-get clean && \
    rm -rf /var/lib/apt/lists/*
