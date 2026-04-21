#!/bin/bash
#
# Copyright (c) 2021 Easystack Inc.
#
# SPDX-License-Identifier: Apache-2.0

set -e

cidir=$(dirname "$0")
# shellcheck source=/dev/null
source "${cidir}/../tests/common.bash"

run_docs_url_alive_check
