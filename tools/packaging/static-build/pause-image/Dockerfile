# Copyright (c) 2024 Intel
#
# SPDX-License-Identifier: Apache-2.0
ARG IMAGE_REGISTRY=docker.io
FROM ${IMAGE_REGISTRY}/ubuntu:22.04

ENV DEBIAN_FRONTEND=noninteractive

SHELL ["/bin/bash", "-o", "pipefail", "-c"]
RUN apt-get update && \
    apt-get --no-install-recommends -y install \
    ca-certificates \
    curl \
    umoci \
    skopeo
