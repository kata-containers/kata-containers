# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under both the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree and the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree.

load("@prelude//:attributes.bzl", "RType")
load("@prelude//:resources.bzl", "gather_resources")
load("@prelude//android:aapt2_link.bzl", "get_aapt2_link")
load("@prelude//android:android_manifest.bzl", "generate_android_manifest")
load("@prelude//android:android_providers.bzl", "AndroidBinaryResourcesInfo", "AndroidResourceInfo", "ExopackageResourcesInfo")
load("@prelude//android:android_resource.bzl", "aapt2_compile")
load("@prelude//android:android_toolchain.bzl", "AndroidToolchainInfo")
load("@prelude//android:r_dot_java.bzl", "generate_r_dot_javas")
load("@prelude//java:java_toolchain.bzl", "JavaToolchainInfo")
load("@prelude//utils:utils.bzl", "expect")

def get_android_binary_resources_info(
        ctx: "context",
        deps: ["dependency"],
        android_packageable_info: "AndroidPackageableInfo",
        java_packaging_deps: ["JavaPackagingDep"],
        use_proto_format: bool.type,
        referenced_resources_lists: ["artifact"],
        manifest_entries: dict.type = {},
        resource_infos_to_exclude: [AndroidResourceInfo.type] = []) -> "AndroidBinaryResourcesInfo":
    android_toolchain = ctx.attrs._android_toolchain[AndroidToolchainInfo]
    unfiltered_resource_infos = [resource_info for resource_info in list(android_packageable_info.resource_infos.traverse() if android_packageable_info.resource_infos else []) if resource_info not in resource_infos_to_exclude]
    resource_infos, override_symbols, string_files_list, string_files_res_dirs = _maybe_filter_resources(
        ctx,
        unfiltered_resource_infos,
        android_toolchain,
    )

    android_manifest = _get_manifest(ctx, android_packageable_info, manifest_entries)

    aapt2_link_info = get_aapt2_link(
        ctx,
        ctx.attrs._android_toolchain[AndroidToolchainInfo],
        [resource_info.aapt2_compile_output for resource_info in resource_infos if resource_info.aapt2_compile_output != None],
        android_manifest,
        includes_vector_drawables = getattr(ctx.attrs, "includes_vector_drawables", False),
        no_auto_version = getattr(ctx.attrs, "no_auto_version_resources", False),
        no_version_transitions = getattr(ctx.attrs, "no_version_transitions_resources", False),
        no_auto_add_overlay = getattr(ctx.attrs, "no_auto_add_overlay_resources", False),
        use_proto_format = use_proto_format,
        no_resource_removal = True,
        package_id_offset = 0,
        should_keep_raw_values = getattr(ctx.attrs, "aapt2_keep_raw_values", False),
        resource_stable_ids = getattr(ctx.attrs, "resource_stable_ids", None),
        compiled_resource_apks = [],
        additional_aapt2_params = getattr(ctx.attrs, "additional_aapt_params", []),
        extra_filtered_resources = getattr(ctx.attrs, "extra_filtered_resources", []),
        locales = getattr(ctx.attrs, "locales", []),
        filter_locales = getattr(ctx.attrs, "aapt2_locale_filtering", False),
    )

    prebuilt_jars = [packaging_dep.jar for packaging_dep in java_packaging_deps if packaging_dep.is_prebuilt_jar]

    cxx_resources = _get_cxx_resources(ctx, deps)
    is_exopackaged_enabled_for_resources = "resources" in getattr(ctx.attrs, "exopackage_modes", [])
    primary_resources_apk, exopackaged_assets, exopackaged_assets_hash = _merge_assets(
        ctx,
        is_exopackaged_enabled_for_resources,
        aapt2_link_info.primary_resources_apk,
        resource_infos,
        cxx_resources,
    )

    if is_exopackaged_enabled_for_resources:
        r_dot_txt = ctx.actions.declare_output("after_exo/R.txt")
        primary_resources_apk = ctx.actions.declare_output("after_exo/primary_resources_apk.apk")
        exo_resources = ctx.actions.declare_output("exo_resources.apk")
        exo_resources_hash = ctx.actions.declare_output("exo_resources.apk.hash")
        ctx.actions.run(cmd_args([
            android_toolchain.exo_resources_rewriter[RunInfo],
            "--original-r-dot-txt",
            aapt2_link_info.r_dot_txt,
            "--new-r-dot-txt",
            r_dot_txt.as_output(),
            "--original-primary-apk-resources",
            aapt2_link_info.primary_resources_apk,
            "--new-primary-apk-resources",
            primary_resources_apk.as_output(),
            "--exo-resources",
            exo_resources.as_output(),
            "--exo-resources-hash",
            exo_resources_hash.as_output(),
            "--zipalign-tool",
            android_toolchain.zipalign[RunInfo],
        ]), category = "write_exo_resources")

        third_party_jars = ctx.actions.write("third_party_jars", prebuilt_jars)
        third_party_jar_resources = ctx.actions.declare_output("third_party_jars.resources")
        third_party_jar_resources_hash = ctx.actions.declare_output("third_party_jars.resources.hash")
        ctx.actions.run(cmd_args([
            android_toolchain.merge_third_party_jar_resources[RunInfo],
            "--output",
            third_party_jar_resources.as_output(),
            "--output-hash",
            third_party_jar_resources_hash.as_output(),
            "--third-party-jars",
            third_party_jars,
        ]).hidden(prebuilt_jars), category = "merge_third_party_jar_resources")

        exopackage_info = ExopackageResourcesInfo(
            assets = exopackaged_assets,
            assets_hash = exopackaged_assets_hash,
            res = exo_resources,
            res_hash = exo_resources_hash,
            third_party_jar_resources = third_party_jar_resources,
            third_party_jar_resources_hash = third_party_jar_resources_hash,
        )
        jar_files_that_may_contain_resources = []
    else:
        exopackage_info = None
        jar_files_that_may_contain_resources = prebuilt_jars
        r_dot_txt = aapt2_link_info.r_dot_txt

    override_symbols_paths = [override_symbols] if override_symbols else []
    resources = [resource for resource in resource_infos if resource.res != None]
    r_dot_javas = [] if len(resources) == 0 else generate_r_dot_javas(
        ctx,
        ctx.attrs._android_toolchain[AndroidToolchainInfo].merge_android_resources[RunInfo],
        ctx.attrs._java_toolchain[JavaToolchainInfo],
        resources,
        get_effective_banned_duplicate_resource_types(
            getattr(ctx.attrs, "duplicate_resource_behavior", "allow_by_default"),
            getattr(ctx.attrs, "allowed_duplicate_resource_types", []),
            getattr(ctx.attrs, "banned_duplicate_resource_types", []),
        ),
        [r_dot_txt],
        override_symbols_paths,
        getattr(ctx.attrs, "duplicate_resource_whitelist", None),
        getattr(ctx.attrs, "resource_union_package", None),
        referenced_resources_lists,
    )
    string_source_map = _maybe_generate_string_source_map(
        ctx.actions,
        getattr(ctx.attrs, "build_string_source_map", False),
        resources,
        android_toolchain,
    )
    packaged_string_assets = _maybe_package_strings_as_assets(
        ctx,
        string_files_list,
        string_files_res_dirs,
        r_dot_txt,
        android_toolchain,
    )

    return AndroidBinaryResourcesInfo(
        exopackage_info = exopackage_info,
        manifest = android_manifest,
        packaged_string_assets = packaged_string_assets,
        primary_resources_apk = primary_resources_apk,
        proguard_config_file = aapt2_link_info.proguard_config_file,
        r_dot_javas = r_dot_javas,
        string_source_map = string_source_map,
        jar_files_that_may_contain_resources = jar_files_that_may_contain_resources,
        unfiltered_resource_infos = unfiltered_resource_infos,
    )

