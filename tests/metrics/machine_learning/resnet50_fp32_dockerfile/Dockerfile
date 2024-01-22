# Copyright (c) 2023 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

# Usage: FROM [image name]
FROM intel/intel-optimized-tensorflow:2.9.1

ENV DEBIAN_FRONTEND=noninteractive

# Version of the Dockerfile
LABEL DOCKERFILE_VERSION="1.0"

RUN apt-get update && \
	apt-get install -y --no-install-recommends wget nano curl build-essential git zlib1g-dev libncurses5-dev libgdbm-dev libnss3-dev libssl-dev libreadline-dev libffi-dev libsqlite3-dev libbz2-dev && \
	wget -q https://storage.googleapis.com/intel-optimized-tensorflow/models/v1_8/resnet50_fp32_pretrained_model.pb && \
	git clone https://github.com/IntelAI/models.git && \
	wget -q https://www.python.org/ftp/python/3.8.10/Python-3.8.10.tgz && \
	tar xzf Python-3.8.10.tgz && \
	cd Python-3.8.10 && \
	./configure --enable-optimizations && make altinstall && make install && \
	apt-get install -y python3-pip && \
	pip install --no-cache-dir intel-tensorflow-avx512==2.8.0 && \
	pip install --no-cache-dir protobuf==3.20.*

CMD ["/bin/bash"]
