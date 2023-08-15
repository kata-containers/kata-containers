# Copyright (c) 2023 Microsoft Corporation
#
# SPDX-License-Identifier: Apache-2.0

OS_NAME=cbl-mariner
OS_VERSION=${OS_VERSION:-2.0}
LIBC="gnu"
PACKAGES="core-packages-base-image ca-certificates"
[ "$AGENT_INIT" = no ] && PACKAGES+=" systemd"
[ "$SECCOMP" = yes ] && PACKAGES+=" libseccomp"
[ "$AGENT_POLICY" = yes ] && PACKAGES+=" opa" || true
