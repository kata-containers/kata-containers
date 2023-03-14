# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under both the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree and the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree.

load(":apple_utility.bzl", "expand_relative_prefixed_sdk_path", "get_disable_pch_validation_flags")
load(":swift_module_map.bzl", "write_swift_module_map")
load(":swift_toolchain_types.bzl", "SdkCompiledModuleInfo", "SdkUncompiledModuleInfo")

def compile_sdk_swiftinterface(
        ctx: "context",
        toolchain_context: struct.type,
        sdk_deps_set: "SDKDepTSet",
        uncompiled_sdk_module_info: "SdkUncompiledModuleInfo",
        sdk_module_providers: {str.type: "SdkCompiledModuleInfo"}):
    uncompiled_module_info_name = uncompiled_sdk_module_info.module_name

    cmd = cmd_args(toolchain_context.compiler)
    cmd.add(uncompiled_sdk_module_info.partial_cmd)
    cmd.add(["-sdk", toolchain_context.sdk_path])
    cmd.add(toolchain_context.compiler_flags)

    if toolchain_context.swift_resource_dir:
        cmd.add([
            "-resource-dir",
            toolchain_context.swift_resource_dir,
        ])

    swift_module_map_artifact = write_swift_module_map(ctx, uncompiled_module_info_name, list(sdk_deps_set.traverse()))
    cmd.add([
        "-explicit-swift-module-map-file",
        swift_module_map_artifact,
    ])

    # sdk_swiftinterface_compile should explicitly depend on its deps that go to swift_modulemap
    cmd.hidden(sdk_deps_set.project_as_args("hidden"))
    cmd.add(sdk_deps_set.project_as_args("clang_deps"))

    swiftmodule_output = ctx.actions.declare_output(uncompiled_module_info_name + ".swiftmodule")
    expanded_swiftinterface_cmd = expand_relative_prefixed_sdk_path(
        cmd_args(toolchain_context.sdk_path),
        cmd_args(toolchain_context.swift_resource_dir),
        uncompiled_sdk_module_info.input_relative_path,
    )
    cmd.add([
        "-o",
        swiftmodule_output.as_output(),
        expanded_swiftinterface_cmd,
    ])

    sdk_module_providers[uncompiled_sdk_module_info.name] = SdkCompiledModuleInfo(
        name = uncompiled_sdk_module_info.name,
        module_name = uncompiled_module_info_name,
        is_framework = uncompiled_sdk_module_info.is_framework,
        is_swiftmodule = True,
        output_artifact = swiftmodule_output,
        deps = sdk_deps_set,
        input_relative_path = expanded_swiftinterface_cmd,
    )

    ctx.actions.run(cmd, category = "sdk_swiftinterface_compile", identifier = uncompiled_module_info_name)

def apple_sdk_swift_module_impl(ctx: "context") -> ["provider"]:
    module_name = ctx.attrs.module_name

    cmd = cmd_args([
        "-frontend",
        "-compile-module-from-interface",
        "-disable-implicit-swift-modules",
        "-serialize-parseable-module-interface-dependency-hashes",
        "-disable-modules-validate-system-headers",
        "-suppress-warnings",
        "-module-name",
        module_name,
        "-target",
        ctx.attrs.target,
        "-Xcc",
        "-fno-implicit-modules",
        "-Xcc",
        "-fno-implicit-module-maps",
    ])
    cmd.add(get_disable_pch_validation_flags())

    if module_name == "Swift" or module_name == "SwiftOnoneSupport":
        cmd.add([
            "-parse-stdlib",
        ])

    module_dependency_infos = filter(None, [d.get(SdkUncompiledModuleInfo) for d in ctx.attrs.deps])
    return [
        DefaultInfo(),
        SdkUncompiledModuleInfo(
            name = ctx.attrs.name,
            module_name = ctx.attrs.module_name,
            is_framework = ctx.attrs.is_framework,
            is_swiftmodule = True,
            partial_cmd = cmd,
            input_relative_path = ctx.attrs.swiftinterface_relative_path,
            deps = module_dependency_infos,
        ),
    ]

# This rule represent a Swift module from SDK and forms a graph of dependencies between such modules.
apple_sdk_swift_module = rule(
    impl = apple_sdk_swift_module_impl,
    attrs = {
        "deps": attrs.list(attrs.dep(), default = []),
        "is_framework": attrs.bool(default = False),
        # This is a real module name, contrary to `name`
        # which has a special suffix to distinguish Swift and Clang modules with the same name
        "module_name": attrs.string(),
        # A prefixed path ($SDKROOT/$PLATFORM_DIR) to swiftinterface textual file.
        "swiftinterface_relative_path": attrs.option(attrs.string(), default = None),  # if `swiftinterface` is None represents a Root node.
        "target": attrs.string(),
    },
)
