# Copyright (c) 2023 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

FROM ubuntu:22.04

ARG GITHUB_ACTIONS=false

COPY apt-ci-tune.sh /tmp/apt-ci-tune.sh
RUN GITHUB_ACTIONS="${GITHUB_ACTIONS}" bash /tmp/apt-ci-tune.sh && rm -f /tmp/apt-ci-tune.sh

COPY debug.sh /usr/bin/debug.sh

RUN \
apt-get update && \
apt-get install -y --no-install-recommends tree && \
apt-get clean && \
rm -rf /var/lib/apt/lists/*

CMD ["/usr/bin/debug.sh"]
