# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under both the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree and the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree.

AndroidPlatformInfo = provider(fields = [
    "name",
])

AndroidToolchainInfo = provider(fields = [
    "aapt2",
    "adb",
    "aidl",
    "android_jar",
    "android_bootclasspath",
    "apk_builder",
    "apk_module_graph",
    "combine_native_library_dirs",
    "compress_libraries",
    "d8_command",
    "exo_resources_rewriter",
    "exopackage_agent_apk",
    "filter_dex_class_names",
    "filter_prebuilt_native_library_dir",
    "multi_dex_command",
    "copy_string_resources",
    "filter_resources",
    "framework_aidl_file",
    "generate_build_config",
    "generate_manifest",
    "instrumentation_test_runner_classpath",
    "instrumentation_test_runner_main_class",
    "manifest_utils",
    "merge_android_resources",
    "merge_assets",
    "merge_third_party_jar_resources",
    "mini_aapt",
    "native_libs_as_assets_metadata",
    "optimized_proguard_config",
    "package_strings_as_assets",
    "proguard_config",
    "proguard_jar",
    "proguard_max_heap_size",
    "r_dot_java_weight_factor",
    "secondary_dex_weight_limit",
    "unpack_aar",
    "zipalign",
])