def _maybe_filter_resources(
        ctx: "context",
        resources: [AndroidResourceInfo.type],
        android_toolchain: AndroidToolchainInfo.type) -> ([AndroidResourceInfo.type], ["artifact", None], ["artifact", None], ["artifact"]):
    resources_filter_strings = getattr(ctx.attrs, "resource_filter", [])
    resources_filter = _get_resources_filter(resources_filter_strings)
    resource_compression_mode = getattr(ctx.attrs, "resource_compression", "disabled")
    is_store_strings_as_assets = _is_store_strings_as_assets(resource_compression_mode)
    locales = getattr(ctx.attrs, "locales", None)
    use_aapt2_locale_filtering = getattr(ctx.attrs, "aapt2_locale_filtering", False)
    needs_resource_filtering_for_locales = locales != None and len(locales) > 0 and not use_aapt2_locale_filtering
    post_filter_resources_cmd = getattr(ctx.attrs, "post_filter_resources_cmd", None)

    needs_resource_filtering = (
        resources_filter != None or
        is_store_strings_as_assets or
        needs_resource_filtering_for_locales or
        post_filter_resources_cmd != None
    )

    if not needs_resource_filtering:
        return resources, None, None, []

    res_info_to_out_res_dir = {}
    res_infos_with_no_res = []
    skip_crunch_pngs = getattr(ctx.attrs, "skip_crunch_pngs", None) or False
    for i, resource in enumerate(resources):
        if resource.res == None:
            res_infos_with_no_res.append(resource)
        else:
            filtered_res = ctx.actions.declare_output("filtered_res_{}".format(i))
            res_info_to_out_res_dir[resource] = filtered_res

    filter_resources_cmd = cmd_args(android_toolchain.filter_resources[RunInfo])
    in_res_dir_to_out_res_dir_dict = {
        in_res.res: out_res
        for in_res, out_res in res_info_to_out_res_dir.items()
    }
    in_res_dir_to_out_res_dir_map = ctx.actions.write_json("in_res_dir_to_out_res_dir_map", {"res_dir_map": in_res_dir_to_out_res_dir_dict})
    in_res_dirs = [in_res.res for in_res in res_info_to_out_res_dir.keys()]
    filter_resources_cmd.hidden(in_res_dirs)
    filter_resources_cmd.hidden([out_res.as_output() for out_res in res_info_to_out_res_dir.values()])
    filter_resources_cmd.add([
        "--in-res-dir-to-out-res-dir-map",
        in_res_dir_to_out_res_dir_map,
    ])

    if resources_filter:
        filter_resources_cmd.add([
            "--target-densities",
            ",".join(resources_filter.densities),
        ])

    all_strings_files_list = None
    all_strings_files_res_dirs = []
    if is_store_strings_as_assets:
        all_strings_files_list = ctx.actions.declare_output("all_strings_files")
        all_strings_files_res_dirs = in_res_dirs
        filter_resources_cmd.add([
            "--enable-string-as-assets-filtering",
            "--string-files-list-output",
            all_strings_files_list.as_output(),
        ])

        packaged_locales = getattr(ctx.attrs, "packaged_locales", [])
        if packaged_locales:
            filter_resources_cmd.add([
                "--packaged-locales",
                ",".join(packaged_locales),
            ])

        not_filtered_string_dirs = [resource.res for resource in resources if not resource.allow_strings_as_assets_resource_filtering]
        if not_filtered_string_dirs:
            filter_resources_cmd.add([
                "--not-filtered-string-dirs",
                ctx.actions.write("not_filtered_string_dirs", not_filtered_string_dirs),
            ])

    if needs_resource_filtering_for_locales:
        filter_resources_cmd.add([
            "--locales",
            ",".join(locales),
        ])

    override_symbols_artifact = None
    if post_filter_resources_cmd != None:
        override_symbols_artifact = ctx.actions.declare_output("post_filter_resources_cmd/R.json")
        filter_resources_cmd.add([
            "--post-filter-resources-cmd",
            post_filter_resources_cmd,
            "--post-filter-resources-cmd-override-symbols-output",
            override_symbols_artifact.as_output(),
        ])

    ctx.actions.run(filter_resources_cmd, category = "filter_resources")

    filtered_resource_infos = []
    for i, resource in enumerate(resources):
        if resource.res == None:
            continue

        filtered_res = res_info_to_out_res_dir[resource]
        filtered_aapt2_compile_output = aapt2_compile(
            ctx,
            filtered_res,
            android_toolchain,
            skip_crunch_pngs = skip_crunch_pngs,
            identifier = "filtered_res_{}".format(i),
        )
        filtered_resource = AndroidResourceInfo(
            aapt2_compile_output = filtered_aapt2_compile_output,
            assets = resource.assets,
            manifest_file = resource.manifest_file,
            r_dot_java_package = resource.r_dot_java_package,
            res = filtered_res,
            text_symbols = resource.text_symbols,
        )
        filtered_resource_infos.append(filtered_resource)

    return (
        res_infos_with_no_res + filtered_resource_infos,
        override_symbols_artifact,
        all_strings_files_list,
        all_strings_files_res_dirs,
    )

