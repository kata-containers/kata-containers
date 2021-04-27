# Copyright (c) 2019 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
FROM microsoft/azure-cli:latest

LABEL com.github.actions.name="Test kata-deploy in an AKS cluster"
LABEL com.github.actions.description="Test kata-deploy in an AKS cluster"

# Default to latest validated AKS-engine version
ARG AKS_ENGINE_VER="v0.62.0"
ARG ARCH=amd64

ENV GITHUB_ACTION_NAME="Test kata-deploy in an AKS cluster"

# When run, we expect the caller (GitHub Action workflow) to provide the
# PKG_SHA environment variable
ENV PKG_SHA=HEAD

RUN curl -LO https://storage.googleapis.com/kubernetes-release/release/$(curl -s https://storage.googleapis.com/kubernetes-release/release/stable.txt)/bin/linux/${ARCH}/kubectl \
  && chmod +x ./kubectl \
  && mv ./kubectl /usr/local/bin/kubectl

RUN curl -LO https://github.com/Azure/aks-engine/releases/download/${AKS_ENGINE_VER}/aks-engine-${AKS_ENGINE_VER}-linux-${ARCH}.tar.gz \
  && tar xvf aks-engine-${AKS_ENGINE_VER}-linux-${ARCH}.tar.gz \
  && mv aks-engine-${AKS_ENGINE_VER}-linux-${ARCH}/aks-engine /usr/local/bin/aks-engine \
  && rm aks-engine-${AKS_ENGINE_VER}-linux-${ARCH}.tar.gz

COPY kubernetes-containerd.json /
COPY setup-aks.sh test-kata.sh entrypoint.sh /

ENTRYPOINT ["/entrypoint.sh"]
