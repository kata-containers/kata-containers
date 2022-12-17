#!/bin/sh
# Copyright (c) 2018 Intel Corporation, 2021 IBM Corp.
#
# SPDX-License-Identifier: Apache-2.0

OS_NAME=centos
OS_VERSION=${OS_VERSION:-stream9}
PACKAGES="chrony iptables"
[ "$AGENT_INIT" = no ] && PACKAGES+=" systemd"
[ "$SECCOMP" = yes ] && PACKAGES+=" libseccomp"
[ "$SELINUX" = yes ] && PACKAGES+=" container-selinux"

# Container registry tag is different from metalink repo, e.g. "stream9" => "9-stream"
os_repo_version="$(sed -E "s/(stream)(.+)/\2-\1/" <<< "$OS_VERSION")"

METALINK="https://mirrors.centos.org/metalink?repo=centos-baseos-$os_repo_version&arch=\$basearch"
if [ "$SELINUX" == yes ]; then
    # AppStream repository is required for the container-selinux package
    METALINK_APPSTREAM="https://mirrors.centos.org/metalink?repo=centos-appstream-$os_repo_version&arch=\$basearch"
fi
GPG_KEY_FILE=RPM-GPG-KEY-CentOS-Official
GPG_KEY_URL="https://centos.org/keys/$GPG_KEY_FILE"
