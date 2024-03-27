# Copyright (c) 2023 Intel Corporatiion
#
# SPDX-License-Identifier: Apache-2.0

# We know that using latest is error prone, we're taking the risk here.
# hadolint ignore=DL3007
FROM alpine:latest

# We don't need a specific version of those packages
# hadolint ignore=DL3018
RUN apk add --no-cache curl openssh-server

# Download and install `cpuid`, which will be used to detect
# whether we're the container is running on a TEE guest
# hadolint ignore=DL3059
RUN /bin/sh -c \
    'ARCH=$(uname -m) && \
    [[ "${ARCH}" == "x86_64" ]] && \
    curl -LO https://github.com/klauspost/cpuid/releases/download/v2.2.7/cpuid-Linux_x86_64_2.2.7.tar.gz && \
    tar -xvzf cpuid-Linux_x86_64_2.2.7.tar.gz  -C /usr/bin && \
    rm -rf cpuid-Linux_x86_64_2.2.7.tar.gz && \
    rm -f /usr/bin/LICENSE' || true

# This is done just to avoid the following error starting sshd
# `sshd: no hostkeys available -- exiting.`
# hadolint ignore=DL3059
RUN ssh-keygen -t ed25519 -f /etc/ssh/ssh_host_ed25519_key -P ""

# A password needs to be set for login to work. An empty password is
# unproblematic as password-based login to root is not allowed.
# hadolint ignore=DL3059
RUN passwd -d root

# Generated with `ssh-keygen -t ed25519 -f unencrypted -P "" -C ""`
COPY ssh/unencrypted.pub /root/.ssh/authorized_keys

ENTRYPOINT ["/usr/sbin/sshd", "-D"]
