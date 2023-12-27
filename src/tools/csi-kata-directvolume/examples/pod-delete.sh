#!/bin/bash
#
# Copyright (c) 2023 Ant Group
#
# SPDX-License-Identifier: Apache-2.0
#

set -e
set -o pipefail

BASE_DIR=$(dirname "$0")/pod-with-directvol

kubectl delete -f ${BASE_DIR}/csi-app.yaml 
kubectl delete -f ${BASE_DIR}/csi-pvc.yaml 
kubectl delete -f ${BASE_DIR}/csi-storageclass.yaml
