# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under both the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree and the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree.

load("@prelude//android:android_binary_native_library_rules.bzl", "get_android_binary_native_library_info")
load("@prelude//android:android_binary_resources_rules.bzl", "get_android_binary_resources_info")
load("@prelude//android:android_build_config.bzl", "generate_android_build_config", "get_build_config_fields")
load("@prelude//android:android_providers.bzl", "AndroidApkInfo", "AndroidApkUnderTestInfo", "BuildConfigField", "CPU_FILTER_TO_ABI_DIRECTORY", "ExopackageInfo", "merge_android_packageable_info")
load("@prelude//android:android_toolchain.bzl", "AndroidToolchainInfo")
load("@prelude//android:configuration.bzl", "get_deps_by_platform")
load("@prelude//android:dex_rules.bzl", "get_multi_dex", "get_single_primary_dex", "get_split_dex_merge_config", "merge_to_single_dex", "merge_to_split_dex")
load("@prelude//android:exopackage.bzl", "get_exopackage_flags")
load("@prelude//android:preprocess_java_classes.bzl", "get_preprocessed_java_classes")
load("@prelude//android:proguard.bzl", "get_proguard_output")
load("@prelude//android:voltron.bzl", "get_target_to_module_mapping")
load("@prelude//java:java_providers.bzl", "KeystoreInfo", "create_java_packaging_dep", "get_all_java_packaging_deps", "get_all_java_packaging_deps_from_packaging_infos")
load("@prelude//utils:utils.bzl", "expect")

