# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under both the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree and the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree.

load("@prelude//android:android_providers.bzl", "Aapt2LinkInfo")

BASE_PACKAGE_ID = 0x7f

def get_aapt2_link(
        ctx: "context",
        android_toolchain: "AndroidToolchainInfo",
        aapt2_compile_rules: ["artifact"],
        android_manifest: "artifact",
        includes_vector_drawables: bool.type = False,
        no_auto_version: bool.type = False,
        no_version_transitions: bool.type = False,
        no_auto_add_overlay: bool.type = False,
        use_proto_format: bool.type = False,
        no_resource_removal: bool.type = False,
        should_keep_raw_values: bool.type = False,
        package_id_offset: int.type = 0,
        resource_stable_ids: ["artifact", None] = None,
        preferred_density: [str.type, None] = None,
        min_sdk: [int.type, None] = None,
        filter_locales: bool.type = False,
        locales: [str.type] = [],
        compiled_resource_apks: ["artifact"] = [],
        additional_aapt2_params: [str.type] = [],
        extra_filtered_resources: [str.type] = []) -> Aapt2LinkInfo.type:
    aapt2_command = cmd_args(android_toolchain.aapt2)
    aapt2_command.add("link")

    # aapt2 only supports @ for -R or input files, not for all args, so we pass in all "normal"
    # args here.
    resources_apk = ctx.actions.declare_output("resource-apk.ap_")
    aapt2_command.add(["-o", resources_apk.as_output()])
    proguard_config = ctx.actions.declare_output("proguard_config.pro")
    aapt2_command.add(["--proguard", proguard_config.as_output()])

    # We don't need the R.java output, but aapt2 won't output R.txt unless we also request R.java.
    r_dot_java = ctx.actions.declare_output("initial-rdotjava")
    aapt2_command.add(["--java", r_dot_java.as_output()])
    r_dot_txt = ctx.actions.declare_output("R.txt")
    aapt2_command.add(["--output-text-symbols", r_dot_txt.as_output()])

    aapt2_command.add(["--manifest", android_manifest])
    aapt2_command.add(["-I", android_toolchain.android_jar])

    if includes_vector_drawables:
        aapt2_command.add("--no-version-vectors")
    if no_auto_version:
        aapt2_command.add("--no-auto-version")
    if no_version_transitions:
        aapt2_command.add("--no-version-transitions")
    if not no_auto_add_overlay:
        aapt2_command.add("--auto-add-overlay")
    if use_proto_format:
        aapt2_command.add("--proto-format")
    if no_resource_removal:
        aapt2_command.add("--no-resource-removal")
    if should_keep_raw_values:
        aapt2_command.add("--keep-raw-values")
    if package_id_offset != 0:
        aapt2_command.add(["--package-id", "0x{}".format(BASE_PACKAGE_ID + package_id_offset)])
    if resource_stable_ids != None:
        aapt2_command.add(["--stable-ids", resource_stable_ids])
    if preferred_density != None:
        aapt2_command.add(["--preferred-density", preferred_density])
    if min_sdk != None:
        aapt2_command.add(["--min-sdk-version", min_sdk])
    if filter_locales and len(locales) > 0:
        aapt2_command.add("-c")

        # "NONE" means "en", update the list of locales
        aapt2_command.add(cmd_args([locale if locale != "NONE" else "en" for locale in locales], delimiter = ","))

    for compiled_resource_apk in compiled_resource_apks:
        aapt2_command.add(["-I", compiled_resource_apk])

    aapt2_compile_rules_args_file = ctx.actions.write("aapt2_compile_rules_args_file", cmd_args(aapt2_compile_rules, delimiter = " "))
    aapt2_command.add("-R")
    aapt2_command.add(cmd_args(aapt2_compile_rules_args_file, format = "@{}"))
    aapt2_command.hidden(aapt2_compile_rules)

    aapt2_command.add(additional_aapt2_params)

    ctx.actions.run(aapt2_command, category = "aapt2_link")

    # The normal resource filtering apparatus is super slow, because it extracts the whole apk,
    # strips files out of it, then repackages it.
    #
    # This is a faster filtering step that just uses zip -d to remove entries from the archive.
    # It's also superbly dangerous.
    if len(extra_filtered_resources) > 0:
        filter_resources_cmd = cmd_args()
        filter_resources_cmd.add(["zip", "-d"])
        filter_resources_cmd.add(resources_apk)
        filter_resources_cmd.add(extra_filtered_resources)

        ctx.actions.run(filter_resources_cmd, category = "aapt2_filter_resources")

    return Aapt2LinkInfo(
        primary_resources_apk = resources_apk,
        proguard_config_file = proguard_config,
        r_dot_txt = r_dot_txt,
    )