ResourcesFilter = record(
    densities = [str.type],
    downscale = bool.type,
)

def _get_resources_filter(resources_filter_strings: [str.type]) -> [ResourcesFilter.type, None]:
    if not resources_filter_strings:
        return None

    densities = [resources_filter_string for resources_filter_string in resources_filter_strings if resources_filter_string != "downscale"]
    if not densities:
        return None

    downscale = len(densities) < len(resources_filter_strings)
    return ResourcesFilter(densities = densities, downscale = downscale)

def _maybe_generate_string_source_map(
        actions: "actions",
        should_build_source_string_map: bool.type,
        resource_infos: [AndroidResourceInfo.type],
        android_toolchain: AndroidToolchainInfo.type) -> ["artifact", None]:
    if not should_build_source_string_map or len(resource_infos) == 0:
        return None

    res_dirs = [resource_info.res for resource_info in resource_infos]
    output = actions.declare_output("string_source_map")
    res_dirs_file = actions.write("resource_dirs_for_string_source_map", res_dirs)
    generate_string_source_map_cmd = cmd_args([
        android_toolchain.copy_string_resources[RunInfo],
        "--res-dirs",
        res_dirs_file,
        "--output",
        output.as_output(),
    ]).hidden(res_dirs)

    actions.run(generate_string_source_map_cmd, category = "generate_string_source_map")

    return output