def android_apk_impl(ctx: "context") -> ["provider"]:
    sub_targets = {}

    _verify_params(ctx)

    cpu_filters = ctx.attrs.cpu_filters or CPU_FILTER_TO_ABI_DIRECTORY.keys()
    deps_by_platform = get_deps_by_platform(ctx)
    primary_platform = cpu_filters[0]
    deps = deps_by_platform[primary_platform]

    no_dx_target_labels = [no_dx_target.label.raw_target() for no_dx_target in ctx.attrs.no_dx]
    java_packaging_deps = [packaging_dep for packaging_dep in get_all_java_packaging_deps(ctx, deps) if packaging_dep.dex and packaging_dep.dex.dex.owner.raw_target() not in no_dx_target_labels]

    android_packageable_info = merge_android_packageable_info(ctx.label, ctx.actions, deps)
    build_config_infos = list(android_packageable_info.build_config_infos.traverse()) if android_packageable_info.build_config_infos else []

    build_config_libs = _get_build_config_java_libraries(ctx, build_config_infos)
    java_packaging_deps += get_all_java_packaging_deps_from_packaging_infos(ctx, build_config_libs)

    has_proguard_config = ctx.attrs.proguard_config != None or ctx.attrs.android_sdk_proguard_config == "default" or ctx.attrs.android_sdk_proguard_config == "optimized"
    should_pre_dex = not ctx.attrs.disable_pre_dex and not has_proguard_config and not ctx.attrs.preprocess_java_classes_bash

    referenced_resources_lists = [java_packaging_dep.dex.referenced_resources for java_packaging_dep in java_packaging_deps] if ctx.attrs.trim_resource_ids and should_pre_dex else []
    resources_info = get_android_binary_resources_info(
        ctx,
        deps,
        android_packageable_info,
        java_packaging_deps,
        use_proto_format = False,
        referenced_resources_lists = referenced_resources_lists,
        manifest_entries = ctx.attrs.manifest_entries,
    )
    android_toolchain = ctx.attrs._android_toolchain[AndroidToolchainInfo]
    java_packaging_deps += [
        create_java_packaging_dep(
            ctx,
            r_dot_java.library_output.full_library,
            dex_weight_factor = android_toolchain.r_dot_java_weight_factor,
        )
        for r_dot_java in resources_info.r_dot_javas
    ]

    target_to_module_mapping_file = get_target_to_module_mapping(ctx, deps)
    if should_pre_dex:
        pre_dexed_libs = [java_packaging_dep.dex for java_packaging_dep in java_packaging_deps]
        if ctx.attrs.use_split_dex:
            dex_files_info = merge_to_split_dex(
                ctx,
                android_toolchain,
                pre_dexed_libs,
                get_split_dex_merge_config(ctx, android_toolchain),
                target_to_module_mapping_file,
            )
        else:
            dex_files_info = merge_to_single_dex(ctx, android_toolchain, pre_dexed_libs)
    else:
        jars_to_owners = {packaging_dep.jar: packaging_dep.jar.owner.raw_target() for packaging_dep in java_packaging_deps}
        if ctx.attrs.preprocess_java_classes_bash:
            jars_to_owners = get_preprocessed_java_classes(ctx, jars_to_owners)
        if has_proguard_config:
            proguard_output = get_proguard_output(ctx, jars_to_owners, java_packaging_deps, resources_info.proguard_config_file)
            jars_to_owners = proguard_output.jars_to_owners
            sub_targets["proguard_text_output"] = [
                DefaultInfo(
                    default_outputs = [ctx.actions.symlinked_dir(
                        "proguard_text_output",
                        {artifact.basename: artifact for artifact in proguard_output.proguard_artifacts},
                    )],
                ),
            ]
        else:
            proguard_output = None

        if ctx.attrs.use_split_dex:
            dex_files_info = get_multi_dex(
                ctx,
                ctx.attrs._android_toolchain[AndroidToolchainInfo],
                jars_to_owners,
                ctx.attrs.primary_dex_patterns,
                proguard_output.proguard_configuration_output_file if proguard_output else None,
                proguard_output.proguard_mapping_output_file if proguard_output else None,
                is_optimized = has_proguard_config,
                apk_module_graph_file = target_to_module_mapping_file,
            )
        else:
            dex_files_info = get_single_primary_dex(
                ctx,
                ctx.attrs._android_toolchain[AndroidToolchainInfo],
                jars_to_owners.keys(),
                is_optimized = has_proguard_config,
            )

    native_library_info = get_android_binary_native_library_info(ctx, android_packageable_info, deps_by_platform, apk_module_graph_file = target_to_module_mapping_file)
    unstripped_native_libs = native_library_info.unstripped_libs
    sub_targets["unstripped_native_libraries"] = [
        DefaultInfo(
            default_outputs = [ctx.actions.write("unstripped_native_libraries", unstripped_native_libs)],
            other_outputs = unstripped_native_libs,
        ),
    ]
    if resources_info.string_source_map:
        sub_targets["generate_string_resources"] = [DefaultInfo(default_outputs = [resources_info.string_source_map])]

    if dex_files_info.primary_dex_class_names:
        sub_targets["primary_dex_class_names"] = [DefaultInfo(default_outputs = [dex_files_info.primary_dex_class_names])]

    keystore = ctx.attrs.keystore[KeystoreInfo]
    output_apk = build_apk(
        label = ctx.label,
        actions = ctx.actions,
        android_toolchain = ctx.attrs._android_toolchain[AndroidToolchainInfo],
        keystore = keystore,
        dex_files_info = dex_files_info,
        native_library_info = native_library_info,
        resources_info = resources_info,
        compress_resources_dot_arsc = ctx.attrs.resource_compression == "enabled" or ctx.attrs.resource_compression == "enabled_with_strings_as_assets",
    )

    exopackage_info = ExopackageInfo(
        secondary_dex_info = dex_files_info.secondary_dex_exopackage_info,
        native_library_info = native_library_info.exopackage_info,
        resources_info = resources_info.exopackage_info,
    )

    return [
        AndroidApkInfo(apk = output_apk, manifest = resources_info.manifest),
        AndroidApkUnderTestInfo(
            java_packaging_deps = java_packaging_deps,
            keystore = keystore,
            manifest_entries = ctx.attrs.manifest_entries,
            prebuilt_native_library_dirs = native_library_info.apk_under_test_prebuilt_native_library_dirs,
            platforms = deps_by_platform.keys(),
            primary_platform = primary_platform,
            resource_infos = resources_info.unfiltered_resource_infos,
            shared_libraries = native_library_info.apk_under_test_shared_libraries,
        ),
        DefaultInfo(default_outputs = [output_apk], other_outputs = _get_exopackage_outputs(exopackage_info), sub_targets = sub_targets),
        _get_install_info(ctx, output_apk = output_apk, manifest = resources_info.manifest, exopackage_info = exopackage_info),
    ]

