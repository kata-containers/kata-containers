# Copyright (c) 2026 Kata Contributors
#
# SPDX-License-Identifier: Apache-2.0

# Alpine-based image with kubectl for multi-arch support
# Used for kata-deploy verification jobs and other kubectl operations

ARG ALPINE_VERSION=3.23
FROM alpine:${ALPINE_VERSION}

# Install bash, curl, and download kubectl
# hadolint ignore=DL3018
RUN apk add --no-cache bash curl ca-certificates && \
    ARCH=$(uname -m) && \
    case "${ARCH}" in \
        x86_64)  KUBECTL_ARCH=amd64 ;; \
        aarch64) KUBECTL_ARCH=arm64 ;; \
        ppc64le) KUBECTL_ARCH=ppc64le ;; \
        s390x)   KUBECTL_ARCH=s390x ;; \
        *)       echo "Unsupported architecture: ${ARCH}" && exit 1 ;; \
    esac && \
    KUBECTL_VERSION=$(curl -L -s https://dl.k8s.io/release/stable.txt) && \
    curl -fL --progress-bar -o /usr/local/bin/kubectl \
        "https://dl.k8s.io/release/${KUBECTL_VERSION}/bin/linux/${KUBECTL_ARCH}/kubectl" && \
    chmod +x /usr/local/bin/kubectl && \
    kubectl version --client

# Default to bash shell
CMD ["/bin/bash"]
