# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under both the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree and the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree.

# Utilities for checking and ignoring types

# Ignores the type, always returning "" (the wildcard type).
# Used where the type is true, but performance concerns preclude the type in normal operation.
#
# FIXME: Probably have a way to unconditionally enable such types, to ensure they remain accurate.
def unchecked(_):
    return ""

# Assert that a given value has a specific type, and return that value.
# Fails at runtime if the value does not have the right type.
def cast(value, type):
    def inner(_: type):
        pass

    inner(value)
    return value
