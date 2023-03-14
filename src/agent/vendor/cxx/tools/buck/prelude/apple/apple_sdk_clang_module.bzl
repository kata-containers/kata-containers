# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under both the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree and the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree.

load(":swift_pcm_compilation.bzl", "get_shared_pcm_compilation_args")
load(":swift_toolchain_types.bzl", "SdkUncompiledModuleInfo")

def apple_sdk_clang_module_impl(ctx: "context") -> ["provider"]:
    cmd = get_shared_pcm_compilation_args(ctx.attrs.target, ctx.attrs.module_name)
    module_dependency_infos = filter(None, [d.get(SdkUncompiledModuleInfo) for d in ctx.attrs.deps])
    return [
        DefaultInfo(),
        SdkUncompiledModuleInfo(
            name = ctx.attrs.name,
            module_name = ctx.attrs.module_name,
            is_framework = ctx.attrs.is_framework,
            is_swiftmodule = False,
            partial_cmd = cmd,
            input_relative_path = ctx.attrs.modulemap_relative_path,
            deps = module_dependency_infos,
        ),
    ]

# This rule represent a Clang module from SDK and forms a graph of dependencies between such modules.
apple_sdk_clang_module = rule(
    impl = apple_sdk_clang_module_impl,
    attrs = {
        "deps": attrs.list(attrs.dep(), default = []),
        "is_framework": attrs.bool(default = False),
        # This is a real module name, contrary to `name`
        # which has a special suffix to distinguish Swift and Clang modules with the same name
        "module_name": attrs.string(),
        "modulemap_relative_path": attrs.string(),
        "target": attrs.string(),
    },
)
