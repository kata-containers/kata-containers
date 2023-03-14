# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under both the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree and the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree.

AbiGenerationMode = enum("class", "source", "source_only")

JavacProtocol = enum("classic", "javacd")

JavaPlatformInfo = provider(
    "Java platform info",
    fields = [
        "name",
    ],
)

JavaToolchainInfo = provider(
    "Java toolchain info",
    fields = [
        "abi_generation_mode",
        "ast_dumper",
        "bootclasspath_7",
        "bootclasspath_8",
        "class_abi_generator",
        "compile_and_package",
        "dep_files",
        "fallback_javac",
        "fat_jar",
        "fat_jar_main_class_lib",
        "jar",
        "jar_builder",
        "java",
        "java_for_tests",
        "javac",
        "javac_protocol",
        "src_dir_helper",
        "source_level",
        "src_root_elements",
        "src_root_prefixes",
        "target_level",
        "used_classes_to_dep_file",
        "zip_scrubber",
        "is_bootstrap_toolchain",
    ],
)

JavaTestToolchainInfo = provider(
    "Java test toolchain info",
    fields = [
        "java_custom_class_loader_class",
        "java_custom_class_loader_library_jar",
        "java_custom_class_loader_vm_args",
        "test_runner_library_jar",
        "junit_test_runner_main_class_args",
        "junit5_test_runner_main_class_args",
        "testng_test_runner_main_class_args",
        "list_class_names",
        "use_java_custom_class_loader",
    ],
)

# prebuilt_jar needs so little of the Java toolchain that it's worth
# giving it its own to reduce the occurrence of cycles as we add
# more Java- and Kotlin-built tools to the Java and Kotlin toolchains
PrebuiltJarToolchainInfo = provider(
    "prebuilt_jar toolchain info",
    fields = [
        "class_abi_generator",
        "is_bootstrap_toolchain",
    ],
)
