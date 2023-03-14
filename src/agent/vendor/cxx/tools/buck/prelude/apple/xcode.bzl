# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under both the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree and the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree.

load("@prelude//apple:apple_sdk.bzl", "get_apple_sdk_name")
load("@prelude//apple:apple_target_sdk_version.bzl", "get_min_deployment_version_for_node")
load("@prelude//apple:apple_utility.bzl", "has_apple_toolchain")
load(
    "@prelude//cxx:compile.bzl",
    "CxxSrcWithFlags",  # @unused Used as a type
)
load("@prelude//cxx:xcode.bzl", "cxx_populate_xcode_attributes")
load("@prelude//utils:utils.bzl", "expect")

def apple_populate_xcode_attributes(
        ctx,
        srcs: [CxxSrcWithFlags.type],
        argsfiles_by_ext: {str.type: "artifact"},
        product_name: str.type) -> {str.type: ""}:
    data = cxx_populate_xcode_attributes(ctx = ctx, srcs = srcs, argsfiles_by_ext = argsfiles_by_ext, product_name = product_name)

    if has_apple_toolchain(ctx):
        data["sdk"] = get_apple_sdk_name(ctx)
        data["deployment_version"] = get_min_deployment_version_for_node(ctx)

    if hasattr(ctx.attrs, "swift_version"):
        swift_version = ctx.attrs.swift_version
        if swift_version != None:
            data["swift_version"] = swift_version

    apple_xcode_data_add_xctoolchain(ctx, data)
    return data

def apple_xcode_data_add_xctoolchain(ctx: "context", data: {str.type: ""}):
    _add_label_for_attr(ctx, "_apple_xctoolchain_bundle_id", "xctoolchain_bundle_id_target", data)
    _add_output_for_attr(ctx, "_apple_xctoolchain_bundle_id", "xctoolchain_bundle_id", data)
    _add_label_for_attr(ctx, "_apple_xctoolchain", "xctoolchain_bundle_target", data)

def _add_label_for_attr(ctx: "context", attr_name: str.type, field_name: str.type, data: {str.type: ""}):
    xctoolchain_dep = _get_attribute_with_output(ctx, attr_name)
    if xctoolchain_dep:
        data[field_name] = xctoolchain_dep.label

def _add_output_for_attr(ctx: "context", attr_name: str.type, field_name: str.type, data: {str.type: ""}):
    xctoolchain_dep = _get_attribute_with_output(ctx, attr_name)
    if xctoolchain_dep:
        default_info = xctoolchain_dep[DefaultInfo]
        expect(len(default_info.default_outputs) == 1, "Expected only one output, got {}", len(default_info.default_outputs))
        data[field_name] = default_info.default_outputs[0]

def _get_attribute_with_output(ctx: "context", attr_name: str.type) -> ["dependency", None]:
    if hasattr(ctx.attrs, attr_name):
        dep = getattr(ctx.attrs, attr_name)
        default_info = dep[DefaultInfo]
        if len(default_info.default_outputs) > 0:
            # When there's no xctoolchain, there will be an empty `DefaultInfo`.
            # So, an empty `DefaultInfo` basically signifies that there's no xctoolchain.
            return dep
    return None
