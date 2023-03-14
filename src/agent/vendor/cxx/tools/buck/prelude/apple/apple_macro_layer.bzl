# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under both the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree and the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree.

load(
    ":apple_rules_impl_utility.bzl",
    "APPLE_ARCHIVE_OBJECTS_LOCALLY_OVERRIDE_ATTR_NAME",
    "APPLE_LINK_BINARIES_LOCALLY_OVERRIDE_ATTR_NAME",
    "APPLE_LINK_LIBRARIES_LOCALLY_OVERRIDE_ATTR_NAME",
)

_APPLE_LIBRARY_LOCAL_EXECUTION_OVERRIDES = {
    APPLE_LINK_LIBRARIES_LOCALLY_OVERRIDE_ATTR_NAME: ("apple", "link_libraries_locally_override"),
    APPLE_ARCHIVE_OBJECTS_LOCALLY_OVERRIDE_ATTR_NAME: ("apple", "archive_objects_locally_override"),
}

_APPLE_BINARY_LOCAL_EXECUTION_OVERRIDES = {
    APPLE_LINK_BINARIES_LOCALLY_OVERRIDE_ATTR_NAME: ("apple", "link_binaries_locally_override"),
}

def apple_macro_layer_set_bool_override_attrs_from_config(attrib_map: {str.type: (str.type, str.type)}) -> {str.type: "selector"}:
    attribs = {}
    for (attrib_name, (config_section, config_key)) in attrib_map.items():
        config_value = read_config(config_section, config_key, None)
        if config_value != None:
            config_truth_value = config_value.lower() == "true"
            attribs[attrib_name] = select({
                "DEFAULT": config_truth_value,
                # Do not set attribute value for host tools
                "ovr_config//platform/macos/constraints:execution-platform-transitioned": None,
            })
    return attribs

def apple_library_macro_impl(apple_library_rule = None, **kwargs):
    kwargs.update(apple_macro_layer_set_bool_override_attrs_from_config(_APPLE_LIBRARY_LOCAL_EXECUTION_OVERRIDES))
    apple_library_rule(**kwargs)

def apple_binary_macro_impl(apple_binary_rule = None, **kwargs):
    kwargs.update(apple_macro_layer_set_bool_override_attrs_from_config(_APPLE_BINARY_LOCAL_EXECUTION_OVERRIDES))
    apple_binary_rule(**kwargs)
