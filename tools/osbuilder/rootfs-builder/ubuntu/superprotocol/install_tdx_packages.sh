#!/bin/bash

curl -fsSL https://download.01.org/intel-sgx/sgx_repo/ubuntu/intel-sgx-deb.key | apt-key add -
echo 'deb [arch=amd64] https://download.01.org/intel-sgx/sgx_repo/ubuntu jammy main' >> /etc/apt/sources.list.d/intel-sgx.list
apt-get update

DEBIAN_FRONTEND=noninteractive apt install libtdx-attest libtdx-attest-dev --no-install-recommends -y
