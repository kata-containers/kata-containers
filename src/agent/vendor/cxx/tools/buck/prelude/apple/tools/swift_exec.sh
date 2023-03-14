#!/bin/bash
# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under both the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree and the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree.

# - Apply a debug prefix map for the current directory
# to make debug info relocatable.
# - Use $TMPDIR for the module cache location. This
# will be set to a unique location for each RE action
# which will avoid sharing modules across RE actions.
# This is necessary as the inputs to the modules will
# be transient and can be removed at any point, causing
# module validation errors to fail builds.
if [ -n "$INSIDE_RE_WORKER" ]; then
    MODULE_CACHE_PATH="$TMPDIR/module-cache"
else
    # When building locally we can use a shared module
    # cache as the inputs should remain at a fixed
    # location.
    MODULE_CACHE_PATH="/tmp/buck-module-cache"
fi

exec "$@" -debug-prefix-map "$PWD"=. -module-cache-path "$MODULE_CACHE_PATH"
