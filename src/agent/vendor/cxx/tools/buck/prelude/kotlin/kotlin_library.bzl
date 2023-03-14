# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under both the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree and the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree.

load("@prelude//android:android_providers.bzl", "merge_android_packageable_info")
load(
    "@prelude//java:java_library.bzl",
    "build_java_library",
    "split_on_archives_and_plain_files",
)
load(
    "@prelude//java:java_providers.bzl",
    "JavaLibraryInfo",
    "JavaPackagingDepTSet",
    "JavaPackagingInfo",
    "JavaProviders",
    "create_java_library_providers",
    "create_native_providers",
    "derive_compiling_deps",
    "to_list",
)
load(
    "@prelude//java:java_toolchain.bzl",
    "AbiGenerationMode",
    "JavaToolchainInfo",
)
load("@prelude//java/plugins:java_annotation_processor.bzl", "create_ap_params", "create_ksp_ap_params")
load("@prelude//java/plugins:java_plugin.bzl", "create_plugin_params")
load("@prelude//java/utils:java_utils.bzl", "get_abi_generation_mode", "get_default_info", "get_java_version_attributes", "get_path_separator")
load(
    "@prelude//kotlin:kotlin_toolchain.bzl",
    "KotlinToolchainInfo",
)
load("@prelude//kotlin:kotlincd_jar_creator.bzl", "create_jar_artifact_kotlincd")
load("@prelude//utils:utils.bzl", "map_idx")

_JAVA_OR_KOTLIN_FILE_EXTENSION = [".java", ".kt"]

