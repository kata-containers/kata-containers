# Copyright (c) 2024 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

# Set up an Ubuntu image with 'phoronix-test-suite' installed

FROM ubuntu:22.04

LABEL DOCKERFILE_VERSION="1.0"

ENV PHORONIX_VER="10.8.4"
ENV PHORONIX_URL="https://phoronix-test-suite.com/releases/"
ENV PHORONIX_TAR_FILE="phoronix-test-suite-${PHORONIX_VER}.tar.gz"
ENV PHORONIX_SRC="${PHORONIX_URL}/${PHORONIX_TAR_FILE}"
ENV PHORONIX_CFG_ALT="/etc/phoronix-test-suite.xml"
ENV PHORONIX_CFG="/usr/share/phoronix-test-suite/pts-core/static/user-config-defaults.xml"

ENV DEBIAN_FRONTEND noninteractive
ENV DEBCONF_NONINTERACTIVE_SEEN true

SHELL ["/bin/bash", "-o", "pipefail", "-c"]
RUN apt-get update && \
	apt-get install -y --no-install-recommends git curl build-essential autoconf && \
	apt-get install -y --no-install-recommends php libapache2-mod-php php-bz2 php-gd php-sqlite3 php-xml && \
	apt-get install -y --no-install-recommends cmake cmake-data mesa-utils vulkan-tools unzip apt-file && \
        curl -OkL ${PHORONIX_SRC} && \
	tar zxf ${PHORONIX_TAR_FILE} && cd phoronix-test-suite && ./install-sh && \
	apt-get clean && rm -rf /var/lib/apt/lists && \
	rm -f ${PHORONIX_TAR_FILE} && \
	if [ -n "$http_proxy" ] ; then \
	sed -i "s|<ProxyAddress></ProxyAddress>|<ProxyAddress>$(getent hosts "$(echo $http_proxy | sed 's~http[s]*://~~g' | sed -e 's/:[0-9]*//g')" | awk '{ print $1 }')</ProxyAddress>|g" ${PHORONIX_CFG} ; \
	sed -i "s|<ProxyPort></ProxyPort>|<ProxyPort>$(echo $http_proxy | sed 's/^.*://g')</ProxyPort>|g" ${PHORONIX_CFG} ; \
	fi && \
	sed -i "s|<UploadResults>TRUE</UploadResults>|<UploadResults>FALSE</UploadResults>|g" ${PHORONIX_CFG} && \
	sed -i "s|<PromptForTestIdentifier>TRUE</PromptForTestIdentifier>|<PromptForTestIdentifier>FALSE</PromptForTestIdentifier>|g" ${PHORONIX_CFG} && \
	sed -i "s|<PromptSaveName>TRUE</PromptSaveName>|<PromptSaveName>FALSE</PromptSaveName>|g" ${PHORONIX_CFG} && \
	sed -i "s|<PromptForTestDescription>TRUE</PromptForTestDescription>|<PromptForTestDescription>FALSE</PromptForTestDescription>|g" ${PHORONIX_CFG} && \
	sed -i "s|<Configured>FALSE</Configured>|<Configured>TRUE</Configured>|g" ${PHORONIX_CFG} && \
	sed -i "s|<Timeout>20</Timeout>|<Timeout>2</Timeout>|g" ${PHORONIX_CFG} && \
	sed -i "s|<SaveResults>FALSE</SaveResults>|<SaveResults>TRUE</SaveResults>|g" ${PHORONIX_CFG} && \
	phoronix-test-suite download-test-files openvino && \
	phoronix-test-suite install openvino && \
	sed -i 's/<NoNetworkCommunication>FALSE/<NoNetworkCommunication>TRUE/g' ${PHORONIX_CFG} && \
	sed -i 's/<Timeout>20/<Timeout>3/g' ${PHORONIX_CFG}

CMD ["/bin/bash"]
