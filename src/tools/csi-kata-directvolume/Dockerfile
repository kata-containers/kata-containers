# Copyright (c) 2024 Ant Group
#
# SPDX-License-Identifier: Apache-2.0

FROM alpine:3.19

LABEL maintainers="Kata Containers Authors"
LABEL description="Kata DirectVolume Driver"
ARG binary=./bin/directvolplugin

RUN apk add --no-cache util-linux=2.39.3-r0 coreutils=9.4-r2 e2fsprogs=1.47.0-r5 xfsprogs=6.5.0-r0 xfsprogs-extra=6.5.0-r0 btrfs-progs=6.6.2-r0 \
    && apk update \
    && apk upgrade
COPY ${binary} /kata-directvol-plugin
ENTRYPOINT ["/kata-directvol-plugin"]