def _create_kotlin_sources(
        ctx: "context",
        srcs: ["artifact"],
        deps: ["dependency"],
        annotation_processor_params: [["AnnotationProcessorParams"], None],
        ksp_annotation_processor_params: ["AnnotationProcessorParams", None],
        additional_classpath_entries: ["artifact"]) -> ("artifact", ["artifact", None], ["artifact", None]):
    """
    Runs kotlinc on the provided kotlin sources.
    """

    kotlin_toolchain = ctx.attrs._kotlin_toolchain[KotlinToolchainInfo]
    compile_kotlin_tool = kotlin_toolchain.compile_kotlin[RunInfo]
    kotlinc = kotlin_toolchain.kotlinc[RunInfo]
    kotlinc_output = ctx.actions.declare_output("kotlinc_classes_output")

    compile_kotlin_cmd = cmd_args([
        compile_kotlin_tool,
        "--kotlinc_output",
        kotlinc_output.as_output(),
    ])
    java_toolchain = ctx.attrs._java_toolchain[JavaToolchainInfo]
    zip_scrubber_args = ["--zip_scrubber", cmd_args(java_toolchain.zip_scrubber, delimiter = " ")]
    compile_kotlin_cmd.add(zip_scrubber_args)

    kotlinc_cmd_args = cmd_args([kotlinc])

    compiling_classpath = [] + additional_classpath_entries
    compiling_deps_tset = derive_compiling_deps(ctx.actions, None, deps + kotlin_toolchain.kotlinc_classpath)
    if compiling_deps_tset:
        compiling_classpath.extend(
            [compiling_dep.abi for compiling_dep in list(compiling_deps_tset.traverse())],
        )

    classpath_args = cmd_args(
        compiling_classpath,
        delimiter = get_path_separator(),
    )

    # write joined classpath string into args file
    classpath_args_file, _ = ctx.actions.write(
        "kotlinc_classpath",
        classpath_args,
        allow_args = True,
    )

    compile_kotlin_cmd.hidden([compiling_classpath])

    kotlinc_cmd_args.add(["-classpath"])
    kotlinc_cmd_args.add(cmd_args(classpath_args_file, format = "@{}"))

    module_name = ctx.label.package.replace("/", ".") + "." + ctx.label.name
    kotlinc_cmd_args.add(
        [
            "-module-name",
            module_name,
            "-no-stdlib",
            "-no-reflect",
        ] + ctx.attrs.extra_kotlinc_arguments,
    )

    jvm_target = _get_kotlinc_compatible_target(ctx.attrs.target) if ctx.attrs.target else None
    if jvm_target:
        kotlinc_cmd_args.add([
            "-jvm-target",
            jvm_target,
        ])

    kapt_generated_sources_output = None
    if annotation_processor_params:
        compile_kotlin_cmd.add(["--kapt_annotation_processing_jar", kotlin_toolchain.annotation_processing_jar[JavaLibraryInfo].library_output.full_library])
        compile_kotlin_cmd.add(["--kapt_annotation_processors", ",".join([p for ap in annotation_processor_params for p in ap.processors])])
        compile_kotlin_cmd.add(["--kapt_annotation_processor_params", ";".join([p for ap in annotation_processor_params for p in ap.params])])

        annotation_processor_classpath_tsets = (
            filter(None, ([ap.deps for ap in annotation_processor_params])) +
            [dep[JavaPackagingInfo].packaging_deps for dep in [kotlin_toolchain.annotation_processing_jar, kotlin_toolchain.kotlin_stdlib]]
        )
        annotation_processor_classpath = ctx.actions.tset(
            JavaPackagingDepTSet,
            children = annotation_processor_classpath_tsets,
        ).project_as_args("full_jar_args")
        kapt_classpath_file = ctx.actions.write("kapt_classpath_file", annotation_processor_classpath)
        compile_kotlin_cmd.add(["--kapt_classpath_file", kapt_classpath_file])
        compile_kotlin_cmd.hidden(annotation_processor_classpath)

        sources_output = ctx.actions.declare_output("kapt_sources_output")
        compile_kotlin_cmd.add(["--kapt_sources_output", sources_output.as_output()])
        classes_output = ctx.actions.declare_output("kapt_classes_output")
        compile_kotlin_cmd.add(["--kapt_classes_output", classes_output.as_output()])
        stubs = ctx.actions.declare_output("kapt_stubs")
        compile_kotlin_cmd.add(["--kapt_stubs", stubs.as_output()])

        kapt_generated_sources_output = ctx.actions.declare_output("kapt_generated_sources_output.src.zip")
        compile_kotlin_cmd.add(["--kapt_generated_sources_output", kapt_generated_sources_output.as_output()])
        compile_kotlin_cmd.add(["--kapt_base64_encoder", cmd_args(kotlin_toolchain.kapt_base64_encoder[RunInfo], delimiter = " ")])
        generated_kotlin_output = ctx.actions.declare_output("kapt_generated_kotlin_output")
        compile_kotlin_cmd.add(["--kapt_generated_kotlin_output", generated_kotlin_output.as_output()])
        if jvm_target:
            compile_kotlin_cmd.add(["--kapt_jvm_target", jvm_target])

    friend_paths = ctx.attrs.friend_paths
    if friend_paths:
        concat_friends_paths = cmd_args([friend_path.library_output.abi for friend_path in map_idx(JavaLibraryInfo, friend_paths)], delimiter = ",")
        kotlinc_cmd_args.add(cmd_args(["-Xfriend-paths", concat_friends_paths], delimiter = "="))

    zipped_sources, plain_sources = split_on_archives_and_plain_files(srcs, _JAVA_OR_KOTLIN_FILE_EXTENSION)

    kotlinc_cmd_args.add(plain_sources)

    ksp_zipped_sources_output = None
    if ksp_annotation_processor_params:
        ksp_cmd = cmd_args(compile_kotlin_tool)
        ksp_cmd.add(zip_scrubber_args)

        if ksp_annotation_processor_params.deps:
            ksp_cmd.add(["--ksp_processor_jars"])
            ksp_cmd.add(cmd_args(ksp_annotation_processor_params.deps.project_as_args("full_jar_args"), delimiter = ","))

        ksp_cmd.add(["--ksp_classpath", classpath_args])
        ksp_classes_and_resources_output = ctx.actions.declare_output("ksp_output_dir/ksp_classes_and_resources_output")
        ksp_cmd.add(["--ksp_classes_and_resources_output", ksp_classes_and_resources_output.as_output()])
        ksp_output = cmd_args(ksp_classes_and_resources_output.as_output()).parent()
        ksp_cmd.add(["--ksp_output", ksp_output])
        ksp_sources_output = ctx.actions.declare_output("ksp_output_dir/ksp_sources_output")
        ksp_cmd.add(["--ksp_sources_output", ksp_sources_output.as_output()])
        ksp_zipped_sources_output = ctx.actions.declare_output("ksp_output_dir/ksp_zipped_sources_output.src.zip")
        ksp_cmd.add(["--ksp_zipped_sources_output", ksp_zipped_sources_output.as_output()])
        ksp_cmd.add(["--ksp_project_base_dir", ctx.label.path])

        ksp_kotlinc_cmd_args = cmd_args(kotlinc_cmd_args)
        _add_plugins(ctx, ksp_kotlinc_cmd_args, ksp_cmd, is_ksp = True)

        ksp_cmd_args_file, _ = ctx.actions.write(
            "ksp_kotlinc_cmd",
            ksp_kotlinc_cmd_args,
            allow_args = True,
        )

        ksp_cmd.add("--kotlinc_cmd_file")
        ksp_cmd.add(ksp_cmd_args_file)
        ksp_cmd.hidden(ksp_kotlinc_cmd_args)

        ctx.actions.run(ksp_cmd, category = "ksp_kotlinc")

        zipped_sources = (zipped_sources or []) + [ksp_zipped_sources_output]
        compile_kotlin_cmd.add(["--ksp_generated_classes_and_resources", ksp_classes_and_resources_output])

    _add_plugins(ctx, kotlinc_cmd_args, compile_kotlin_cmd, is_ksp = False)

    if zipped_sources:
        zipped_sources_file = ctx.actions.write("kotlinc_zipped_source_args", zipped_sources)
        compile_kotlin_cmd.add(["--zipped_sources_file", zipped_sources_file])
        compile_kotlin_cmd.hidden(zipped_sources)

    args_file, _ = ctx.actions.write(
        "kotlinc_cmd",
        kotlinc_cmd_args,
        allow_args = True,
    )

    compile_kotlin_cmd.hidden([plain_sources])

    compile_kotlin_cmd.add("--kotlinc_cmd_file")
    compile_kotlin_cmd.add(args_file)
    compile_kotlin_cmd.hidden(kotlinc_cmd_args)

    ctx.actions.run(compile_kotlin_cmd, category = "kotlinc")

    return kotlinc_output, kapt_generated_sources_output, ksp_zipped_sources_output

