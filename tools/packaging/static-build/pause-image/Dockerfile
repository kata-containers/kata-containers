# Copyright (c) 2024 Intel
#
# SPDX-License-Identifier: Apache-2.0
FROM ubuntu:22.04

ENV DEBIAN_FRONTEND=noninteractive

ARG GITHUB_ACTIONS=false

SHELL ["/bin/bash", "-o", "pipefail", "-c"]

COPY apt-ci-tune.sh /tmp/apt-ci-tune.sh
RUN GITHUB_ACTIONS="${GITHUB_ACTIONS}" bash /tmp/apt-ci-tune.sh && rm -f /tmp/apt-ci-tune.sh

RUN apt-get update && \
    apt-get --no-install-recommends -y install \
    ca-certificates \
    curl \
    umoci \
    skopeo
