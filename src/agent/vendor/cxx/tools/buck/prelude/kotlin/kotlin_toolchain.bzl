# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under both the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree and the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree.

KotlincProtocol = enum("classic", "kotlincd")

KotlinToolchainInfo = provider(
    "Kotlin toolchain info",
    fields = [
        "annotation_processing_jar",
        "compile_kotlin",
        "kapt_base64_encoder",
        "kotlinc",
        "kotlinc_classpath",
        "kotlinc_protocol",
        "kotlin_stdlib",
        "kotlin_home_libraries",
        "kosabi_stubs_gen_plugin",
        "kosabi_applicability_plugin",
        "kosabi_jvm_abi_gen_plugin",
    ],
)
