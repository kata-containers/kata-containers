# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under both the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree and the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree.

load("@prelude//configurations:rules.bzl", _config_implemented_rules = "implemented_rules")

# Combine the attributes we generate, we the custom implementations we have.
load(":attributes.bzl", "attributes")
load(":rules_impl.bzl", "extra_attributes", "implemented_rules")

def _unimplemented(name, ctx):
    fail("Unimplemented rule type `{}` for target `{}`.".format(name, ctx.label))

def _unimplemented_impl(name):
    # We could use a lambda here, but then it means every single parse evaluates a lambda.
    # Lambda's have tricky semantics, so using partial lets us test Starlark prototypes with
    # some features disabled.
    return partial(_unimplemented, name)

def _mk_rule(name: str.type, attributes: {str.type: "attribute"}) -> "rule":
    # We want native code-containing rules to be marked incompatible with fat
    # platforms. Getting the ones that use cxx/apple toolchains is a little
    # overly broad as it includes things like python that don't themselves have
    # native code but need the toolchains if they depend on native code and in
    # that case incompatibility is transitive and they'll get it.
    fat_platform_compatible = True
    if name not in ("python_library", "python_binary", "python_test"):
        for toolchain_attr in ("_apple_toolchain", "_cxx_toolchain", "_go_toolchain"):
            if toolchain_attr in attributes:
                fat_platform_compatible = False

    # Fat platforms is an idea specific to our toolchains, so doesn't apply to
    # open source. Ideally this restriction would be done at the toolchain level.
    fat_platform_compatible = True # @oss-enable

    if not fat_platform_compatible:
        # copy so we don't try change the passed in object
        attributes = dict(attributes)

        attributes["_cxx_toolchain_target_configuration"] = attrs.dep(default = "fbcode//buck2/platform/execution:fat_platform_incompatible")

    return rule(
        impl = getattr(implemented_rules, name, _unimplemented_impl(name)),
        attrs = attributes,
        is_configuration_rule = name in _config_implemented_rules,
    )

def _merge_attributes() -> {str.type: {str.type: "attribute"}}:
    attrs = dict(attributes)
    for k in dir(extra_attributes):
        v = getattr(extra_attributes, k)
        if k in attrs:
            d = dict(attrs[k])
            d.update(v)
            attrs[k] = d
        else:
            attrs[k] = v
    return attrs

rules = {name: _mk_rule(name, attrs) for name, attrs in _merge_attributes().items()}

# The rules are accessed by doing module.name, so we have to put them on the correct module.
load_symbols(rules)
