# Copyright (c) 2026 Kata Contributors
#
# SPDX-License-Identifier: Apache-2.0

# Helm image based on kata-containers/kubectl for multi-arch support
# Used for kata-lifecycle-manager workflows and other helm operations

# hadolint ignore=DL3007
FROM quay.io/kata-containers/kubectl:latest

# Use bash with pipefail for safer pipe handling
SHELL ["/bin/bash", "-o", "pipefail", "-c"]

# Download and install helm
# hadolint ignore=DL3018
RUN ARCH=$(uname -m) && \
    case "${ARCH}" in \
        x86_64)  HELM_ARCH=amd64 ;; \
        aarch64) HELM_ARCH=arm64 ;; \
        ppc64le) HELM_ARCH=ppc64le ;; \
        s390x)   HELM_ARCH=s390x ;; \
        *)       echo "Unsupported architecture: ${ARCH}" && exit 1 ;; \
    esac && \
    HELM_VERSION=$(curl -s https://api.github.com/repos/helm/helm/releases/latest | grep '"tag_name"' | sed -E 's/.*"([^"]+)".*/\1/') && \
    curl -fL --progress-bar -o /tmp/helm.tar.gz \
        "https://get.helm.sh/helm-${HELM_VERSION}-linux-${HELM_ARCH}.tar.gz" && \
    tar -xzf /tmp/helm.tar.gz -C /tmp && \
    mv "/tmp/linux-${HELM_ARCH}/helm" /usr/local/bin/helm && \
    rm -rf /tmp/helm.tar.gz "/tmp/linux-${HELM_ARCH}" && \
    chmod +x /usr/local/bin/helm && \
    helm version --short

# Default to bash shell
CMD ["/bin/bash"]
