# Copyright (c) 2023 IBM Corp.
#
# SPDX-License-Identifier: Apache-2.0
FROM ubuntu:22.04

RUN apt-get update && \
DEBIAN_FRONTEND=noninteractive apt-get install -y s390-tools

COPY zcrypttest /usr/local/bin/

CMD ["bash"]

