# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under both the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree and the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree.

load(
    "@prelude//java:java_providers.bzl",
    "make_compile_outputs",
)
load("@prelude//java:java_resources.bzl", "get_resources_map")
load("@prelude//java:java_toolchain.bzl", "AbiGenerationMode")
load(
    "@prelude//jvm:cd_jar_creator_util.bzl",
    "OutputPaths",
    "TargetType",
    "add_java_7_8_bootclasspath",
    "add_output_paths_to_cmd_args",
    "base_qualified_name",
    "declare_prefixed_output",
    "define_output_paths",
    "encode_base_jar_command",
    "encode_jar_params",
    "encode_path",
    "generate_abi_jars",
    "get_abi_generation_mode",
    "prepare_final_jar",
)

def create_jar_artifact_javacd(
        actions: "actions",
        actions_prefix: str.type,
        abi_generation_mode: [AbiGenerationMode.type, None],
        java_toolchain: "JavaToolchainInfo",
        label,
        output: ["artifact", None],
        javac_tool: ["", None],
        srcs: ["artifact"],
        remove_classes: [str.type],
        resources: ["artifact"],
        resources_root: [str.type, None],
        manifest_file: ["artifact", None],
        ap_params: ["AnnotationProcessorParams"],
        plugin_params: ["PluginParams", None],
        source_level: int.type,
        target_level: int.type,
        deps: ["dependency"],
        required_for_source_only_abi: bool.type,
        source_only_abi_deps: ["dependency"],
        extra_arguments: ["string"],
        additional_classpath_entries: ["artifact"],
        additional_compiled_srcs: ["artifact", None],
        bootclasspath_entries: ["artifact"],
        is_building_android_binary: bool.type) -> "JavaCompileOutputs":
    if javac_tool != None:
        # TODO(cjhopman): We can probably handle this better. I think we should be able to just use the non-javacd path.
        fail("cannot set explicit javac on library when using javacd")

    resources_map = get_resources_map(java_toolchain, label.package, resources, resources_root)

    # TODO(cjhopman): Handle manifest file.
    _ = manifest_file
    _ = AbiGenerationMode("class")

    bootclasspath_entries = add_java_7_8_bootclasspath(target_level, bootclasspath_entries, java_toolchain)
    abi_generation_mode = get_abi_generation_mode(abi_generation_mode, java_toolchain, srcs, ap_params)

    # now prefix is just used for categories and prefixes of path segments, so we want it either non-empty or w/ a trailing underscore
    if actions_prefix:
        actions_prefix += "_"

    output_paths = define_output_paths(actions, actions_prefix)
    path_to_class_hashes_out = declare_prefixed_output(actions, actions_prefix, "classes.txt")

    def encode_library_command(output_paths: OutputPaths.type, path_to_class_hashes: "artifact") -> struct.type:
        target_type = TargetType("library")

        base_jar_command = encode_base_jar_command(
            target_type,
            output_paths,
            remove_classes,
            label,
            actions,
            deps,
            additional_classpath_entries,
            source_only_abi_deps,
            bootclasspath_entries,
            source_level,
            target_level,
            abi_generation_mode,
            srcs,
            resources_map,
            ap_params,
            plugin_params,
            extra_arguments,
            track_class_usage = True,
            build_target_value_extra_params = None,
        )

        return struct(
            baseCommandParams = struct(
                withDownwardApi = True,
                spoolMode = "DIRECT_TO_JAR",
            ),
            libraryJarCommand = struct(
                baseJarCommand = base_jar_command,
                libraryJarBaseCommand = struct(
                    pathToClasses = encode_path(output_paths.jar.as_output()),
                    rootOutput = encode_path(output_paths.jar_parent.as_output()),
                    pathToClassHashes = encode_path(path_to_class_hashes.as_output()),
                    annotationsPath = encode_path(output_paths.annotations.as_output()),
                ),
            ),
        )

    def encode_abi_command(output_paths: OutputPaths.type, target_type: TargetType.type) -> struct.type:
        base_jar_command = encode_base_jar_command(
            target_type,
            output_paths,
            remove_classes,
            label,
            actions,
            deps,
            additional_classpath_entries,
            source_only_abi_deps,
            bootclasspath_entries,
            source_level,
            target_level,
            abi_generation_mode,
            srcs,
            resources_map,
            ap_params,
            plugin_params,
            extra_arguments,
            track_class_usage = True,
            build_target_value_extra_params = None,
        )
        abi_params = encode_jar_params(remove_classes, output_paths)

        abi_command = struct(
            baseJarCommand = base_jar_command,
            abiJarParameters = abi_params,
        )

        return struct(
            baseCommandParams = struct(
                withDownwardApi = True,
                spoolMode = "DIRECT_TO_JAR",
            ),
            abiJarCommand = abi_command,
        )

    # buildifier: disable=uninitialized
    def define_javacd_action(actions_prefix: str.type, encoded_command: struct.type, qualified_name: str.type, output_paths: OutputPaths.type, path_to_class_hashes: ["artifact", None]):
        proto = declare_prefixed_output(actions, actions_prefix, "jar_command.proto.json")

        classpath_jars_tag = actions.artifact_tag()

        proto_with_inputs = classpath_jars_tag.tag_inputs(actions.write_json(proto, encoded_command, with_inputs = True))

        cmd = cmd_args([
            java_toolchain.javac,
            "--action-id",
            qualified_name,
            "--command-file",
            proto_with_inputs,
        ])
        cmd = add_output_paths_to_cmd_args(cmd, output_paths, path_to_class_hashes)

        # TODO(cjhopman): make sure this works both locally and remote.
        event_pipe_out = declare_prefixed_output(actions, actions_prefix, "events.data")

        dep_files = {}
        if srcs and java_toolchain.dep_files == "simple":
            dep_files["classpath_jars"] = classpath_jars_tag
            used_classes_json = output_paths.jar_parent.project("used-classes.json")
            dep_file = declare_prefixed_output(actions, actions_prefix, "dep_file.txt")

            # TODO(T134944772) We won't need this once we can do tag_artifacts on a JSON projection,
            # but for now we have to tag all the inputs on the .proto definition, and so we need to
            # tell the dep file to include all the inputs that the compiler won't report.
            srcs_and_resources = actions.write(
                declare_prefixed_output(actions, actions_prefix, "srcs_and_resources"),
                srcs + resources_map.values(),
            )

            cmd = cmd_args([
                java_toolchain.used_classes_to_dep_file[RunInfo],
                srcs_and_resources,
                used_classes_json.as_output(),
                classpath_jars_tag.tag_artifacts(dep_file.as_output()),
                cmd,
            ])

        actions.run(
            cmd,
            env = {
                "BUCK_EVENT_PIPE": event_pipe_out.as_output(),
                "JAVACD_ABSOLUTE_PATHS_ARE_RELATIVE_TO_CWD": "1",
            },
            category = "{}javacd_jar".format(actions_prefix),
            dep_files = dep_files,
        )

    command = encode_library_command(output_paths, path_to_class_hashes_out)
    define_javacd_action(actions_prefix, command, base_qualified_name(label), output_paths, path_to_class_hashes_out)
    final_jar = prepare_final_jar(actions, actions_prefix, output, output_paths, additional_compiled_srcs, java_toolchain.jar_builder)
    class_abi, source_abi, source_only_abi, classpath_abi = generate_abi_jars(
        actions,
        actions_prefix,
        label,
        abi_generation_mode,
        additional_compiled_srcs,
        is_building_android_binary,
        java_toolchain.class_abi_generator,
        final_jar,
        encode_abi_command,
        define_javacd_action,
    )

    result = make_compile_outputs(
        full_library = final_jar,
        class_abi = class_abi,
        source_abi = source_abi,
        source_only_abi = source_only_abi,
        classpath_abi = classpath_abi,
        required_for_source_only_abi = required_for_source_only_abi,
        annotation_processor_output = output_paths.annotations,
    )
    return result
