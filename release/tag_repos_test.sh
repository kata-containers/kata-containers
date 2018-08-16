#!/bin/bash
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

set -o errexit
set -o nounset
set -o pipefail

echo "Check tag_repos.sh show help"
./release/tag_repos.sh | grep Usage

echo "Check tag_repos.sh -h option"
./release/tag_repos.sh -h | grep Usage

echo "Check tag_repos.sh status"
./release/tag_repos.sh status | grep runtime

echo "Check tag_repos.sh create tags but not push"
./release/tag_repos.sh tag | grep "tags not pushed"
