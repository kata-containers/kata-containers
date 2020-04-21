# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

# Usage: FROM [image name]
FROM debian

# Version of the Dockerfile
LABEL DOCKERFILE_VERSION="1.0"

RUN apt-get update && \
    apt-get -y install autoconf git bc libacl1-dev libacl1 acl gcc make perl g++ perl-modules && \
    git clone https://github.com/pjd/pjdfstest.git && \
    cd pjdfstest && \
    autoreconf -ifs && \
    ./configure && \
    make

CMD ["/bin/bash"]
