# Copyright (c) 2023 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

# Usage: FROM [image name]
FROM intel/intel-optimized-tensorflow:2.9.1

# Version of the Dockerfile
LABEL DOCKERFILE_VERSION="1.0"

ENV DEBIAN_FRONTEND=noninteractive

RUN apt-get update && \
	apt-get install -y --no-install-recommends build-essential git && \
	apt-get remove -y unattended-upgrades && \
	git clone https://github.com/tensorflow/benchmarks

CMD ["/bin/bash"]
