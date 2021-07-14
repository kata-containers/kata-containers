#!/usr/bin/env bash
#
# Copyright 2021 Red Hat Inc.
#
# SPDX-License-Identifier: Apache-2.0
#
set -e

STATUS=$(git status --porcelain)
if [[ -z $STATUS ]]; then
    echo "tree is clean"
else
    echo "tree is dirty, please commit all changes"
    echo ""
    echo "$STATUS"
    git diff
    exit 1
fi
