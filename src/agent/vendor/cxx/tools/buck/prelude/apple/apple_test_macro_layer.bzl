# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under both the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree and the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree.

load(":apple_bundle_config.bzl", "apple_bundle_config")
load(":apple_macro_layer.bzl", "apple_macro_layer_set_bool_override_attrs_from_config")
load(
    ":apple_rules_impl_utility.bzl",
    "APPLE_LINK_LIBRARIES_LOCALLY_OVERRIDE_ATTR_NAME",
)

_APPLE_TEST_LOCAL_EXECUTION_OVERRIDES = {
    APPLE_LINK_LIBRARIES_LOCALLY_OVERRIDE_ATTR_NAME: ("apple", "link_libraries_locally_override"),
}

def apple_test_macro_impl(apple_test_rule = None, **kwargs):
    kwargs.update(apple_bundle_config())
    kwargs.update(apple_macro_layer_set_bool_override_attrs_from_config(_APPLE_TEST_LOCAL_EXECUTION_OVERRIDES))
    apple_test_rule(
        **kwargs
    )