def _is_ksp_plugin(plugin: str.type) -> bool.type:
    return "symbol-processing" in plugin

def _add_plugins(
        ctx: "context",
        kotlinc_cmd_args: "cmd_args",
        compile_kotlin_cmd: "cmd_args",
        is_ksp: bool.type):
    for plugin, plugin_options in ctx.attrs.kotlin_compiler_plugins.items():
        if _is_ksp_plugin(str(plugin)) != is_ksp:
            continue

        kotlinc_cmd_args.add(cmd_args(["-Xplugin", plugin], delimiter = "="))
        options = []
        for option_key, option_val in plugin_options.items():
            # "_codegen_dir_" means buck should provide a dir
            if option_val == "__codegen_dir__":
                option_val = ctx.actions.declare_output("kotlin_compiler_plugin_dir")
                options.append(cmd_args([option_key, option_val.as_output()], delimiter = "="))
                compile_kotlin_cmd.add(["--kotlin_compiler_plugin_dir", option_val.as_output()])
            else:
                options.append(cmd_args([option_key, option_val], delimiter = "="))

        if options:
            kotlinc_cmd_args.add(["-P", cmd_args(options, delimiter = ",")])

# kotlinc is strict about the target that you can pass, e.g.
# error: unknown JVM target version: 8.  Supported versions: 1.6, 1.8, 9, 10, 11, 12
def _get_kotlinc_compatible_target(target: str.type) -> str.type:
    return "1.6" if target == "6" else "1.8" if target == "8" else target

def kotlin_library_impl(ctx: "context") -> ["provider"]:
    packaging_deps = ctx.attrs.deps + ctx.attrs.exported_deps + ctx.attrs.runtime_deps
    if ctx.attrs._build_only_native_code:
        shared_library_info, cxx_resource_info = create_native_providers(ctx.actions, ctx.label, packaging_deps)
        return [
            shared_library_info,
            cxx_resource_info,
            # Add an unused default output in case this target is used an an attr.source() anywhere.
            DefaultInfo(default_outputs = [ctx.actions.write("unused.jar", [])]),
            TemplatePlaceholderInfo(keyed_variables = {
                "classpath": "unused_but_needed_for_analysis",
            }),
        ]

    java_providers = build_kotlin_library(ctx)
    return to_list(java_providers) + [
        # TODO(T107163344) this shouldn't be in kotlin_library itself, use overlays to remove it.
        merge_android_packageable_info(
            ctx.label,
            ctx.actions,
            ctx.attrs.deps + ctx.attrs.exported_deps + ctx.attrs.runtime_deps,
        ),
    ]

