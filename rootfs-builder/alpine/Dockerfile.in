#
# Copyright (c) 2018 HyperHQ Inc.
#
# SPDX-License-Identifier: Apache-2.0

From golang:@GO_VERSION@-alpine

RUN apk update && apk add \
    git \
    make \
    bash \
    gcc \
    musl-dev \
    linux-headers \
    apk-tools-static \
    libseccomp \
    libseccomp-dev \
    curl
