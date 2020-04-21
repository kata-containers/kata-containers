#/bin/bash
#
# Copyright (c) 2018 HyperHQ Inc.
#
# SPDX-License-Identifier: Apache-2.0
#
# This test will perform several tests to validate kata containers with
# factory enabled using shimv2 + containerd + cri 

SCRIPT_PATH=$(dirname "$(readlink -f "$0")")

source "${SCRIPT_PATH}/../../../metrics/lib/common.bash"
source /etc/os-release || source /usr/lib/os-release
extract_kata_env

if [[ "$ID" =~ ^opensuse.*$ ]] || [ "$ID" == sles ] || [ "$ID" == rhel ]; then
	issue="https://github.com/kata-containers/tests/issues/1251"
	echo "Skip shimv2 on $ID, see: $issue"
	exit
fi

if [ -z $INITRD_PATH ]; then
	echo "Skipping vm templating test as initrd is not set"
        exit 0
fi

echo "========================================"
echo "   start shimv2 with factory testing"
echo "========================================"

export SHIMV2_TEST=true
export FACTORY_TEST=true

${SCRIPT_PATH}/../cri/integration-tests.sh