def build_apk(
        label: "label",
        actions: "actions",
        keystore: KeystoreInfo.type,
        android_toolchain: AndroidToolchainInfo.type,
        dex_files_info: "DexFilesInfo",
        native_library_info: "AndroidBinaryNativeLibsInfo",
        resources_info: "AndroidBinaryResourcesInfo",
        compress_resources_dot_arsc: bool.type = False) -> "artifact":
    output_apk = actions.declare_output("{}.apk".format(label.name))

    apk_builder_args = cmd_args([
        android_toolchain.apk_builder[RunInfo],
        "--output-apk",
        output_apk.as_output(),
        "--resource-apk",
        resources_info.primary_resources_apk,
        "--dex-file",
        dex_files_info.primary_dex,
        "--keystore-path",
        keystore.store,
        "--keystore-properties-path",
        keystore.properties,
        "--zipalign_tool",
        android_toolchain.zipalign[RunInfo],
    ])

    if compress_resources_dot_arsc:
        apk_builder_args.add("--compress-resources-dot-arsc")

    asset_directories = native_library_info.native_lib_assets + dex_files_info.secondary_dex_dirs
    asset_directories_file = actions.write("asset_directories.txt", asset_directories)
    apk_builder_args.hidden(asset_directories)
    native_library_directories = actions.write("native_library_directories", native_library_info.native_libs_for_primary_apk)
    apk_builder_args.hidden(native_library_info.native_libs_for_primary_apk)
    all_zip_files = [resources_info.packaged_string_assets] if resources_info.packaged_string_assets else []
    zip_files = actions.write("zip_files", all_zip_files)
    apk_builder_args.hidden(all_zip_files)
    jar_files_that_may_contain_resources = actions.write("jar_files_that_may_contain_resources", resources_info.jar_files_that_may_contain_resources)
    apk_builder_args.hidden(resources_info.jar_files_that_may_contain_resources)

    apk_builder_args.add([
        "--asset-directories-list",
        asset_directories_file,
        "--native-libraries-directories-list",
        native_library_directories,
        "--zip-files-list",
        zip_files,
        "--jar-files-that-may-contain-resources-list",
        jar_files_that_may_contain_resources,
    ])

    actions.run(apk_builder_args, category = "apk_build")

    return output_apk

def _get_install_info(ctx: "context", output_apk: "artifact", manifest: "artifact", exopackage_info: ExopackageInfo.type) -> InstallInfo.type:
    files = {
        ctx.attrs.name: output_apk,
        "manifest": manifest,
        "options": generate_install_config(ctx),
    }

    secondary_dex_exopackage_info = exopackage_info.secondary_dex_info
    if secondary_dex_exopackage_info:
        files["secondary_dex_exopackage_info_directory"] = secondary_dex_exopackage_info.directory
        files["secondary_dex_exopackage_info_metadata"] = secondary_dex_exopackage_info.metadata

    native_library_exopackage_info = exopackage_info.native_library_info
    if native_library_exopackage_info:
        files["native_library_exopackage_info_directory"] = native_library_exopackage_info.directory
        files["native_library_exopackage_info_metadata"] = native_library_exopackage_info.metadata

    resources_info = exopackage_info.resources_info
    if resources_info:
        if resources_info.assets:
            files["resources_exopackage_assets"] = resources_info.assets
            files["resources_exopackage_assets_hash"] = resources_info.assets_hash

        files["resources_exopackage_res"] = resources_info.res
        files["resources_exopackage_res_hash"] = resources_info.res_hash
        files["resources_exopackage_third_party_jar_resources"] = resources_info.third_party_jar_resources
        files["resources_exopackage_third_party_jar_resources_hash"] = resources_info.third_party_jar_resources_hash

    if secondary_dex_exopackage_info or native_library_exopackage_info or resources_info:
        files["exopackage_agent_apk"] = ctx.attrs._android_toolchain[AndroidToolchainInfo].exopackage_agent_apk

    return InstallInfo(
        installer = ctx.attrs._android_installer,
        files = files,
    )

