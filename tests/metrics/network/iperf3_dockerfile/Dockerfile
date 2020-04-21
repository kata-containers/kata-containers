# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

# Usage: FROM [image name]
FROM fedora

# Version of the Dockerfile
LABEL DOCKERFILE_VERSION="1.0"

# Install iperf3
RUN dnf -y update && \
    dnf -y install iperf3

CMD ["/bin/bash"]
