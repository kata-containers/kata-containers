# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under both the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree and the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree.

load(":apple_sdk_modules_utility.bzl", "SDKDepTSet")
load(":apple_sdk_swift_module.bzl", "compile_sdk_swiftinterface")
load(":swift_pcm_compilation.bzl", "compile_swift_sdk_pcm")

# Starting from a root node, this helper function traverses a graph of uncompiled SDK modules
# to create a graph of compiled ones.
def create_sdk_modules_graph(
        ctx: "context",
        sdk_module_providers: {str.type: "SdkCompiledModuleInfo"},
        uncompiled_sdk_module_info: "SdkUncompiledModuleInfo",
        toolchain_context: struct.type):
    # If input_relative_path is None then this module represents a root node of SDK modules graph.
    # In such case, we need to handle only its deps.
    if uncompiled_sdk_module_info.input_relative_path == None:
        for uncompiled_dependency_info in uncompiled_sdk_module_info.deps:
            create_sdk_modules_graph(ctx, sdk_module_providers, uncompiled_dependency_info, toolchain_context)
        return

    # If provider is already created, return.
    if uncompiled_sdk_module_info.name in sdk_module_providers:
        return

    compiled_dependency_infos_tsets = []
    for uncompiled_dependency_info in uncompiled_sdk_module_info.deps:
        create_sdk_modules_graph(ctx, sdk_module_providers, uncompiled_dependency_info, toolchain_context)
        compiled_dependency_info = sdk_module_providers[uncompiled_dependency_info.name]
        sdk_dep_tset = ctx.actions.tset(
            SDKDepTSet,
            value = compiled_dependency_info,
            children = [compiled_dependency_info.deps],
        )
        compiled_dependency_infos_tsets.append(sdk_dep_tset)

    sdk_deps_set = ctx.actions.tset(SDKDepTSet, children = compiled_dependency_infos_tsets)
    if uncompiled_sdk_module_info.is_swiftmodule:
        compile_sdk_swiftinterface(ctx, toolchain_context, sdk_deps_set, uncompiled_sdk_module_info, sdk_module_providers)
    else:
        compile_swift_sdk_pcm(ctx, toolchain_context, sdk_deps_set, uncompiled_sdk_module_info, sdk_module_providers)