def _get_build_config_java_libraries(ctx: "context", build_config_infos: ["AndroidBuildConfigInfo"]) -> ["JavaPackagingInfo"]:
    # BuildConfig deps should not be added for instrumented APKs because BuildConfig.class has
    # already been added to the APK under test.
    if ctx.attrs.package_type == "instrumented":
        return []

    build_config_constants = [
        BuildConfigField(type = "boolean", name = "DEBUG", value = str(ctx.attrs.package_type != "release").lower()),
        BuildConfigField(type = "boolean", name = "IS_EXOPACKAGE", value = str(len(ctx.attrs.exopackage_modes) > 0).lower()),
        BuildConfigField(type = "int", name = "EXOPACKAGE_FLAGS", value = str(get_exopackage_flags(ctx.attrs.exopackage_modes))),
    ]

    default_build_config_fields = get_build_config_fields(ctx.attrs.build_config_values)

    java_libraries = []
    java_packages_seen = []
    for build_config_info in build_config_infos:
        java_package = build_config_info.package
        expect(java_package not in java_packages_seen, "Got the same java_package {} for different AndroidBuildConfigs".format(java_package))
        java_packages_seen.append(java_package)

        all_build_config_values = {}
        for build_config_field in build_config_info.build_config_fields + default_build_config_fields + build_config_constants:
            all_build_config_values[build_config_field.name] = build_config_field

        java_libraries.append(generate_android_build_config(
            ctx,
            java_package,
            java_package,
            True,  # use_constant_expressions
            all_build_config_values.values(),
            ctx.attrs.build_config_values_file[DefaultInfo].default_outputs[0] if type(ctx.attrs.build_config_values_file) == "dependency" else ctx.attrs.build_config_values_file,
        )[1])

    return java_libraries

def _get_exopackage_outputs(exopackage_info: ExopackageInfo.type) -> ["artifact"]:
    outputs = []
    secondary_dex_exopackage_info = exopackage_info.secondary_dex_info
    if secondary_dex_exopackage_info:
        outputs.append(secondary_dex_exopackage_info.metadata)
        outputs.append(secondary_dex_exopackage_info.directory)

    native_library_exopackage_info = exopackage_info.native_library_info
    if native_library_exopackage_info:
        outputs.append(native_library_exopackage_info.metadata)
        outputs.append(native_library_exopackage_info.directory)

    resources_info = exopackage_info.resources_info
    if resources_info:
        outputs.append(resources_info.res)
        outputs.append(resources_info.res_hash)
        outputs.append(resources_info.third_party_jar_resources)
        outputs.append(resources_info.third_party_jar_resources_hash)

        if resources_info.assets:
            outputs.append(resources_info.assets)
            outputs.append(resources_info.assets_hash)

    return outputs

def _verify_params(ctx: "context"):
    expect(ctx.attrs.aapt_mode == "aapt2", "aapt1 is deprecated!")
    expect(ctx.attrs.dex_tool == "d8", "dx is deprecated!")
    expect(ctx.attrs.allow_r_dot_java_in_secondary_dex == True)

def generate_install_config(ctx: "context") -> "artifact":
    data = get_install_config()
    return ctx.actions.write_json("install_android_options.json", data)

def get_install_config() -> {str.type: ""}:
    # TODO: read from toolchains
    return {
        "adb_executable": read_config("android", "adb", "/opt/android_sdk/platform-tools/adb"),
        "adb_restart_on_failure": read_config("adb", "adb_restart_on_failure", "false"),
        "agent_port_base": read_config("adb", "agent_port_base", "2828"),
        "always_use_java_agent": read_config("adb", "always_use_java_agent", "false"),
        "is_zstd_compression_enabled": read_config("adb", "is_zstd_compression_enabled", "false"),
        "multi_install_mode": read_config("adb", "multi_install_mode", "false"),
        "skip_install_metadata": read_config("adb", "skip_install_metadata", "false"),
    }
