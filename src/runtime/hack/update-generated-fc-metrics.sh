# Copyright (c) 2020 Ant Financial
#
# SPDX-License-Identifier: Apache-2.0
#

set -e
BASEDIR=$(dirname "$0")

if [ "$#" -ne 1 ]; then
    echo "Usage:  ${0} <metrics.rs_URL>, for example: ${0} https://github.com/firecracker-microvm/firecracker/blob/master/src/logger/src/metrics.rs#L255-L688"
    exit
fi

python ${BASEDIR}/fc_metrics.py $1 > virtcontainers/fc_metrics.go
go fmt virtcontainers/fc_metrics.go
echo "ok"