def build_kotlin_library(
        ctx: "context",
        additional_classpath_entries: ["artifact"] = [],
        bootclasspath_entries: ["artifact"] = []) -> "JavaProviders":
    srcs = ctx.attrs.srcs
    has_kotlin_srcs = any([src.extension == ".kt" or src.basename.endswith(".src.zip") or src.basename.endswith("-sources.jar") for src in srcs])

    if not has_kotlin_srcs:
        return build_java_library(
            ctx,
            ctx.attrs.srcs,
            bootclasspath_entries = bootclasspath_entries,
            additional_classpath_entries = additional_classpath_entries,
            # Match buck1, which always does class ABI generation for Kotlin targets unless explicitly specified.
            override_abi_generation_mode = get_abi_generation_mode(ctx.attrs.abi_generation_mode) or AbiGenerationMode("class"),
        )

    else:
        deps_query = getattr(ctx.attrs, "deps_query", []) or []
        provided_deps_query = getattr(ctx.attrs, "provided_deps_query", []) or []
        deps = (
            ctx.attrs.deps +
            deps_query +
            ctx.attrs.exported_deps +
            ctx.attrs.provided_deps +
            provided_deps_query +
            ctx.attrs.exported_provided_deps
        )
        annotation_processor_params = create_ap_params(
            ctx,
            ctx.attrs.plugins,
            ctx.attrs.annotation_processors,
            ctx.attrs.annotation_processor_params,
            ctx.attrs.annotation_processor_deps,
        )
        ksp_annotation_processor_params = create_ksp_ap_params(ctx, ctx.attrs.plugins)

        kotlin_toolchain = ctx.attrs._kotlin_toolchain[KotlinToolchainInfo]
        if kotlin_toolchain.kotlinc_protocol == "classic":
            kotlinc_classes, kapt_generated_sources, ksp_generated_sources = _create_kotlin_sources(
                ctx,
                ctx.attrs.srcs,
                deps,
                annotation_processor_params,
                ksp_annotation_processor_params,
                # kotlic doesn't support -bootclasspath param, so adding `bootclasspath_entries` into kotlin classpath
                additional_classpath_entries + bootclasspath_entries,
            )
            srcs = [src for src in ctx.attrs.srcs if not src.extension == ".kt"]
            if kapt_generated_sources:
                srcs.append(kapt_generated_sources)
            if ksp_generated_sources:
                srcs.append(ksp_generated_sources)
            java_lib = build_java_library(
                ctx,
                srcs,
                run_annotation_processors = False,
                bootclasspath_entries = bootclasspath_entries,
                additional_classpath_entries = [kotlinc_classes] + additional_classpath_entries,
                additional_compiled_srcs = kotlinc_classes,
                generated_sources = filter(None, [kapt_generated_sources, ksp_generated_sources]),
            )
            return java_lib
        elif kotlin_toolchain.kotlinc_protocol == "kotlincd":
            source_level, target_level = get_java_version_attributes(ctx)
            outputs = create_jar_artifact_kotlincd(
                actions = ctx.actions,
                actions_prefix = "",
                abi_generation_mode = get_abi_generation_mode(ctx.attrs.abi_generation_mode),
                java_toolchain = ctx.attrs._java_toolchain[JavaToolchainInfo],
                kotlin_toolchain = kotlin_toolchain,
                label = ctx.label,
                srcs = srcs,
                remove_classes = ctx.attrs.remove_classes,
                resources = ctx.attrs.resources,
                resources_root = ctx.attrs.resources_root,
                ap_params = annotation_processor_params + ([ksp_annotation_processor_params] if ksp_annotation_processor_params else []),
                plugin_params = create_plugin_params(ctx, ctx.attrs.plugins),
                source_level = source_level,
                target_level = target_level,
                deps = deps,
                required_for_source_only_abi = ctx.attrs.required_for_source_only_abi,
                source_only_abi_deps = ctx.attrs.source_only_abi_deps,
                extra_arguments = ctx.attrs.extra_arguments,
                additional_classpath_entries = additional_classpath_entries,
                bootclasspath_entries = bootclasspath_entries,
                is_building_android_binary = ctx.attrs._is_building_android_binary,
                friend_paths = ctx.attrs.friend_paths,
                kotlin_compiler_plugins = ctx.attrs.kotlin_compiler_plugins,
                extra_kotlinc_arguments = ctx.attrs.extra_kotlinc_arguments,
            )

            java_library_info, java_packaging_info, shared_library_info, cxx_resource_info, template_placeholder_info, intellij_info = create_java_library_providers(
                ctx,
                library_output = outputs.classpath_entry if outputs else None,
                declared_deps = ctx.attrs.deps + deps_query,
                exported_deps = ctx.attrs.exported_deps,
                provided_deps = ctx.attrs.provided_deps + provided_deps_query,
                exported_provided_deps = ctx.attrs.exported_provided_deps,
                runtime_deps = ctx.attrs.runtime_deps,
                needs_desugar = source_level > 7 or target_level > 7,
                generated_sources = [outputs.annotation_processor_output] if outputs and outputs.annotation_processor_output else [],
            )

            default_info = get_default_info(outputs)
            return JavaProviders(
                java_library_info = java_library_info,
                java_library_intellij_info = intellij_info,
                java_packaging_info = java_packaging_info,
                shared_library_info = shared_library_info,
                cxx_resource_info = cxx_resource_info,
                template_placeholder_info = template_placeholder_info,
                default_info = default_info,
            )
        else:
            fail("unrecognized kotlinc protocol `{}`".format(kotlin_toolchain.kotlinc_protocol))
