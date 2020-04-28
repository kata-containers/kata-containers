#!/usr/bin/env bash
#
# Copyright 2020 Red Hat Inc.
#
# SPDX-License-Identifier: Apache-2.0
#
pkg-config libselinux 2> /dev/null && echo selinux
