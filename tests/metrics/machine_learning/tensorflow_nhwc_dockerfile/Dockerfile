# Copyright (c) 2023 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

# Usage: FROM [image name]
FROM intel/intel-optimized-tensorflow:2.9.1

# Version of the Dockerfile
LABEL DOCKERFILE_VERSION="1.0"

ENV DEBIAN_FRONTEND=noninteractive

RUN apt-get update && \
	apt-get install -y --no-install-recommends build-essential git zlib1g-dev libncurses5-dev libgdbm-dev libnss3-dev libssl-dev libreadline-dev libffi-dev libsqlite3-dev wget libbz2-dev && \
	apt-get remove -y unattended-upgrades && \
	git clone https://github.com/tensorflow/benchmarks && \
	wget -q https://www.python.org/ftp/python/3.8.10/Python-3.8.10.tgz && \
	tar xzf Python-3.8.10.tgz && \
	cd Python-3.8.10 && \
	./configure --prefix=/usr --enable-optimizations && make install && \
	python -m pip install --no-cache-dir absl-py==1.4.0 && \
	python -m pip install --no-cache-dir tensorflow==2.15.*

CMD ["/bin/bash"]
