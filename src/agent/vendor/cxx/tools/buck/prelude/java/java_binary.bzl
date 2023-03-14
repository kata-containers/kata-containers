# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under both the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree and the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree.

load("@prelude//java:java_toolchain.bzl", "JavaToolchainInfo")
load("@prelude//linking:shared_libraries.bzl", "SharedLibraryInfo", "merge_shared_libraries", "traverse_shared_library_info")
load("@prelude//utils:utils.bzl", "expect")
load(
    ":java_providers.bzl",
    "create_template_info",
    "derive_compiling_deps",
    "get_java_packaging_info",
)

def _generate_script(generate_wrapper: bool.type, native_libs: {str.type: "SharedLibrary"}) -> bool.type:
    # if `generate_wrapper` is set and no native libs then it should be a wrapper script as result,
    # otherwise fat jar will be generated (inner jar or script will be included inside a final fat jar)
    return generate_wrapper and len(native_libs) == 0

def _create_fat_jar(
        ctx: "context",
        java_toolchain: JavaToolchainInfo.type,
        jars: "cmd_args",
        native_libs: {str.type: "SharedLibrary"},
        generate_wrapper: bool.type) -> ["artifact"]:
    extension = "sh" if _generate_script(generate_wrapper, native_libs) else "jar"
    output = ctx.actions.declare_output("{}.{}".format(ctx.label.name, extension))

    args = [
        java_toolchain.fat_jar[RunInfo],
        "--jar_tool",
        java_toolchain.jar,
        "--jar_builder_tool",
        cmd_args(java_toolchain.jar_builder, delimiter = " "),
        "--output",
        output.as_output(),
        "--jars_file",
        ctx.actions.write("jars_file", jars),
    ]

    if native_libs:
        expect(
            java_toolchain.is_bootstrap_toolchain == False,
            "Bootstrap java toolchain could not be used for java_binary() with native code.",
        )
        args += [
            "--fat_jar_lib",
            java_toolchain.fat_jar_main_class_lib,
            "--native_libs_file",
            ctx.actions.write("native_libs", [cmd_args([so_name, native_lib.lib.output], delimiter = " ") for so_name, native_lib in native_libs.items()]),
            # fat jar's main class
            "--fat_jar_main_class",
            "com.facebook.buck.jvm.java.FatJarMain",
            # native libraries directory name. Main class expects to find libraries packed inside this directory.
            "--fat_jar_native_libs_directory_name",
            "nativelibs",
        ]

    main_class = ctx.attrs.main_class
    if main_class:
        args += ["--main_class", main_class]

    manifest_file = ctx.attrs.manifest_file
    if manifest_file:
        args += ["--manifest", manifest_file]

    blocklist = ctx.attrs.blacklist
    if blocklist:
        args += ["--blocklist", ctx.actions.write("blocklist_args", blocklist)]

    if ctx.attrs.meta_inf_directory:
        args += ["--meta_inf_directory", ctx.attrs.meta_inf_directory]

    outputs = [output]
    if generate_wrapper:
        classpath_args_output = ctx.actions.declare_output("classpath_args")
        args += [
            "--generate_wrapper",
            "--classpath_args_output",
            classpath_args_output.as_output(),
            "--java_tool",
            java_toolchain.java[RunInfo],
            "--script_marker_file_name",
            "wrapper_script",
        ]
        outputs.append(classpath_args_output)

    fat_jar_cmd = cmd_args(args)
    fat_jar_cmd.hidden(jars, [native_lib.lib.output for native_lib in native_libs.values()])

    ctx.actions.run(fat_jar_cmd, category = "fat_jar")

    if generate_wrapper == False:
        expect(
            len(outputs) == 1,
            "expected exactly one output when creating a fat jar",
        )

    # If `generate_wrapper` is not set then the result will contain only 1 item that represent fat jar artifact.
    # Else if `generate_wrapper` is set then the first item in the result list will be script or far jar, and the second one is for @classpath_args file
    return outputs

def _get_run_cmd(
        attrs: struct.type,
        script_mode: bool.type,
        main_artifact: "artifact",
        java_toolchain: JavaToolchainInfo.type) -> "cmd_args":
    if script_mode:
        return cmd_args(["/bin/bash", main_artifact])
    else:
        return cmd_args([java_toolchain.java[RunInfo]] + attrs.java_args_for_run_info + ["-jar", main_artifact])

def _get_java_tool_artifacts(java_toolchain: JavaToolchainInfo.type) -> ["artifact"]:
    default_info = java_toolchain.java[DefaultInfo]
    return default_info.default_outputs + default_info.other_outputs

def java_binary_impl(ctx: "context") -> ["provider"]:
    """
     java_binary() rule implementation

    Args:
        ctx: rule analysis context
    Returns:
        list of created providers (DefaultInfo and RunInfo)
    """

    if ctx.attrs._build_only_native_code:
        return [
            DefaultInfo(default_outputs = [ctx.actions.write("unused.jar", [])]),
            RunInfo(),
        ]

    packaging_info = get_java_packaging_info(ctx, ctx.attrs.deps, None)

    first_order_deps = derive_compiling_deps(ctx.actions, None, ctx.attrs.deps)
    first_order_libs = [dep.full_library for dep in (list(first_order_deps.traverse()) if first_order_deps else [])]

    shared_library_info = merge_shared_libraries(
        ctx.actions,
        deps = filter(None, [x.get(SharedLibraryInfo) for x in ctx.attrs.deps]),
    )
    native_deps = traverse_shared_library_info(shared_library_info)

    java_toolchain = ctx.attrs._java_toolchain[JavaToolchainInfo]
    need_to_generate_wrapper = ctx.attrs.generate_wrapper == True
    packaging_jar_args = packaging_info.packaging_deps.project_as_args("full_jar_args")
    outputs = _create_fat_jar(ctx, java_toolchain, cmd_args(packaging_jar_args), native_deps, need_to_generate_wrapper)

    main_artifact = outputs[0]
    other_outputs = []

    run_cmd = _get_run_cmd(
        attrs = ctx.attrs,
        script_mode = _generate_script(need_to_generate_wrapper, native_deps),
        main_artifact = main_artifact,
        java_toolchain = java_toolchain,
    )

    if need_to_generate_wrapper:
        classpath_file = outputs[1]
        run_cmd.hidden(
            java_toolchain.java[RunInfo],
            classpath_file,
            packaging_jar_args,
        )
        other_outputs = [classpath_file] + [packaging_jar_args] + _get_java_tool_artifacts(java_toolchain)

    return [
        DefaultInfo(default_outputs = [main_artifact], other_outputs = other_outputs),
        RunInfo(args = run_cmd),
        create_template_info(packaging_info, first_order_libs),
    ]