def _maybe_package_strings_as_assets(
        ctx: "context",
        string_files_list: ["artifact", None],
        string_files_res_dirs: ["artifact"],
        r_dot_txt: "artifact",
        android_toolchain: AndroidToolchainInfo.type) -> ["artifact", None]:
    resource_compression_mode = getattr(ctx.attrs, "resource_compression", "disabled")
    is_store_strings_as_assets = _is_store_strings_as_assets(resource_compression_mode)
    expect(is_store_strings_as_assets == (string_files_list != None))

    if not is_store_strings_as_assets:
        return None

    string_assets_dir = ctx.actions.declare_output("package_strings_as_assets/string_assets")
    string_assets_zip = ctx.actions.declare_output("package_strings_as_assets/string_assets_zip.zip")
    all_locales_string_assets_zip = ctx.actions.declare_output("package_strings_as_assets/all_locales_string_assets_zip.zip")

    locales = getattr(ctx.attrs, "locales", [])

    package_strings_as_assets_cmd = cmd_args([
        android_toolchain.package_strings_as_assets[RunInfo],
        "--string-files-list",
        string_files_list,
        "--r-dot-txt",
        r_dot_txt,
        "--string-assets-dir",
        string_assets_dir.as_output(),
        "--string-assets-zip",
        string_assets_zip.as_output(),
        "--all-locales-string-assets-zip",
        all_locales_string_assets_zip.as_output(),
    ]).hidden(string_files_res_dirs)

    if locales:
        package_strings_as_assets_cmd.add("--locales", ",".join(locales))

    ctx.actions.run(package_strings_as_assets_cmd, category = "package_strings_as_assets")

    return string_assets_zip

def _get_manifest(
        ctx: "context",
        android_packageable_info: "AndroidPackageableInfo",
        manifest_entries: dict.type) -> "artifact":
    robolectric_manifest = getattr(ctx.attrs, "robolectric_manifest", None)
    if robolectric_manifest:
        return robolectric_manifest

    if ctx.attrs.manifest:
        expect(ctx.attrs.manifest_skeleton == None, "Only one of manifest and manifest_skeleton should be declared")
        if type(ctx.attrs.manifest) == "dependency":
            android_manifest = ctx.attrs.manifest[DefaultInfo].default_outputs[0]
        else:
            android_manifest = ctx.attrs.manifest
    else:
        expect(ctx.attrs.manifest_skeleton != None, "Must declare one of manifest and manifest_skeleton")
        if type(ctx.attrs.manifest_skeleton) == "dependency":
            manifest_skeleton = ctx.attrs.manifest_skeleton[DefaultInfo].default_outputs[0]
        else:
            manifest_skeleton = ctx.attrs.manifest_skeleton
        android_manifest, _ = generate_android_manifest(
            ctx,
            ctx.attrs._android_toolchain[AndroidToolchainInfo].generate_manifest[RunInfo],
            manifest_skeleton,
            "dex",  # ROOT_APKMODULE_NAME,
            android_packageable_info.manifests,
            manifest_entries.get("placeholders", {}),
        )

    return android_manifest

