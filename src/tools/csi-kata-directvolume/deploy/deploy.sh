#!/usr/bin/env bash
#
# Copyright (c) 2023 Ant Group
#
# SPDX-License-Identifier: Apache-2.0
#

set -e
set -o pipefail

BASE_DIR=$(dirname "$0")

${BASE_DIR}/rbac-deploy.sh
${BASE_DIR}/directvol-deploy.sh
