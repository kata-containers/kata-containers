# Copyright (c) 2024 NVIDIA Corporation
#
# SPDX-License-Identifier: Apache-2.0

FROM alpine:3.20.0

# Install build dependencies and ORAS for cache support
RUN apk update && apk --no-cache add gpg gpg-agent coreutils bash curl make \
 	gcc g++ pkgconf libselinux-dev gpg-agent

# Install ORAS CLI for tarball caching
ARG ORAS_VERSION=1.3.0
RUN ARCH=$(uname -m) && \
    case "${ARCH}" in \
        x86_64) ORAS_ARCH="amd64" ;; \
        aarch64) ORAS_ARCH="arm64" ;; \
        ppc64le) ORAS_ARCH="ppc64le" ;; \
        s390x) ORAS_ARCH="s390x" ;; \
        *) echo "Unsupported architecture: ${ARCH}" && exit 1 ;; \
    esac && \
    curl -sSL "https://github.com/oras-project/oras/releases/download/v${ORAS_VERSION}/oras_${ORAS_VERSION}_linux_${ORAS_ARCH}.tar.gz" -o /tmp/oras.tar.gz && \
    tar -xzf /tmp/oras.tar.gz -C /usr/local/bin oras && \
    rm /tmp/oras.tar.gz && \
    oras version
