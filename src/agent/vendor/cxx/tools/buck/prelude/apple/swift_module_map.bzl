# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under both the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree and the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree.

def write_swift_module_map(
        ctx: "context",
        module_name: str.type,
        sdk_deps: ["SdkCompiledModuleInfo"]) -> "artifact":
    return write_swift_module_map_with_swift_deps(ctx, module_name, sdk_deps, [])

def write_swift_module_map_with_swift_deps(
        ctx: "context",
        module_name: str.type,
        sdk_swift_deps: ["SdkCompiledModuleInfo"],
        swift_deps: ["artifact"]) -> "artifact":
    deps = {}
    for sdk_dep in sdk_swift_deps:
        if sdk_dep.is_swiftmodule:
            deps[sdk_dep.module_name] = {
                "isFramework": sdk_dep.is_framework,
                "moduleName": sdk_dep.module_name,
                "modulePath": sdk_dep.output_artifact,
            }

    for swift_dep in swift_deps:
        # The swiftmodule filename always matches the module name
        name = swift_dep.basename[:-12]
        deps[name] = {
            "isFramework": False,
            "moduleName": name,
            "modulePath": swift_dep,
        }

    return ctx.actions.write_json(
        module_name + ".swift_module_map.json",
        deps.values(),
    )
