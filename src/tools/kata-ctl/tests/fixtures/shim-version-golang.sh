#!/bin/sh
# Copyright (c) 2026 Kata Containers community
#
# SPDX-License-Identifier: Apache-2.0

if [ "$#" -ne 1 ] || [ "$1" != "--version" ]; then
    printf '%s\n' 'expected --version' >&2
    exit 2
fi

printf '%s\n' 'Kata Containers containerd shim (Golang): id: "io.containerd.kata.v2", version: 4.0.0, commit: abc123'