# Returns the "primary resources APK" (i.e. the resource that are packaged into the primary APK),
# and optionally an "exopackaged assets APK" and the hash for that APK.
def _merge_assets(
        ctx: "context",
        is_exopackaged_enabled_for_resources: bool.type,
        base_apk: "artifact",
        resource_infos: ["AndroidResourceInfo"],
        cxx_resources: ["artifact", None]) -> ("artifact", ["artifact", None], ["artifact", None]):
    assets_dirs = [resource_info.assets for resource_info in resource_infos if resource_info.assets]
    if cxx_resources != None:
        assets_dirs.extend([cxx_resources])
    if len(assets_dirs) == 0:
        return base_apk, None, None

    merge_assets_cmd = cmd_args(ctx.attrs._android_toolchain[AndroidToolchainInfo].merge_assets[RunInfo])

    merged_assets_output = ctx.actions.declare_output("merged_assets.ap_")
    merge_assets_cmd.add(["--output-apk", merged_assets_output.as_output()])

    if is_exopackaged_enabled_for_resources:
        merged_assets_output_hash = ctx.actions.declare_output("merged_assets.ap_.hash")
        merge_assets_cmd.add(["--output-apk-hash", merged_assets_output_hash.as_output()])
    else:
        merge_assets_cmd.add(["--base-apk", base_apk])
        merged_assets_output_hash = None

    assets_dirs_file = ctx.actions.write("assets_dirs", assets_dirs)
    merge_assets_cmd.add(["--assets-dirs", assets_dirs_file])
    merge_assets_cmd.hidden(assets_dirs)

    ctx.actions.run(merge_assets_cmd, category = "merge_assets")

    if is_exopackaged_enabled_for_resources:
        return base_apk, merged_assets_output, merged_assets_output_hash
    else:
        return merged_assets_output, None, None

def get_effective_banned_duplicate_resource_types(
        duplicate_resource_behavior: str.type,
        allowed_duplicate_resource_types: [str.type],
        banned_duplicate_resource_types: [str.type]) -> [str.type]:
    if duplicate_resource_behavior == "allow_by_default":
        expect(
            len(allowed_duplicate_resource_types) == 0,
            "Cannot set allowed_duplicate_resource_types if duplicate_resource_behaviour is allow_by_default",
        )
        return banned_duplicate_resource_types
    elif duplicate_resource_behavior == "ban_by_default":
        expect(
            len(banned_duplicate_resource_types) == 0,
            "Cannot set banned_duplicate_resource_types if duplicate_resource_behaviour is ban_by_default",
        )
        return [rtype for rtype in RType if rtype not in allowed_duplicate_resource_types]
    else:
        fail("Unrecognized duplicate_resource_behavior: {}".format(duplicate_resource_behavior))

def _get_cxx_resources(ctx: "context", deps: ["dependency"]) -> ["artifact", None]:
    cxx_resources = gather_resources(
        label = ctx.label,
        resources = {},
        deps = deps,
    )

    symlink_tree_dict = {}
    resource_maps = cxx_resources.values()
    for resource_map in resource_maps:
        for name, (resource, _other) in resource_map.items():
            symlink_tree_dict["cxx-resources/{}".format(name)] = resource

    return ctx.actions.symlinked_dir("cxx_resources_dir", symlink_tree_dict) if symlink_tree_dict else None

def _is_store_strings_as_assets(resource_compression: str.type) -> bool.type:
    return resource_compression == "enabled_strings_only" or resource_compression == "enabled_with_strings_as_assets"
