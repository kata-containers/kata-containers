# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under both the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree and the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree.

load("@prelude//android:android_toolchain.bzl", "AndroidToolchainInfo")
load("@prelude//java/utils:java_utils.bzl", "get_path_separator")
load("@prelude//utils:utils.bzl", "expect")

def get_preprocessed_java_classes(ctx: "context", input_jars = {"artifact": "target_label"}) -> {"artifact": "target_label"}:
    sh_script, _ = ctx.actions.write(
        "preprocessed_java_classes/script.sh",
        cmd_args(ctx.attrs.preprocess_java_classes_bash),
        is_executable = True,
        allow_args = True,
    )

    preprocess_cmd = cmd_args(["/bin/bash", sh_script])
    preprocess_cmd.hidden(cmd_args(ctx.attrs.preprocess_java_classes_bash))
    for dep in ctx.attrs.preprocess_java_classes_deps:
        preprocess_cmd.hidden(dep[DefaultInfo].default_outputs + dep[DefaultInfo].other_outputs)

    input_srcs = {}
    output_jars = {}

    for i, (input_jar, target_label) in enumerate(input_jars.items()):
        expect(input_jar.extension == ".jar", "Expected {} to have extension .jar!".format(input_jar))
        jar_name = "{}_{}".format(i, input_jar.basename)
        input_srcs[jar_name] = input_jar
        output_jar = ctx.actions.declare_output(
            "preprocessed_java_classes/output_dir/{}".format(jar_name),
        )
        output_jars[output_jar] = target_label
        preprocess_cmd.hidden(output_jar.as_output())

    if not output_jars:
        return {}

    input_dir = ctx.actions.symlinked_dir("preprocessed_java_classes/input_dir", input_srcs)
    output_dir = cmd_args(output_jars.keys()[0].as_output()).parent()

    env = {
        "ANDROID_BOOTCLASSPATH": cmd_args(
            ctx.attrs._android_toolchain[AndroidToolchainInfo].android_bootclasspath,
            delimiter = get_path_separator(),
        ),
        "IN_JARS_DIR": cmd_args(input_dir),
        "OUT_JARS_DIR": output_dir,
    }

    ctx.actions.run(preprocess_cmd, env = env, category = "preprocess_java_classes")

    return output_jars
