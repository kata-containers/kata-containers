# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under both the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree and the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree.

load("@prelude//java:java_toolchain.bzl", "AbiGenerationMode", "JavaToolchainInfo")
load("@prelude//utils:utils.bzl", "expect")

def get_path_separator() -> "string":
    # TODO: msemko : replace with system-dependent path-separator character
    # On UNIX systems, this character is ':'; on Microsoft Windows systems it is ';'.
    return ":"

def derive_javac(javac_attribute: [str.type, "dependency", "artifact"]) -> [str.type, "RunInfo", "artifact"]:
    javac_attr_type = type(javac_attribute)
    if javac_attr_type == "dependency":
        javac_run_info = javac_attribute.get(RunInfo)
        if javac_run_info:
            return javac_run_info
        outputs = javac_attribute[DefaultInfo].default_outputs
        expect(len(outputs) == 1, "Expect one default output from build dep of attr javac!")
        return outputs[0]

    if javac_attr_type == "artifact":
        return javac_attribute

    if javac_attr_type == str.type:
        return javac_attribute

    fail("Type of attribute javac {} that equals to {} is not supported.\n Supported types are \"dependency\", \"artifact\" and \"string\".".format(javac_attr_type, javac_attribute))

def get_java_version_attributes(ctx: "context") -> (int.type, int.type):
    java_toolchain = ctx.attrs._java_toolchain[JavaToolchainInfo]
    java_version = ctx.attrs.java_version
    java_source = ctx.attrs.source
    java_target = ctx.attrs.target

    if java_version:
        if java_source or java_target:
            fail("No need to set 'source' and/or 'target' attributes when 'java_version' is present")
        java_version = to_java_version(java_version)
        return (java_version, java_version)

    source = java_source or java_toolchain.source_level
    target = java_target or java_toolchain.target_level

    expect(bool(source) and bool(target), "Java source level and target level must be set!")

    source = to_java_version(source)
    target = to_java_version(target)

    expect(source <= target, "java library source level {} is higher than target {} ", source, target)

    return (source, target)

def to_java_version(java_version: str.type) -> int.type:
    if java_version.startswith("1."):
        expect(len(java_version) == 3, "Supported java version number format is 1.X, where X is a single digit numnber, but it was set to {}", java_version)
        java_version_number = int(java_version[2:])
        expect(java_version_number < 9, "Supported java version number format is 1.X, where X is a single digit numnber that is less than 9, but it was set to {}", java_version)
        return java_version_number
    else:
        return int(java_version)

def get_abi_generation_mode(abi_generation_mode):
    return {
        None: None,
        "class": AbiGenerationMode("class"),
        "migrating_to_source_only": AbiGenerationMode("source"),
        "source": AbiGenerationMode("source"),
        "source_only": AbiGenerationMode("source_only"),
    }[abi_generation_mode]

def get_default_info(
        outputs: ["JavaCompileOutputs", None],
        extra_sub_targets: dict.type = {}) -> DefaultInfo.type:
    sub_targets = {}
    default_info = DefaultInfo()
    if outputs:
        abis = [
            ("class-abi", outputs.class_abi),
            ("source-abi", outputs.source_abi),
            ("source-only-abi", outputs.source_only_abi),
        ]
        for (name, artifact) in abis:
            if artifact != None:
                sub_targets[name] = [DefaultInfo(default_outputs = [artifact])]
        default_info = DefaultInfo(
            default_outputs = [outputs.full_library],
            sub_targets = extra_sub_targets | sub_targets,
        )
    return default_info
