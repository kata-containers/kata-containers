# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under both the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree and the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree.

def _maybe_get_bool(config: str.type, default: [None, bool.type]) -> [None, bool.type]:
    result = read_config("apple", config, None)
    if result == None:
        return default
    return result.lower() == "true"

def apple_bundle_config() -> {str.type: ""}:
    return {
        "_codesign_type": read_config("apple", "codesign_type_override", None),
        "_compile_resources_locally_override": _maybe_get_bool("compile_resources_locally_override", None),
        "_incremental_bundling_enabled": _maybe_get_bool("incremental_bundling_enabled", True),
        "_profile_bundling_enabled": _maybe_get_bool("profile_bundling_enabled", False),
    }
