# Copyright (c) 2023 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

# Usage: FROM [image name]
FROM intel/intel-optimized-pytorch:1.12.100

# Version of the Dockerfile
LABEL DOCKERFILE_VERSION="1.0"

RUN apt-get update && \
	apt-get install -y --no-install-recommends build-essential curl git && \
	apt-get remove -y unattended-upgrades && \
	curl -OkL https://github.com/dionhaefner/pyhpc-benchmarks/archive/refs/tags/v3.0.tar.gz  && \
	tar -xf v3.0.tar.gz && \
	pip install --no-cache-dir click==8.1.3 && \
	cd pyhpc-benchmarks-3.0 && pip3 install --no-cache-dir --user torch==1.10.0

CMD ["/bin/bash"]
