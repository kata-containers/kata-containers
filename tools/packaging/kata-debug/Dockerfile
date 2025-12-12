# Copyright (c) 2023 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

FROM quay.io/kata-containers/ubuntu:22.04

COPY debug.sh /usr/bin/debug.sh

RUN \
apt-get update && \
apt-get install -y --no-install-recommends tree && \
apt-get clean && \
rm -rf /var/lib/apt/lists/

CMD ["/usr/bin/debug.sh"]
